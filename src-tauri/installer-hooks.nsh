; dockwin NSIS installer hooks.
;
; Tauri's NSIS template inserts these four macros at the matching points of the
; generated (un)installer, and `!include`s this file at top level — so it may
; also define top-level Functions (used below for the PATH helpers).
;
; Two things happen here:
;   1. PATH: add the install dir to the per-user PATH on install, so the bundled
;      `dockwin` CLI works from any shell. Scope is HKCU to match
;      `installMode: currentUser`.
;
;      We deliberately do NOT edit HKCU\Environment\Path from NSIS directly.
;      NSIS strings are capped at NSIS_MAX_STRLEN (1024 in the stock build Tauri
;      ships), so `ReadRegStr` SILENTLY TRUNCATES any real-world PATH past 1024
;      chars, and writing that back DESTROYS everything beyond it — in the worst
;      case collapsing the whole PATH down to a single entry. Instead we shell
;      out to PowerShell and edit HKCU\Environment\Path via the .NET registry
;      API, which has no such length limit.
;
;      TWO subtleties that both matter and are both verified by test:
;        * We read with DoNotExpandEnvironmentNames and write with
;          RegistryValueKind.ExpandString (REG_EXPAND_SZ). We do NOT use
;          [Environment]::SetEnvironmentVariable: under Windows PowerShell /
;          .NET Framework it rewrites the value as REG_SZ, which would stop
;          Windows from expanding existing %VAR% entries (e.g. %USERPROFILE%\bin)
;          in the user's PATH — silently breaking them.
;        * $INSTDIR is passed as an argument ($args[0]), never embedded into the
;          script text, so a path containing a quote can't break the script.
;      The append is idempotent (skips if the dir is already present). No
;      WM_SETTINGCHANGE broadcast is sent; a newly opened shell picks up the
;      change (already-running shells wouldn't refresh from a broadcast anyway).
;
;      Deliberately NOT removed on uninstall: a read-modify-write against the
;      shared per-user PATH can race with another installer doing the same
;      thing at the same time, and the loser's write clobbers the winner's. A
;      stale PATH entry after uninstall is harmless clutter; that failure mode
;      is not.
;   2. ENGINE: before the app files are deleted, offer to tear down the dockwin
;      engine (the WSL2 distro + docker context + %LOCALAPPDATA%\dockwin\distro)
;      via the bundled CLI's own teardown — otherwise "Uninstall" leaves an
;      orphaned WSL distro behind.

; ---------------------------------------------------------------------------
; Per-user PATH helper (plugin-free, via PowerShell/.NET).
; ---------------------------------------------------------------------------

; Append $INSTDIR to the per-user PATH, unless it is already present. See the
; header comment for WHY this goes through PowerShell/.NET instead of NSIS's own
; registry ops. We write a tiny helper script to $PLUGINSDIR (auto-created by
; InitPluginsDir, auto-deleted at the end of the run) and run it, passing
; $INSTDIR as an argument so its value never needs quoting inside the script.
Function dockwinPathAdd
  Push $0 ; file handle / nsExec exit code

  ; Guarantee $PLUGINSDIR exists even if no plugin has been used yet. Idempotent.
  InitPluginsDir

  FileOpen $0 "$PLUGINSDIR\dockwin-path.ps1" w
  FileWrite $0 "$$ErrorActionPreference = 'Stop'$\r$\n"
  FileWrite $0 "$$dir = $$args[0]$\r$\n"
  FileWrite $0 "$$key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')$\r$\n"
  FileWrite $0 "try {$\r$\n"
  FileWrite $0 "  $$path = [string]$$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)$\r$\n"
  FileWrite $0 "  if ([string]::IsNullOrEmpty($$path)) {$\r$\n"
  FileWrite $0 "    $$new = $$dir$\r$\n"
  FileWrite $0 "  } elseif (($$path -split ';') -notcontains $$dir) {$\r$\n"
  FileWrite $0 "    $$new = $$path.TrimEnd(';') + ';' + $$dir$\r$\n"
  FileWrite $0 "  } else {$\r$\n"
  FileWrite $0 "    $$new = $$null$\r$\n"
  FileWrite $0 "  }$\r$\n"
  FileWrite $0 "  if ($$null -ne $$new) {$\r$\n"
  FileWrite $0 "    $$key.SetValue('Path', $$new, [Microsoft.Win32.RegistryValueKind]::ExpandString)$\r$\n"
  FileWrite $0 "  }$\r$\n"
  FileWrite $0 "} finally {$\r$\n"
  FileWrite $0 "  $$key.Close()$\r$\n"
  FileWrite $0 "}$\r$\n"
  FileClose $0

  DetailPrint "Adding $INSTDIR to the per-user PATH..."
  nsExec::ExecToLog 'powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\dockwin-path.ps1" "$INSTDIR"'
  Pop $0
  DetailPrint "PATH update finished (exit code $0)."

  Pop $0
FunctionEnd

; ---------------------------------------------------------------------------
; Hooks
; ---------------------------------------------------------------------------

!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Put `dockwin` on the per-user PATH so the CLI works from any shell.
  Call dockwinPathAdd
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; On a silent uninstall (/S) never destroy WSL data without an explicit click;
  ; automation can run `dockwin uninstall` itself. Just skip the engine teardown.
  IfSilent dockwin_skip_engine

  ; Only offer it when the bundled CLI is actually present next to the app.
  IfFileExists "$INSTDIR\dockwin.exe" 0 dockwin_skip_engine

  MessageBox MB_YESNO|MB_ICONEXCLAMATION \
    "Also remove the dockwin engine?$\r$\n$\r$\nThis unregisters the 'dockwin' WSL2 distro and PERMANENTLY deletes its containers, images and volumes.$\r$\n$\r$\nChoose No to keep the engine and only remove the app." \
    /SD IDNO IDYES dockwin_remove_engine IDNO dockwin_skip_engine

  dockwin_remove_engine:
    DetailPrint "Removing dockwin engine (wsl --unregister + docker context)..."
    ; -y => non-interactive teardown: terminate, unregister, drop context, clean dir.
    nsExec::ExecToLog '"$INSTDIR\dockwin.exe" uninstall -y'
    Pop $0
    DetailPrint "dockwin engine teardown finished (exit code $0)."

  dockwin_skip_engine:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Intentionally not removing $INSTDIR from PATH here — see the comment at
  ; the top of this file for why.
!macroend
