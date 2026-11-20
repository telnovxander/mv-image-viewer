; Installer hooks of Spiegelmond (IV-10, projects/IV/tasks/IV-10.md in the MV repository), on top of
; the template nsis/installer.nsi (FM-11). Everything for the current user (HKCU), no UAC:
; - "Open with" for every format Spiegelmond shows (the list is the one of iv-core formats.rs, a
;   test keeps them equal) and an entry in Settings -> Apps -> Default apps. Windows never lets a
;   program make itself the default silently; Alex chooses there;
; - `iv` in the user PATH when the finish page box is ticked, or with /PATH on a silent install;
; - the uninstaller removes all of that, the PATH entry and the thumbnail / Vollmond cache
;   (%LOCALAPPDATA%\Spiegelmond\cache), never the settings (unless ticked) or the journal.

!define IV_PROGID "Spiegelmond.Image"
!define IV_CAPS "Software\Moonshine Vibecoder\Spiegelmond\Capabilities"
!define IV_COMMAND "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"%1$\""
!define IV_ICON "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"

!macro IvAssociateExt EXT
  WriteRegStr HKCU "Software\Classes\.${EXT}\OpenWithProgids" "${IV_PROGID}" ""
  WriteRegStr HKCU "Software\Classes\Applications\${MAINBINARYNAME}.exe\SupportedTypes" ".${EXT}" ""
  WriteRegStr HKCU "${IV_CAPS}\FileAssociations" ".${EXT}" "${IV_PROGID}"
!macroend

!macro IvUnassociateExt EXT
  DeleteRegValue HKCU "Software\Classes\.${EXT}\OpenWithProgids" "${IV_PROGID}"
  DeleteRegKey /ifempty HKCU "Software\Classes\.${EXT}\OpenWithProgids"
  DeleteRegKey /ifempty HKCU "Software\Classes\.${EXT}"
!macroend

!macro IvAssociateAll
  !insertmacro IvAssociateExt "jpg"
  !insertmacro IvAssociateExt "jpeg"
  !insertmacro IvAssociateExt "jfif"
  !insertmacro IvAssociateExt "png"
  !insertmacro IvAssociateExt "gif"
  !insertmacro IvAssociateExt "webp"
  !insertmacro IvAssociateExt "bmp"
  !insertmacro IvAssociateExt "avif"
  !insertmacro IvAssociateExt "tif"
  !insertmacro IvAssociateExt "tiff"
  !insertmacro IvAssociateExt "heic"
  !insertmacro IvAssociateExt "heif"
  !insertmacro IvAssociateExt "hif"
  !insertmacro IvAssociateExt "dng"
  !insertmacro IvAssociateExt "cr2"
  !insertmacro IvAssociateExt "nef"
  !insertmacro IvAssociateExt "nrw"
  !insertmacro IvAssociateExt "arw"
  !insertmacro IvAssociateExt "srw"
  !insertmacro IvAssociateExt "orf"
  !insertmacro IvAssociateExt "rw2"
  !insertmacro IvAssociateExt "pef"
  !insertmacro IvAssociateExt "erf"
  !insertmacro IvAssociateExt "3fr"
  !insertmacro IvAssociateExt "cr3"
!macroend

