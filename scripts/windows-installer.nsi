Unicode true
!include MUI2.nsh
!include x64.nsh
!include WinVer.nsh
!include LogicLib.nsh
!include FileFunc.nsh
!define PRODUCT "Meeting for Handy"
!define UNINSTALL_ROOT "Software\Microsoft\Windows\CurrentVersion\Uninstall"
!define UNINSTALL_KEY "${UNINSTALL_ROOT}\MeetingForHandy"
Name "${PRODUCT}"
OutFile "${OUTPUT_FILE}"
; One stable program folder. Updates and repairs replace only the program files listed by the
; installed uninstaller; recordings and settings live in %LOCALAPPDATA%\HandyMeeting.
InstallDir "$LOCALAPPDATA\Programs\${PRODUCT}"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetDatablockOptimize on
VIProductVersion "${NUMERIC_VERSION}"
VIAddVersionKey /LANG=1033 "ProductName" "${PRODUCT}"
VIAddVersionKey /LANG=1033 "FileDescription" "${PRODUCT} setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "${PRODUCT} contributors"
Var Upgrade
!define MUI_ICON "${APP_ICON}"
!define MUI_UNICON "${APP_ICON}"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TEXT "$(WelcomeText)"
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "$(LaunchText)"
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchProgram
!define MUI_FINISHPAGE_SHOWREADME
!define MUI_FINISHPAGE_SHOWREADME_TEXT "$(DesktopText)"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateDesktopShortcut
!define MUI_FINISHPAGE_LINK "$(HelpText)"
!define MUI_FINISHPAGE_LINK_LOCATION "https://github.com/Puma7/meeting-for-handy/blob/main/README.md"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${APP_LICENSE}"
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE CheckDestination
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "German"
LangString WelcomeText ${LANG_ENGLISH} "Setup installs ${PRODUCT} ${APP_VERSION} for this Windows user in one program folder and adds it to the Start menu.$\r$\n$\r$\nRunning setup again repairs or updates that installation. Recordings, transcripts and settings are stored separately and are always kept.$\r$\n$\r$\nRequired: Handy with a compatible streaming model, Microsoft WebView2 and the Microsoft Visual C++ x64 runtime."
LangString WelcomeText ${LANG_GERMAN} "Dieser Assistent installiert ${PRODUCT} ${APP_VERSION} für diesen Windows-Benutzer in einen Programmordner und legt einen Eintrag im Startmenü an.$\r$\n$\r$\nEin erneuter Aufruf repariert oder aktualisiert diese Installation. Aufnahmen, Transkripte und Einstellungen liegen getrennt davon und bleiben immer erhalten.$\r$\n$\r$\nVoraussetzungen: Handy mit einem kompatiblen Streaming-Modell, Microsoft WebView2 und die Microsoft Visual C++ x64 Runtime."
LangString LaunchText ${LANG_ENGLISH} "Start ${PRODUCT}"
LangString LaunchText ${LANG_GERMAN} "${PRODUCT} starten"
LangString DesktopText ${LANG_ENGLISH} "Create a desktop shortcut"
LangString DesktopText ${LANG_GERMAN} "Verknüpfung auf dem Desktop anlegen"
LangString HelpText ${LANG_ENGLISH} "Installation help"
LangString HelpText ${LANG_GERMAN} "Installationshilfe"
LangString Needs64Bit ${LANG_ENGLISH} "${PRODUCT} requires 64-bit Windows."
LangString Needs64Bit ${LANG_GERMAN} "${PRODUCT} benötigt ein 64-Bit-Windows."
LangString NeedsWin10 ${LANG_ENGLISH} "${PRODUCT} requires Windows 10 or newer."
LangString NeedsWin10 ${LANG_GERMAN} "${PRODUCT} benötigt Windows 10 oder neuer."
LangString FolderInUse ${LANG_ENGLISH} "This folder already contains other files. Choose an empty or new folder; existing files are never replaced."
LangString FolderInUse ${LANG_GERMAN} "Dieser Ordner enthält bereits andere Dateien. Bitte einen leeren oder neuen Ordner wählen; vorhandene Dateien werden nie ersetzt."
LangString FolderFailed ${LANG_ENGLISH} "The program folder could not be created. Existing files were preserved."
LangString FolderFailed ${LANG_GERMAN} "Der Programmordner konnte nicht angelegt werden. Vorhandene Dateien bleiben erhalten."
LangString ProgramRunning ${LANG_ENGLISH} "${PRODUCT} is still running. Close it (a running recording should be stopped first) and click Retry."
LangString ProgramRunning ${LANG_GERMAN} "${PRODUCT} läuft noch. Bitte die App schließen (eine laufende Aufnahme vorher beenden) und auf Wiederholen klicken."
LangString ReplaceFailed ${LANG_ENGLISH} "The previous program files could not be replaced. Nothing else was changed; run setup again after closing ${PRODUCT}."
LangString ReplaceFailed ${LANG_GERMAN} "Die bisherigen Programmdateien konnten nicht ersetzt werden. Sonst wurde nichts geändert; Setup nach dem Schließen von ${PRODUCT} erneut ausführen."
LangString LegacyRemoved ${LANG_ENGLISH} "Removed the former program folder"
LangString LegacyRemoved ${LANG_GERMAN} "Früheren Programmordner entfernt"
LangString MovedKept ${LANG_ENGLISH} "The previous installation is still running and was kept; remove it later with its Uninstall.exe"
LangString MovedKept ${LANG_GERMAN} "Die bisherige Installation läuft noch und bleibt erhalten; später über ihre Uninstall.exe entfernen"
LangString LegacyKept ${LANG_ENGLISH} "A former version is still running and was kept; remove it later in Windows Settings > Apps"
LangString LegacyKept ${LANG_GERMAN} "Eine frühere Version läuft noch und bleibt erhalten; später unter Windows-Einstellungen > Apps entfernen"
Function .onInit
  SetShellVarContext current
  SetRegView 64
  ; Offers English or German, preselected from the Windows display language.
  !insertmacro MUI_LANGDLL_DISPLAY
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "$(Needs64Bit)"
    Abort
  ${EndIf}
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP "$(NeedsWin10)"
    Abort
  ${EndIf}
