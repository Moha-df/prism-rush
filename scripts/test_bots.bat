@echo off
REM Test automatique : serveur sans fenetre + 2 bots, puis un client bot qui le
REM trouve dans la liste publique et le rejoint par Internet (iroh).
cd /d "%~dp0.."
call "%~dp0ensure_build.bat" || exit /b 1
start "serveur" "%~dp0..\target\release\prism_rush.exe" server --headless --bots 2 --duration 130 --port 7790
"%~dp0..\target\release\prism_rush.exe" bot --duration 120
pause
