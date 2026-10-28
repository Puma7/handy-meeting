Unicode true
!include MUI2.nsh
!include x64.nsh
!include WinVer.nsh
!include LogicLib.nsh
!include FileFunc.nsh
Name "Meeting for Handy"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\HandyMeeting\${APP_VERSION}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetDatablockOptimize on
VIProductVersion "${NUMERIC_VERSION}"
VIAddVersionKey /LANG=1033 "ProductName" "Meeting for Handy"
VIAddVersionKey /LANG=1033 "FileDescription" "Meeting for Handy installer"
VIAddVersionKey /LANG=1033 "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "Meeting for Handy contributors"
!define MUI_ICON "${APP_ICON}"
!define MUI_UNICON "${APP_ICON}"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "Install Meeting for Handy for this Windows user. Handy, a compatible streaming model, Microsoft WebView2 and the Microsoft Visual C++ x64 runtime are required. Existing recordings and older program versions are preserved."
!define MUI_FINISHPAGE_LINK "Installation help / Installationshilfe"
!define MUI_FINISHPAGE_LINK_LOCATION "https://github.com/Puma7/meeting-for-handy/blob/main/README.md"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${APP_LICENSE}"
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE CheckNewDirectory
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
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "InstallLocation"
  StrCmp $0 "" version_available
  IfSilent existing_version_silent
  MessageBox MB_ICONSTOP "This version is already installed. Uninstall it through Windows Apps first, or choose a different program version. Your recordings will be retained."
  existing_version_silent:
    SetErrorLevel 2
    Abort
  version_available:
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Meeting for Handy requires 64-bit Windows."
    Abort
  ${EndIf}
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP "Meeting for Handy requires Windows 10 or newer."
    Abort
  ${EndIf}
FunctionEnd
Function CheckNewDirectory
  ; Never interpret a failed directory enumeration as an empty destination.
  System::Call 'kernel32::GetFileAttributesW(w "$INSTDIR") i .r0 ?e'
  Pop $1
  StrCmp $0 -1 0 destination_rejected
  StrCmp $1 2 destination_available
  StrCmp $1 3 destination_available
  destination_rejected:
    IfSilent destination_rejected_silent
    MessageBox MB_ICONSTOP "Choose a new version folder. Existing or inaccessible destinations will not be replaced."
  destination_rejected_silent:
    SetErrorLevel 2
    Abort
  destination_available:
FunctionEnd
Function ReserveNewDirectory
  ; Parse the parent without requiring any of the destination folders to exist.
  ${GetParent} "$INSTDIR" $2
  ClearErrors
  CreateDirectory "$2"
  IfErrors reserve_failed
  ; Reserve the final directory exclusively, including against a concurrent setup.
  System::Call 'kernel32::CreateDirectoryW(w "$INSTDIR", p 0) i .r0'
  StrCmp $0 0 reserve_failed
  Push 1
  Return
  reserve_failed:
  Push 0
FunctionEnd
Section "Meeting for Handy" Main
  Call CheckNewDirectory
  SetShellVarContext current
  SetRegView 64
  Call ReserveNewDirectory
  Pop $0
  StrCmp $0 0 reservation_failed destination_reserved
  reservation_failed:
    IfSilent reservation_failed_silent
    MessageBox MB_ICONSTOP "The new program directory could not be reserved. Existing files were preserved."
  reservation_failed_silent:
    SetErrorLevel 2
    Abort
  destination_reserved:
  SetOverwrite off
  !include "${INSTALL_FILES}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -File "$INSTDIR\ManageStartMenu.ps1" -Mode install -InstallDirectory "$INSTDIR" -Version "${APP_VERSION}"'
  Pop $0
  Pop $1
  StrCmp $0 "0" shortcut_ready
  StrCmp $0 "4" 0 shortcut_blocked
  DetailPrint "The Meeting for Handy Start menu link keeps opening the newer installed version. This version remains available in its program folder."
  Goto shortcut_ready
  shortcut_blocked:
  DetailPrint "Existing/custom Start menu links were preserved, or shortcut creation was blocked. HandyMeeting.exe remains available in the new program folder."
  shortcut_ready:
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "DisplayName" "Meeting for Handy ${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "Publisher" "Meeting for Handy contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "DisplayIcon" "$INSTDIR\HandyMeeting.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "NoRepair" 1
SectionEnd
Section "Uninstall"
  SetShellVarContext current
  SetRegView 64
  ; An owned link to this version moves to the newest remaining registered installation.
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -File "$INSTDIR\ManageStartMenu.ps1" -Mode uninstall -InstallDirectory "$INSTDIR"'
  Pop $0
  Pop $1
  !include "${UNINSTALL_FILES}"
  Delete "$INSTDIR\Uninstall.exe"
  ; A different installation must retain its shared registration and shortcut.
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}" "InstallLocation"
  StrCmp $0 "$INSTDIR" 0 keep_registration
  RMDir "$SMPROGRAMS\Meeting for Handy"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\HandyMeeting-${APP_VERSION}"
  keep_registration:
  ; Non-recursive: unknown files and all user sessions are retained.
  RMDir "$INSTDIR"
SectionEnd
