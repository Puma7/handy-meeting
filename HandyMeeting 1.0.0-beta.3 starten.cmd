@echo off
if not exist "%~dp0dist-rust-1.0.0-beta.3\HandyMeeting.exe" (
  echo Bitte das vollstaendige Rust-Beta-Paket entpacken.
  pause
  exit /b 1
)
start "" "%~dp0dist-rust-1.0.0-beta.3\HandyMeeting.exe"
