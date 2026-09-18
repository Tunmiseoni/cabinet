@echo off
rem fcade-lan-windows-firewall - allow inbound UDP for the direct-connect emulator.
rem Run once. Self-elevates via UAC. Safe to re-run (rule is recreated).
setlocal
title FightCade LAN firewall helper

rem --- self-elevate if not already admin ---------------------------------------
net session >nul 2>&1
if %errorlevel% neq 0 (
  echo Requesting administrator privileges...
  powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
  exit /b
)

rem --- locate the emulator (mirrors fcade-lan-windows.bat) ---------------------
if not defined FC_DIR set "FC_DIR=%APPDATA%\Fightcade"
if not exist "%FC_DIR%\emulator\fbneo\fcadefbneo.exe" call :detect
set "EMU=%FC_DIR%\emulator\fbneo\fcadefbneo.exe"

if not exist "%EMU%" (
  echo ERROR: fcadefbneo.exe not found in "%FC_DIR%\emulator\fbneo".
  echo Set FC_DIR and re-run, e.g.:
  echo   fcade-lan-windows-firewall.bat
  goto :done
)

echo Allowing inbound traffic for:
echo   %EMU%
netsh advfirewall firewall delete rule name="FightCade FBNeo LAN" >nul 2>&1
netsh advfirewall firewall add rule name="FightCade FBNeo LAN" dir=in action=allow program="%EMU%" enable=yes profile=any
if %errorlevel% equ 0 (
  echo.
  echo Done. The rule "FightCade FBNeo LAN" is active.
) else (
  echo.
  echo Failed to add the firewall rule.
)

:done
echo.
pause
exit /b

:detect
for %%d in ("%APPDATA%\Fightcade" "%USERPROFILE%\Fightcade" "%LOCALAPPDATA%\Fightcade" "C:\Fightcade" "%USERPROFILE%\Fightcade2" "%USERPROFILE%\Documents\GGs\Fightcade") do (
  if exist "%%~d\emulator\fbneo\fcadefbneo.exe" set "FC_DIR=%%~d"
)
exit /b
