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

Write-Host "Building Rift release binary (-j 1)..." -ForegroundColor Cyan
Set-Location "$PSScriptRoot\src-tauri"

$maxAttempts = 5
for ($attempt = 1; $attempt -le $maxAttempts; $attempt++) {
    Write-Host "Executing build (attempt $attempt of $maxAttempts)..." -ForegroundColor Cyan
    & cargo build --release -j 1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Build completed successfully!" -ForegroundColor Green
        exit 0
    }
    Write-Warning "Build encountered temporary file lock. Retrying in 2 seconds..."
    Start-Sleep -Seconds 2
}

exit 1
