@echo off
echo Starting Visual Studio Developer Command Prompt...
echo.

REM Try to find and run the Developer Command Prompt
if exist "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat" (
    call "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat"
) else if exist "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat" (
    call "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat"
) else if exist "C:\Program Files\Microsoft Visual Studio\2022\Professional\Common7\Tools\VsDevCmd.bat" (
    call "C:\Program Files\Microsoft Visual Studio\2022\Professional\Common7\Tools\VsDevCmd.bat"
) else (
    echo Visual Studio Developer Command Prompt not found!
    echo Please install Visual Studio with C++ tools and Windows SDK.
    pause
    exit /b 1
)

echo.
echo Visual Studio environment loaded.
echo.
echo Running cargo check...
cargo check --all-targets

echo.
echo If build succeeded, you can now run:
echo   cargo build --release
echo   cargo test
echo.
