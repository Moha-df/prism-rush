@echo off
REM Test automatique : serveur sans fenetre + 3 bots pendant 2 minutes, puis un client bot reseau.
cd /d "%~dp0.."
call "%~dp0ensure_build.bat" || exit /b 1
start "serveur" "%~dp0..\target\release\prism_rush.exe" server --headless --bots 2 --duration 130 --port 7790
timeout /t 2 >nul
"%~dp0..\target\release\prism_rush.exe" bot 127.0.0.1 --port 7790 --duration 120
pause
