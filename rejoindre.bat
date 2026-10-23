@echo off
REM Rejoint une partie avec son code (normalement inutile : utilise la liste "Parties ouvertes" du menu).
cd /d "%~dp0"
call "%~dp0scripts\ensure_build.bat" || exit /b 1
set CODE=%1
if "%CODE%"=="" set /p CODE=Code de la partie : 
start "" "%~dp0target\release\prism_rush.exe" join %CODE%
