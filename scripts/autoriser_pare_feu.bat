@echo off
REM Autorise le port UDP 7777 dans le pare-feu Windows (a faire une fois, par l'hote).
REM Doit etre lance en administrateur : clic droit > Executer en tant qu'administrateur.
net session >nul 2>&1
if errorlevel 1 (
  echo Relance ce script en administrateur : clic droit ^> Executer en tant qu'administrateur.
  pause
  exit /b 1
)
netsh advfirewall firewall delete rule name="Prism Rush" >nul 2>&1
netsh advfirewall firewall add rule name="Prism Rush" dir=in action=allow protocol=UDP localport=7777
echo Regle ajoutee : UDP 7777 autorise en entree.
pause
