!define DAYMARK_RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define DAYMARK_RUN_VALUE "Daymark"

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "${DAYMARK_RUN_KEY}" "${DAYMARK_RUN_VALUE}" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Push $0
  ReadRegStr $0 HKCU "${DAYMARK_RUN_KEY}" "${DAYMARK_RUN_VALUE}"
  ${If} $UpdateMode <> 1
  ${AndIf} $0 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\""
    DeleteRegValue HKCU "${DAYMARK_RUN_KEY}" "${DAYMARK_RUN_VALUE}"
  ${EndIf}
  Pop $0
!macroend
