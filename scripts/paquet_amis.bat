@echo off
REM Cree dist\PrismRush.zip a envoyer a tes amis (ils n'ont pas besoin d'installer Rust).
cd /d "%~dp0.."
call "%~dp0ensure_build.bat" || exit /b 1
if exist dist rmdir /s /q dist
mkdir dist\PrismRush
copy target\release\prism_rush.exe dist\PrismRush\ >nul
copy scripts\ami_jouer.bat dist\PrismRush\jouer.bat >nul
copy scripts\ami_rejoindre.bat dist\PrismRush\rejoindre.bat >nul
copy scripts\installer_tailscale.bat dist\PrismRush\ >nul
copy scripts\LISEZMOI_AMIS.txt dist\PrismRush\LISEZMOI.txt >nul
powershell -NoProfile -Command "Compress-Archive -Path dist\PrismRush -DestinationPath dist\PrismRush.zip -Force"
echo.
echo Paquet cree : dist\PrismRush.zip  (envoie-le a tes amis)
pause
