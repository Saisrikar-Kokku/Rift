@echo off
set "PATH=C:\Program Files\CMake\bin;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\VC\bin;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\bin;C:\Users\Saisrikar Kokku\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin;%PATH%"
set "LIB=C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\VC\lib;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\lib"
set "INCLUDE=C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\VC\include;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\include\um;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\include\shared;C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\include\ucrt"
set "RC=C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\bin\rc.exe"
set "PATH=C:\Users\Saisrikar Kokku\AppData\Local\Microsoft\WinGet\Packages\Ninja-build.Ninja_Microsoft.Winget.Source_8wekyb3d8bbwe;%PATH%"
set "WHISPER_DONT_GENERATE_BINDINGS=1"
set "CMAKE_GENERATOR=Ninja"
set "CARGO_INCREMENTAL=0"

cd /d "%~dp0"
cargo check
