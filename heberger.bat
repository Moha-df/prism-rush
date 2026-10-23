@echo off
REM Heberge une partie : tu joues et tes amis la voient dans "Parties ouvertes".
REM Optionnel : heberger.bat 2   -> ajoute 2 bots
cd /d "%~dp0"
call "%~dp0scripts\ensure_build.bat" || exit /b 1
set BOTS=%1
if "%BOTS%"=="" set BOTS=0
start "" "%~dp0target\release\prism_rush.exe" host --bots %BOTS%
