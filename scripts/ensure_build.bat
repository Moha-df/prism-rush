@echo off
REM Compile (ou met a jour) le jeu en mode release. Si rien n'a change, c'est instantane.
REM Si Rust n'est pas installe mais que l'executable existe deja, on le lance tel quel.
where cargo >nul 2>nul
if errorlevel 1 (
  if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
  ) else (
    if exist "%~dp0..\target\release\prism_rush.exe" exit /b 0
    echo Rust n'est pas installe. Installe-le depuis https://rustup.rs puis relance.
    pause
    exit /b 1
  )
)
echo Verification / compilation du jeu (la premiere fois : 5 a 15 minutes)...
pushd "%~dp0.."
cargo build --release
set ERR=%ERRORLEVEL%
popd
if not "%ERR%"=="0" (
  echo La compilation a echoue. Le jeu est-il encore ouvert ? Ferme-le et relance.
  pause
  exit /b 1
)
exit /b 0
