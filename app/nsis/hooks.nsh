; The autostart plugin writes a Run value named after productName; without it Windows keeps starting a missing exe
; at login. Settings and the cache store stay, so a reinstall keeps them; an update (/UPDATE) keeps autostart too.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${PRODUCTNAME}"
  ${EndIf}
!macroend