FunctionEnd
Function un.onInit
  SetShellVarContext current
  SetRegView 64
FunctionEnd
; Waits up to 15 s until nothing holds the program file open; returns 1 when free, 0 when busy.
!macro WAIT_FOR_RELEASE UN
Function ${UN}WaitForRelease
  Exch $0
  Push $1
  Push $2
  StrCpy $2 0
  release_loop:
    IfFileExists "$0" 0 release_free
    System::Call 'kernel32::CreateFileW(w r0, i 0x40000000, i 0, p 0, i 3, i 0x80, p 0) i .r1'
    StrCmp $1 -1 release_busy
    System::Call 'kernel32::CloseHandle(i r1)'
    Goto release_free
  release_busy:
    IntOp $2 $2 + 1
    IntCmp $2 30 release_timeout
    Sleep 500
    Goto release_loop
  release_timeout:
    StrCpy $0 0
    Goto release_done
  release_free:
    StrCpy $0 1
  release_done:
  Pop $2
  Pop $1
  Exch $0
FunctionEnd
!macroend
!insertmacro WAIT_FOR_RELEASE ""
!insertmacro WAIT_FOR_RELEASE "un."
; Accepts the registered installation (repair or update) or a folder that does not exist yet.
Function CheckDestination
  StrCpy $Upgrade 0
  ReadRegStr $0 HKCU "${UNINSTALL_KEY}" "InstallLocation"
  StrCmp $0 "" check_new
  StrCmp $0 "$INSTDIR" 0 check_new
  IfFileExists "$INSTDIR\*.*" 0 check_new
  StrCpy $Upgrade 1
  Return
  check_new:
  ; Never interpret a failed directory enumeration as an empty destination.
  System::Call 'kernel32::GetFileAttributesW(w "$INSTDIR") i .r0 ?e'
  Pop $1
  StrCmp $0 -1 0 destination_rejected
  StrCmp $1 2 destination_available
  StrCmp $1 3 destination_available
  destination_rejected:
    IfSilent destination_rejected_silent
    MessageBox MB_ICONSTOP "$(FolderInUse)"
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
; Removes a previous installation in another folder and those of the former
; one-folder-per-version layout through their own uninstallers, which delete only their
; listed program files. User data is never touched.
Function RemoveFormerInstallations
  ReadRegStr $R3 HKCU "${UNINSTALL_KEY}" "InstallLocation"
  StrCmp $R3 "" legacy_scan
  StrCmp $R3 "$INSTDIR" legacy_scan
  IfFileExists "$R3\Uninstall.exe" 0 legacy_scan
  Push "$R3\HandyMeeting.exe"
  Call WaitForRelease
  Pop $R4
  StrCmp $R4 1 moved_remove
  ; Its registration is replaced by this installation, so Windows Apps cannot list it.
  DetailPrint "$(MovedKept): $R3"
  Goto legacy_scan
  moved_remove:
  ExecWait '"$R3\Uninstall.exe" /S _?=$R3' $R4
  Delete "$R3\Uninstall.exe"
  RMDir "$R3"
  DetailPrint "$(LegacyRemoved): $R3"
  legacy_scan:
  StrCpy $R0 0
  legacy_loop:
    EnumRegKey $R1 HKCU "${UNINSTALL_ROOT}" $R0
    StrCmp $R1 "" legacy_done
    StrCpy $R2 $R1 13
    StrCmp $R2 "HandyMeeting-" 0 legacy_next
    ReadRegStr $R3 HKCU "${UNINSTALL_ROOT}\$R1" "InstallLocation"
    StrCmp $R3 "" legacy_next
    StrCmp $R3 "$INSTDIR" legacy_next
    IfFileExists "$R3\Uninstall.exe" 0 legacy_next
    Push "$R3\HandyMeeting.exe"
    Call WaitForRelease
    Pop $R4
    StrCmp $R4 1 legacy_remove
    DetailPrint "$(LegacyKept): $R3"
    Goto legacy_next
    legacy_remove:
    ExecWait '"$R3\Uninstall.exe" /S _?=$R3' $R4
    ; Run in place, the former uninstaller cannot delete itself or its folder.
    Delete "$R3\Uninstall.exe"
    RMDir "$R3"
    ReadRegStr $R4 HKCU "${UNINSTALL_ROOT}\$R1" "InstallLocation"
    StrCmp $R4 "" 0 legacy_next
    DetailPrint "$(LegacyRemoved): $R3"
    ; The registration is gone, so enumeration restarts from the first key.
    StrCpy $R0 0
    Goto legacy_loop
  legacy_next:
    IntOp $R0 $R0 + 1
    Goto legacy_loop
  legacy_done:
  RMDir "$LOCALAPPDATA\Programs\HandyMeeting"
  ; The former Start menu folder link pointed into a version folder.
  Delete "$SMPROGRAMS\${PRODUCT}\${PRODUCT}.lnk"
  RMDir "$SMPROGRAMS\${PRODUCT}"
  DeleteRegKey HKCU "Software\HandyMeeting\BrandedStartMenuLink"
