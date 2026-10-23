@echo off
REM Rejoint la partie d'un ami. Usage : rejoindre.bat 100.101.102.103
cd /d "%~dp0"
call "%~dp0scripts\ensure_build.bat" || exit /b 1
set ADDR=%1
if "%ADDR%"=="" set /p ADDR=Adresse de l'hote (ex: 100.101.102.103) : 
start "" "%~dp0target\release\prism_rush.exe" join %ADDR%
