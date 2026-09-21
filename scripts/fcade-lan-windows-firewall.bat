@echo off
rem fcade-lan-windows-firewall - allow inbound traffic for the direct-connect emulator
rem and the RetroArch netplay host. Run once. Self-elevates via UAC. Safe to re-run
rem (rules are recreated).
setlocal
title FightCade LAN firewall helper

rem --- self-elevate if not already admin ---------------------------------------
net session >nul 2>&1
if %errorlevel% neq 0 (
  echo Requesting administrator privileges...
  powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
  exit /b
)

rem --- RetroArch netplay host (inbound TCP 55435) ------------------------------
call :ra_rule

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

:ra_rule
set "RA_EXE="
if defined RETROARCH if exist "%RETROARCH%" set "RA_EXE=%RETROARCH%"
if not defined RA_EXE call :detect_ra
if not defined RA_EXE (
  echo.
  echo RetroArch not found; skipping its netplay firewall rule.
  echo Set RETROARCH to retroarch.exe and re-run to allow host mode.
  exit /b
)
echo Allowing inbound netplay traffic for:
echo   %RA_EXE%
netsh advfirewall firewall delete rule name="RetroArch Netplay LAN" >nul 2>&1
netsh advfirewall firewall add rule name="RetroArch Netplay LAN" dir=in action=allow program="%RA_EXE%" protocol=TCP localport=55435 enable=yes profile=any
if %errorlevel% equ 0 (
  echo.
  echo Done. The rule "RetroArch Netplay LAN" is active on TCP 55435.
) else (
  echo.
  echo Failed to add the RetroArch firewall rule.
)
exit /b

:detect_ra
for %%d in ("%ProgramFiles%\RetroArch" "%ProgramFiles(x86)%\RetroArch" "%LOCALAPPDATA%\RetroArch" "%APPDATA%\RetroArch" "%ProgramFiles(x86)%\Steam\steamapps\common\RetroArch" "%ProgramFiles%\Steam\steamapps\common\RetroArch") do (
  if exist "%%~d\retroarch.exe" set "RA_EXE=%%~d\retroarch.exe"
)
if not defined RA_EXE for /f "delims=" %%p in ('where retroarch.exe 2^>nul') do if not defined RA_EXE set "RA_EXE=%%p"
exit /b
