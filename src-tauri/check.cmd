@echo off
set "PATH=C:\Program Files\CMake\bin;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\VC\bin;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\bin;C:\Users\Saisrikar Kokku\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin;%PATH%"
set "RC=C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\bin\rc.exe"
set "LIB=C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\lib;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\VC\lib;%LIB%"
set "INCLUDE=C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\VC\include;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\include\um;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\include\shared;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\include\ucrt;%INCLUDE%"
set "WHISPER_DONT_GENERATE_BINDINGS=1"

:retry
cargo check -j 1
if %ERRORLEVEL% equ 0 goto done
echo Transient lock detected, retrying in 2 seconds...
timeout /t 2 /nobreak >nul
goto retry

:done
echo Compilation check succeeded!
