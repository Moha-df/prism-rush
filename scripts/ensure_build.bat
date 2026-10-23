@echo off
REM Compile le jeu en mode release si l'executable n'existe pas encore.
if exist "%~dp0..\target\release\prism_rush.exe" exit /b 0
where cargo >nul 2>nul
if errorlevel 1 (
  if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
  ) else (
    echo Rust n'est pas installe. Installe-le depuis https://rustup.rs puis relance.
    pause
    exit /b 1
  )
)
echo Premiere compilation du jeu (5 a 15 minutes, une seule fois)...
pushd "%~dp0.."
cargo build --release
set ERR=%ERRORLEVEL%
popd
if not "%ERR%"=="0" (
  echo La compilation a echoue.
  pause
  exit /b 1
)
exit /b 0