!macro IvUnassociateAll
  !insertmacro IvUnassociateExt "jpg"
  !insertmacro IvUnassociateExt "jpeg"
  !insertmacro IvUnassociateExt "jfif"
  !insertmacro IvUnassociateExt "png"
  !insertmacro IvUnassociateExt "gif"
  !insertmacro IvUnassociateExt "webp"
  !insertmacro IvUnassociateExt "bmp"
  !insertmacro IvUnassociateExt "avif"
  !insertmacro IvUnassociateExt "tif"
  !insertmacro IvUnassociateExt "tiff"
  !insertmacro IvUnassociateExt "heic"
  !insertmacro IvUnassociateExt "heif"
  !insertmacro IvUnassociateExt "hif"
  !insertmacro IvUnassociateExt "dng"
  !insertmacro IvUnassociateExt "cr2"
  !insertmacro IvUnassociateExt "nef"
  !insertmacro IvUnassociateExt "nrw"
  !insertmacro IvUnassociateExt "arw"
  !insertmacro IvUnassociateExt "srw"
  !insertmacro IvUnassociateExt "orf"
  !insertmacro IvUnassociateExt "rw2"
  !insertmacro IvUnassociateExt "pef"
  !insertmacro IvUnassociateExt "erf"
  !insertmacro IvUnassociateExt "3fr"
  !insertmacro IvUnassociateExt "cr3"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; The kind of file "Spiegelmond.Image": its icon and how it opens.
  WriteRegStr HKCU "Software\Classes\${IV_PROGID}" "" "Spiegelmond"
  WriteRegStr HKCU "Software\Classes\${IV_PROGID}\DefaultIcon" "" "${IV_ICON}"
  WriteRegStr HKCU "Software\Classes\${IV_PROGID}\shell\open\command" "" "${IV_COMMAND}"
  ; The program itself, for "Open with -> Choose another app".
  WriteRegStr HKCU "Software\Classes\Applications\${MAINBINARYNAME}.exe" "FriendlyAppName" "Spiegelmond"
  WriteRegStr HKCU "Software\Classes\Applications\${MAINBINARYNAME}.exe\DefaultIcon" "" "${IV_ICON}"
  WriteRegStr HKCU "Software\Classes\Applications\${MAINBINARYNAME}.exe\shell\open\command" "" "${IV_COMMAND}"
  ; Settings -> Apps -> Default apps -> Spiegelmond.
  WriteRegStr HKCU "${IV_CAPS}" "ApplicationName" "Spiegelmond"
  WriteRegStr HKCU "${IV_CAPS}" "ApplicationDescription" "${PRODUCTNAME}, an image viewer"
  WriteRegStr HKCU "${IV_CAPS}" "ApplicationIcon" "${IV_ICON}"
  WriteRegStr HKCU "Software\RegisteredApplications" "Spiegelmond" "${IV_CAPS}"
  !insertmacro IvAssociateAll
  ; Tell Explorer the associations changed (SHCNE_ASSOCCHANGED).
  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)"
  ; A silent install adds `iv` to PATH only when asked: setup.exe /S /PATH
  ${GetOptions} $CMDLINE "/PATH" $R0
  ${IfNot} ${Errors}
    Call IvAddToPath
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    !insertmacro IvUnassociateAll
    DeleteRegKey HKCU "Software\Classes\${IV_PROGID}"
    DeleteRegKey HKCU "Software\Classes\Applications\${MAINBINARYNAME}.exe"
    DeleteRegValue HKCU "Software\RegisteredApplications" "Spiegelmond"
    DeleteRegKey HKCU "${IV_CAPS}"
    DeleteRegKey /ifempty HKCU "Software\Moonshine Vibecoder\Spiegelmond"
    DeleteRegKey /ifempty HKCU "Software\Moonshine Vibecoder"
    System::Call "shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)"
    Call un.IvRemoveFromPath
    ; Thumbnails and the Vollmond index: rebuilt when needed. Settings and journal stay.
    SetShellVarContext current
    RMDir /r "$LOCALAPPDATA\${PRODUCTNAME}\cache"
    RMDir "$LOCALAPPDATA\${PRODUCTNAME}"
  ${EndIf}
!macroend

; `iv` in the PATH of the current user. NSIS strings are at most ${NSIS_MAX_STRLEN} characters:
; a longer PATH would come back cut, so it is never written then (a message says so).
Function IvAddToPath
  ClearErrors
  ReadRegStr $0 HKCU "Environment" "Path"
  StrLen $1 $0
  StrLen $2 $INSTDIR
  IntOp $1 $1 + $2
  IntOp $1 $1 + 2
  ${If} $1 >= ${NSIS_MAX_STRLEN}
    ${IfNot} ${Silent}
      MessageBox MB_ICONINFORMATION|MB_OK "$(ivPathTooLong)"
    ${EndIf}
    Return
  ${EndIf}
  ${StrLoc} $3 "$0;" "$INSTDIR;" ">"
  ${If} $3 != ""
    Return ; already there
  ${EndIf}
  ${If} $0 == ""
    WriteRegExpandStr HKCU "Environment" "Path" "$INSTDIR"
  ${Else}
    WriteRegExpandStr HKCU "Environment" "Path" "$0;$INSTDIR"
  ${EndIf}
  WriteRegDWORD HKCU "Software\Moonshine Vibecoder\Spiegelmond" "AddedToPath" 1
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
FunctionEnd

; Remove the entry IvAddToPath made (and only it); a PATH too long to read whole is left alone.
Function un.IvRemoveFromPath
  ReadRegStr $0 HKCU "Environment" "Path"
  StrLen $1 $0
  IntOp $1 $1 + 1
  ${If} $1 >= ${NSIS_MAX_STRLEN}
    Return
  ${EndIf}
  StrCpy $1 ";$0;"
  ${WordReplace} "$1" ";$INSTDIR;" ";" "+" $1
  ; Back without the guard semicolons added above.
  StrCpy $1 $1 "" 1
  StrCpy $1 $1 -1
  ${If} $1 != $0
    ${If} $1 == ""
      DeleteRegValue HKCU "Environment" "Path"
    ${Else}
      WriteRegExpandStr HKCU "Environment" "Path" "$1"
    ${EndIf}
    SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
  ${EndIf}
FunctionEnd
