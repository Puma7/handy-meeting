Unicode true
!include MUI2.nsh
!include x64.nsh
!include WinVer.nsh
!include LogicLib.nsh
Name "HandyMeeting ${APP_VERSION}"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\HandyMeeting\${APP_VERSION}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetDatablockOptimize on
VIProductVersion "${NUMERIC_VERSION}"
VIAddVersionKey /LANG=1033 "ProductName" "HandyMeeting"
VIAddVersionKey /LANG=1033 "FileDescription" "HandyMeeting installer"
VIAddVersionKey /LANG=1033 "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "HandyMeeting contributors"
!define MUI_ICON "${APP_ICON}"
!define MUI_UNICON "${APP_ICON}"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "Install HandyMeeting for this Windows user. Handy, a compatible streaming model, Microsoft WebView2 and the Microsoft Visual C++ x64 runtime are required. Existing recordings and older program versions are preserved."
!define MUI_FINISHPAGE_LINK "Installation help / Installationshilfe"
!define MUI_FINISHPAGE_LINK_LOCATION "https://github.com/Puma7/handy-meeting/blob/main/README.de.md"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${APP_LICENSE}"
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE CheckEmptyDirectory
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "German"
Function .onInit
  SetShellVarContext current
  SetRegView 64
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "HandyMeeting requires 64-bit Windows."
    Abort
  ${EndIf}
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP "HandyMeeting requires Windows 10 or newer."
    Abort
  ${EndIf}
FunctionEnd
Function CheckEmptyDirectory
  FindFirst $0 $1 "$INSTDIR\*"
  loop:
    StrCmp $1 "" empty
    StrCmp $1 "." next
    StrCmp $1 ".." next
    FindClose $0
    MessageBox MB_ICONSTOP "Choose an empty version folder. Existing programs and recordings will not be replaced."
    SetErrorLevel 2
    Abort
  next:
    FindNext $0 $1
    Goto loop
  empty:
    FindClose $0
FunctionEnd
Section "HandyMeeting" Main
  Call CheckEmptyDirectory
  SetShellVarContext current
  SetRegView 64
  !include "${INSTALL_FILES}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\HandyMeeting"
  CreateShortcut "$SMPROGRAMS\HandyMeeting\HandyMeeting ${APP_VERSION}.lnk" "$INSTDIR\HandyMeeting.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "DisplayName" "HandyMeeting ${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "Publisher" "HandyMeeting contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "DisplayIcon" "$INSTDIR\HandyMeeting.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "NoRepair" 1
SectionEnd
Section "Uninstall"
  SetShellVarContext current
  SetRegView 64
  !include "${UNINSTALL_FILES}"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\HandyMeeting\HandyMeeting ${APP_VERSION}.lnk"
  RMDir "$SMPROGRAMS\HandyMeeting"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}"
  ; Non-recursive: unknown files and all user sessions are retained.
  RMDir "$INSTDIR"
SectionEnd
