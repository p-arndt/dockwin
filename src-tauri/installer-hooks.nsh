; dockwin NSIS installer hooks.
;
; Tauri's NSIS template inserts these four macros at the matching points of the
; generated (un)installer. We only need PREUNINSTALL: BEFORE the app files are
; deleted, offer to tear down the dockwin engine by invoking the bundled CLI's
; own teardown (`dockwin uninstall -y`) — which runs `wsl --unregister dockwin`,
; removes the `dockwin` docker context, and cleans %LOCALAPPDATA%\dockwin\distro.
;
; Without this hook, "Uninstall" from Windows Settings removes ONLY the app
; binaries and leaves the WSL2 distro (with its containers/images/volumes) and
; the docker context orphaned — the exact dangling-registration state that makes
; a later reinstall fail to boot.

!macro NSIS_HOOK_PREINSTALL
!macroend

!macro NSIS_HOOK_POSTINSTALL
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
!macroend