FunctionEnd
Function LaunchProgram
  Exec '"$INSTDIR\HandyMeeting.exe"'
FunctionEnd
Function CreateDesktopShortcut
  CreateShortCut "$DESKTOP\${PRODUCT}.lnk" "$INSTDIR\HandyMeeting.exe" "" "$INSTDIR\HandyMeeting.exe" 0
FunctionEnd
Section "${PRODUCT}" Main
  SetShellVarContext current
  SetRegView 64
  ; Silent runs skip the directory page, so the destination is checked here as well.
  Call CheckDestination
  StrCmp $Upgrade 1 replace_existing
  Call ReserveNewDirectory
  Pop $0
  StrCmp $0 1 payload
  IfSilent reservation_failed_silent
  MessageBox MB_ICONSTOP "$(FolderFailed)"
  reservation_failed_silent:
    SetErrorLevel 2
    Abort
  replace_existing:
  ; Never end a running app: wait for it to close, then let the installed uninstaller
  ; remove exactly its own program files.
  Push "$INSTDIR\HandyMeeting.exe"
  Call WaitForRelease
  Pop $0
  StrCmp $0 1 program_closed
  IfSilent program_running_silent
  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(ProgramRunning)" IDRETRY replace_existing
  program_running_silent:
    SetErrorLevel 3
    Abort
  program_closed:
  IfFileExists "$INSTDIR\Uninstall.exe" 0 payload
  ExecWait '"$INSTDIR\Uninstall.exe" /S /UPGRADE _?=$INSTDIR' $0
  StrCmp $0 0 payload
  IfSilent replace_failed_silent
  MessageBox MB_ICONSTOP "$(ReplaceFailed)"
  replace_failed_silent:
    SetErrorLevel 3
    Abort
  payload:
  SetOverwrite on
  !include "${INSTALL_FILES}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  Call RemoveFormerInstallations
  ; Created natively, so a restricted PowerShell execution policy cannot block it.
  CreateShortCut "$SMPROGRAMS\${PRODUCT}.lnk" "$INSTDIR\HandyMeeting.exe" "" "$INSTDIR\HandyMeeting.exe" 0
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "${PRODUCT} contributors"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\HandyMeeting.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" $0
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd
Section "Uninstall"
  ; /UPGRADE: a newer setup replaces the program files; links and registration stay.
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/UPGRADE" $R1
  IfErrors 0 +2
  StrCpy $R1 "remove"
  Push "$INSTDIR\HandyMeeting.exe"
  Call un.WaitForRelease
  Pop $0
  StrCmp $0 1 uninstall_files
  SetErrorLevel 3
  Quit
  uninstall_files:
  !include "${UNINSTALL_FILES}"
  IfFileExists "$INSTDIR\HandyMeeting.exe" 0 +3
  SetErrorLevel 3
  Quit
  StrCmp $R1 "remove" 0 uninstall_done
  Delete "$INSTDIR\Uninstall.exe"
  ; Links and registration of a different folder belong to the current installation and stay.
  ReadRegStr $0 HKCU "${UNINSTALL_KEY}" "InstallLocation"
  StrCmp $0 "" own_links
  StrCmp $0 "$INSTDIR" 0 keep_registration
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  own_links:
  Delete "$SMPROGRAMS\${PRODUCT}.lnk"
  Delete "$DESKTOP\${PRODUCT}.lnk"
  keep_registration:
  ; Non-recursive: unknown files and all user sessions are retained.
  RMDir "$INSTDIR"
  uninstall_done:
SectionEnd
