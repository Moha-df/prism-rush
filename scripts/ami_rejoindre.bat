@echo off
cd /d "%~dp0"
set ADDR=%1
if "%ADDR%"=="" set /p ADDR=Adresse de l'hote (ex: 100.101.102.103) : 
start "" "%~dp0prism_rush.exe" join %ADDR%
