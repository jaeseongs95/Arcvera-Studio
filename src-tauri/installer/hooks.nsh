; Preserve running work during both a renamed upgrade and a normal reinstall.
; Query only the exact installed executable; never shut down its processes.
!macro ArcveraRequireStopped executablePath
  !insertmacro RestartManager_StartSession $R0
  ${If} $R0 == ""
    Abort "Arcvera-Studio could not verify that the installed workspace is closed. Close it and retry."
  ${EndIf}
  !insertmacro RestartManager_RegisterFile $R0 "${executablePath}"
  ${If} $0 != 0
    !insertmacro RestartManager_EndSession $R0
    Abort "Arcvera-Studio could not check the installed executable. No running processes were stopped."
  ${EndIf}
  ; pnProcInfo is an in/out count; there is deliberately no process buffer.
  StrCpy $2 0
  System::Call 'RSTRTMGR::RmGetList(i R0, *i .r1, *i r2, p 0, *i .r3) i .r0'
  StrCpy $R9 $0
  !insertmacro RestartManager_EndSession $R0
  ${If} $R9 != 0
    Abort "Close the installed Arcvera-Studio / OpenDIY Desktop workspace before installing. No running processes were stopped."
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro ArcveraRequireStopped "$INSTDIR\opendiy-desktop.exe"
  !insertmacro ArcveraRequireStopped "$INSTDIR\${MAINBINARYNAME}.exe"
!macroend

; Rename a legacy shortcut only if its target belongs to this installation.
; Leave a conflicting user-created destination shortcut untouched.
; The template also calls this before POSTINSTALL and from the finish page.
; Existing owned links have already been retargeted by its migration checks.
!macro ArcveraCreateShortcutIfAbsent shortcut
  ${If} ${FileExists} "${shortcut}"
    DetailPrint "Existing shortcut retained: ${shortcut}"
  ${Else}
    ClearErrors
    CreateShortcut "${shortcut}" "$INSTDIR\${MAINBINARYNAME}.exe"
    ${If} ${Errors}
      Abort "Could not create the workspace shortcut. Installation files are present; repair the shortcut before launching."
    ${EndIf}
    !insertmacro SetLnkAppUserModelId "${shortcut}"
  ${EndIf}
!macroend

!macro ArcveraMigrateShortcut legacy current
  !insertmacro IsShortcutTarget "${legacy}" "$INSTDIR\opendiy-desktop.exe"
  Pop $R8
  ${If} $R8 = 1
    ${If} ${FileExists} "${current}"
      !insertmacro IsShortcutTarget "${current}" "$INSTDIR\${MAINBINARYNAME}.exe"
      Pop $R8
      ${If} $R8 = 1
        Delete "${legacy}"
      ${Else}
        !insertmacro SetShortcutTarget "${legacy}" "$INSTDIR\${MAINBINARYNAME}.exe"
        DetailPrint "Legacy shortcut retained because its new name is already in use: ${current}"
      ${EndIf}
    ${Else}
      ClearErrors
      Rename "${legacy}" "${current}"
      ${If} ${Errors}
        Abort "Could not rename the existing workspace shortcut. Installation files are present; repair the shortcut before launching."
      ${EndIf}
      !insertmacro SetShortcutTarget "${current}" "$INSTDIR\${MAINBINARYNAME}.exe"
      !insertmacro SetLnkAppUserModelId "${current}"
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro ArcveraMigrateShortcut "$DESKTOP\OpenDIY Desktop.lnk" "$DESKTOP\${PRODUCTNAME}.lnk"
  !insertmacro ArcveraMigrateShortcut "$SMPROGRAMS\OpenDIY Desktop.lnk" "$SMPROGRAMS\${PRODUCTNAME}.lnk"
!macroend
