; dockwin NSIS installer hooks.
;
; Tauri's NSIS template inserts these four macros at the matching points of the
; generated (un)installer, and `!include`s this file at top level — so it may
; also define top-level Functions (used below for the PATH helpers).
;
; Two things happen here:
;   1. PATH: add the install dir to the per-user PATH on install, so the bundled
;      `dockwin` CLI works from any shell. The common Tauri/NSIS recipe uses the
;      EnVar plugin, but Tauri does not bundle it, so rather than vendoring a
;      binary DLL we edit HKCU\Environment directly and broadcast
;      WM_WININICHANGE. Scope is HKCU to match `installMode: currentUser`.
;      Deliberately NOT removed on uninstall: a read-modify-write against the
;      shared per-user PATH can race with another installer doing the same
;      thing at the same time, and the loser's write clobbers the winner's —
;      in the worst case truncating the whole PATH down to one entry. A stale
;      PATH entry after uninstall is harmless clutter; that failure mode is not.
;   2. ENGINE: before the app files are deleted, offer to tear down the dockwin
;      engine (the WSL2 distro + docker context + %LOCALAPPDATA%\dockwin\distro)
;      via the bundled CLI's own teardown — otherwise "Uninstall" leaves an
;      orphaned WSL distro behind.

!include "WinMessages.nsh"

; ---------------------------------------------------------------------------
; Per-user PATH helpers (plugin-free).
; ---------------------------------------------------------------------------

; Append $INSTDIR to the per-user PATH, unless it is already present.
Function dockwinPathAdd
  Push $0 ; current PATH
  Push $2 ; sliding window
  Push $3 ; len($INSTDIR)
  Push $4 ; scan index

  ReadRegStr $0 HKCU "Environment" "Path"
  StrLen $3 "$INSTDIR"
  StrCpy $4 0
  dockwin_pa_scan:
    StrCpy $2 $0 $3 $4
    StrCmp $2 "$INSTDIR" dockwin_pa_done   ; already on PATH -> nothing to do
    StrCmp $2 "" dockwin_pa_append         ; ran past the end -> not found
    IntOp $4 $4 + 1
    Goto dockwin_pa_scan
  dockwin_pa_append:
    StrCmp $0 "" 0 dockwin_pa_sep
      StrCpy $0 "$INSTDIR"                 ; PATH was empty
      Goto dockwin_pa_write
    dockwin_pa_sep:
      StrCpy $0 "$0;$INSTDIR"
  dockwin_pa_write:
    WriteRegExpandStr HKCU "Environment" "Path" "$0"
    SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
  dockwin_pa_done:

  Pop $4
  Pop $3
  Pop $2
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
