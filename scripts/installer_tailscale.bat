@echo off
REM Installe Tailscale (VPN gratuit) pour jouer en ligne sans ouvrir de port.
echo Installation de Tailscale via winget...
winget install --id Tailscale.Tailscale -e --accept-source-agreements --accept-package-agreements
if errorlevel 1 (
  echo.
  echo winget a echoue : telecharge Tailscale ici : https://tailscale.com/download/windows
  start https://tailscale.com/download/windows
)
echo.
echo Ensuite : ouvre Tailscale (icone pres de l'horloge) et connecte-toi (compte Google, Microsoft ou GitHub).
pause
