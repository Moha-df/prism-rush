@echo off
REM Serveur dedie sans fenetre (personne ne joue sur ce PC). Ctrl+C pour arreter.
cd /d "%~dp0.."
call "%~dp0ensure_build.bat" || exit /b 1
"%~dp0..\target\release\prism_rush.exe" server --headless %*
