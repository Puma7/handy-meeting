@echo off
if not exist "%~dp0dist-0.4.1\HandyMeeting.exe" (
  echo Bitte zuerst build.ps1 ausfuehren.
  pause
  exit /b 1
)
start "" "%~dp0dist-0.4.1\HandyMeeting.exe"
