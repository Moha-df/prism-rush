@echo off
REM Lance Prism Rush (menu principal). Compile le jeu au premier lancement.
cd /d "%~dp0"
call "%~dp0scripts\ensure_build.bat" || exit /b 1
start "" "%~dp0target\release\prism_rush.exe"
