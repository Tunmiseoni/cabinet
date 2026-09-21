@echo off
rem fcade-lan-windows - direct-connect FightCade FBNeo over Tailscale (Windows)
rem Bypasses FightCade matchmaking (and therefore CGNAT) by using the emulator's
rem built-in quark:direct mode. Friend-only. Double-click to run.
setlocal EnableDelayedExpansion
title FightCade LAN (direct over Tailscale)

rem --- locate the FightCade install -------------------------------------------
rem Override with: set FC_DIR=C:\path\to\Fightcade
if not defined FC_DIR set "FC_DIR=%APPDATA%\Fightcade"
if not exist "%FC_DIR%\emulator\fbneo\fcadefbneo.exe" call :detect
set "FB_DIR=%FC_DIR%\emulator\fbneo"
set "EMU=%FB_DIR%\fcadefbneo.exe"

if not exist "%EMU%" (
  echo.
  echo ERROR: fcadefbneo.exe not found.
  echo   Looked in: %FB_DIR%
  echo.
  echo Set the install directory and retry, e.g.:
  echo   set FC_DIR=C:\Users\you\AppData\Roaming\Fightcade
  echo   fcade-lan-windows.bat
  goto :fail
)

rem --- locate Tailscale --------------------------------------------------------
set "TS="
where tailscale >nul 2>nul && set "TS=tailscale"
if not defined TS if exist "%ProgramFiles%\Tailscale\tailscale.exe" set "TS=%ProgramFiles%\Tailscale\tailscale.exe"
if not defined TS if exist "%ProgramFiles(x86)%\Tailscale\tailscale.exe" set "TS=%ProgramFiles(x86)%\Tailscale\tailscale.exe"

set "MY_IP="
if defined TS for /f "usebackq delims=" %%i in (`"%TS%" ip -4 2^>nul`) do if not defined MY_IP set "MY_IP=%%i"

if defined MY_IP (
  echo Local Tailscale IP: %MY_IP%
) else (
  echo.
  echo WARNING: Tailscale is not running (or not found^).
  echo The peer cannot be reached until Tailscale is up and logged in.
  echo.
)

rem --- peer -------------------------------------------------------------------
set "PEER_DEFAULT=100.64.0.1"
set "PEER_IP="
set /p "PEER_IP=Peer Tailscale IP (your friend) [%PEER_DEFAULT%]: "
if not defined PEER_IP set "PEER_IP=%PEER_DEFAULT%"

rem --- ROM --------------------------------------------------------------------
echo.
echo ROMs:
if not exist "%FB_DIR%\ROMs\*.zip" goto :no_roms
set /a ROM_COUNT=0
for %%f in ("%FB_DIR%\ROMs\*.zip") do (
  set /a ROM_COUNT+=1
  set "ROM_!ROM_COUNT!=%%~nf"
  echo    !ROM_COUNT!. %%~nf
)
echo.
set "PICK="
set /p "PICK=Choose ROM number [1]: "
if not defined PICK set "PICK=1"
for /f "delims=" %%p in ("!PICK!") do set "ROM=!ROM_%%p!"
if not defined ROM (
  echo Invalid ROM selection.
  goto :fail
)
goto :side

:no_roms
echo   (none found in %FB_DIR%\ROMs)
set "ROM="
set /p "ROM=ROM short name [sfiii3nr1]: "
if not defined ROM set "ROM=sfiii3nr1"

rem --- side -------------------------------------------------------------------
:side
echo.
set "SIDE_LABEL="
set /p "SIDE_LABEL=Your side P1/P2 [P1]: "
if not defined SIDE_LABEL set "SIDE_LABEL=P1"
if /i "!SIDE_LABEL!"=="P2" (set "SIDE=1") else (set "SIDE=0")
if "!SIDE!"=="0" (set "LOCAL=7001" & set "PEER=7000") else (set "LOCAL=7000" & set "PEER=7001")

rem --- launch -----------------------------------------------------------------
cd /d "%FB_DIR%" || goto :fail
set "ARG=quark:direct,!ROM!,!LOCAL!,!PEER_IP!,!PEER!,!SIDE!,0"
echo.
echo Local !MY_IP!  ^|  Peer !PEER_IP!  ^|  side !SIDE!  ^|  ROM !ROM!
echo Exec: fcadefbneo.exe "!ARG!" -w
echo.
"%EMU%" "!ARG!" -w
set "RC=%ERRORLEVEL%"
echo.
echo Emulator exited (code %RC%).
pause
exit /b %RC%

:detect
for %%d in ("%APPDATA%\Fightcade" "%USERPROFILE%\Fightcade" "%LOCALAPPDATA%\Fightcade" "C:\Fightcade" "%USERPROFILE%\Fightcade2" "%USERPROFILE%\Documents\GGs\Fightcade") do (
  if exist "%%~d\emulator\fbneo\fcadefbneo.exe" set "FC_DIR=%%~d"
)
exit /b

:fail
echo.
pause
exit /b 1
