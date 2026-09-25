$ErrorActionPreference = "Stop"

$vcBin = "C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\VC\bin"
$sdkBin = "C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\bin"
$cmakeBin = "C:\Program Files\CMake\bin"
$rustBin = "C:\Users\Saisrikar Kokku\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin"

$env:PATH = "$cmakeBin;$vcBin;$sdkBin;$rustBin;" + $env:PATH
$env:RC = "$sdkBin\rc.exe"
$env:WHISPER_DONT_GENERATE_BINDINGS = "1"

$sdkLib = "C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\lib"
$vcLib = "C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\VC\lib"
$env:LIB = "$sdkLib;$vcLib;" + $env:LIB

$vcInclude = "C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\VC\include"
$sdkIncludeUm = "C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\include\um"
$sdkIncludeShared = "C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\include\shared"
$sdkIncludeUcrt = "C:\Program Files\Microsoft Visual Studio\2022\Community\SDK\ScopeCppSDK\vc15\SDK\include\ucrt"
$env:INCLUDE = "$vcInclude;$sdkIncludeUm;$sdkIncludeShared;$sdkIncludeUcrt;" + $env:INCLUDE

Set-Location "$PSScriptRoot\src-tauri"
cargo check
