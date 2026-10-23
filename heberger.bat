@echo off
REM Heberge une partie : tu joues et tes amis peuvent te rejoindre.
REM Optionnel : heberger.bat 2   -> ajoute 2 bots
cd /d "%~dp0"
call "%~dp0scripts\ensure_build.bat" || exit /b 1
set BOTS=%1
if "%BOTS%"=="" set BOTS=0
echo.
echo  Partie hebergee sur le port UDP 7777.
echo  L'adresse a donner a tes amis s'affiche dans le salon (IP Tailscale 100.x.y.z).
echo.
start "" "%~dp0target\release\prism_rush.exe" host --bots %BOTS%
