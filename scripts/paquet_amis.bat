@echo off
REM Cree dist\PrismRush.zip a envoyer a tes amis (ils n'ont rien a installer).
cd /d "%~dp0.."
call "%~dp0ensure_build.bat" || exit /b 1
if exist dist rmdir /s /q dist
mkdir dist\PrismRush
copy target\release\prism_rush.exe dist\PrismRush\ >nul
copy scripts\ami_jouer.bat "dist\PrismRush\Jouer.bat" >nul
copy scripts\LISEZMOI_AMIS.txt dist\PrismRush\LISEZMOI.txt >nul
powershell -NoProfile -Command "Compress-Archive -Path dist\PrismRush -DestinationPath dist\PrismRush.zip -Force"
echo.
echo Paquet cree : dist\PrismRush.zip  (envoie-le a tes amis)
pause
