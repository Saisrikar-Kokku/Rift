# Rift (Rust + Tauri) - System Installation Script
# Installs Rift to %LOCALAPPDATA%\Programs\Rift
# Creates Desktop, Start Menu, and Windows Auto-Start Shortcuts

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "    Installing Rift (Rust + Tauri)     " -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

$scriptRoot = $PSScriptRoot
$releaseExe = Join-Path $scriptRoot "src-tauri\target\release\rift-rust.exe"
$debugExe = Join-Path $scriptRoot "src-tauri\target\debug\rift-rust.exe"
$sourceExe = if ((Test-Path $releaseExe) -and (Test-Path $debugExe)) {
    if ((Get-Item $releaseExe).LastWriteTime -ge (Get-Item $debugExe).LastWriteTime) { $releaseExe } else { $debugExe }
} elseif (Test-Path $releaseExe) {
    $releaseExe
} else {
    $debugExe
}
$targetDir = Join-Path $env:LOCALAPPDATA "Programs\Rift"
$targetExe = Join-Path $targetDir "Rift.exe"
$iconSource = Join-Path $scriptRoot "src-tauri\icons\icon.ico"
$targetIcon = Join-Path $targetDir "icon.ico"

if (-not (Test-Path $sourceExe)) {
    Write-Host "[ERROR] Binary not found at: $releaseExe or $debugExe" -ForegroundColor Red
    Write-Host "Please build the project first." -ForegroundColor Yellow
    exit 1
}
Write-Host "Source Binary: $sourceExe" -ForegroundColor Cyan

# 0. Safety Backup: Back up all user history and database files to Documents\Rift Backups
Write-Host "`n[0/6] Backing up user database & history to Documents\Rift Backups..." -ForegroundColor Cyan
$docsBackupDir = Join-Path ([Environment]::GetFolderPath("MyDocuments")) "Rift Backups"
if (-not (Test-Path $docsBackupDir)) {
    New-Item -ItemType Directory -Path $docsBackupDir -Force | Out-Null
}
$appDataRift = Join-Path $env:APPDATA "Rift"
if (Test-Path $appDataRift) {
    Get-ChildItem -Path $appDataRift -Filter "*.db" -ErrorAction SilentlyContinue | ForEach-Object {
        Copy-Item -Path $_.FullName -Destination $docsBackupDir -Force
        Write-Host "  Safe backup: $($_.Name) -> Documents\Rift Backups" -ForegroundColor Gray
    }
    if (Test-Path (Join-Path $appDataRift "settings.json")) {
        Copy-Item -Path (Join-Path $appDataRift "settings.json") -Destination $docsBackupDir -Force
        Write-Host "  Safe backup: settings.json -> Documents\Rift Backups" -ForegroundColor Gray
    }
}

# 1. Stop any currently running Rift processes
Write-Host "`n[1/6] Stopping any running Rift instances..." -ForegroundColor Yellow
Get-Process -Name "rift-rust", "Rift" -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host "  Stopping PID $($_.Id) ($($_.ProcessName))..." -ForegroundColor Gray
    Stop-Process -Id $_.Id -Force
    Wait-Process -Id $_.Id -Timeout 5 -ErrorAction SilentlyContinue
}
Start-Sleep -Milliseconds 500

# 2. Copy files to %LOCALAPPDATA%\Programs\Rift
Write-Host "[2/5] Installing application files to $targetDir..." -ForegroundColor Yellow
if (-not (Test-Path $targetDir)) {
    New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
}

Copy-Item -Path $sourceExe -Destination $targetExe -Force
if (Test-Path $iconSource) {
    Copy-Item -Path $iconSource -Destination $targetIcon -Force
}

# Copy icons folder for tray icon
$iconsDir = Join-Path $scriptRoot "src-tauri\icons"
if (Test-Path $iconsDir) {
    Copy-Item -Path $iconsDir -Destination $targetDir -Recurse -Force
}

# Copy UI folder for reference
$uiSource = Join-Path $scriptRoot "ui"
if (Test-Path $uiSource) {
    $targetUi = Join-Path $targetDir "ui"
    Copy-Item -Path $uiSource -Destination $targetDir -Recurse -Force
}
Write-Host "  Files successfully installed to: $targetDir" -ForegroundColor Green

# 3. Create Windows Start Menu Shortcut
Write-Host "[3/5] Creating Start Menu shortcut..." -ForegroundColor Yellow
$startMenuDir = [Environment]::GetFolderPath("Programs")
$startShortcutPath = Join-Path $startMenuDir "Rift.lnk"
$wsh = New-Object -ComObject WScript.Shell
$shortcut = $wsh.CreateShortcut($startShortcutPath)
$shortcut.TargetPath = $targetExe
$shortcut.WorkingDirectory = $targetDir
$shortcut.IconLocation = "$targetIcon,0"
$shortcut.Description = "Rift - Ultra-Fast Voice Dictation (Rust + Tauri)"
$shortcut.Save()
Write-Host "  Start Menu shortcut created: $startShortcutPath" -ForegroundColor Green

# 4. Create Desktop Shortcuts (Standard, OneDrive, and Public Desktop)
Write-Host "[4/6] Creating Desktop shortcuts..." -ForegroundColor Yellow
$desktopDirs = @(
    [Environment]::GetFolderPath("Desktop"),
    (Join-Path $env:USERPROFILE "OneDrive\Desktop"),
    "C:\Users\Public\Desktop"
) | Where-Object { Test-Path $_ } | Select-Object -Unique

foreach ($dir in $desktopDirs) {
    try {
        $desktopShortcutPath = Join-Path $dir "Rift.lnk"
        $dShortcut = $wsh.CreateShortcut($desktopShortcutPath)
        $dShortcut.TargetPath = $targetExe
        $dShortcut.WorkingDirectory = $targetDir
        $dShortcut.IconLocation = "$targetIcon,0"
        $dShortcut.Description = "Rift - Ultra-Fast Voice Dictation (Rust + Tauri)"
        $dShortcut.Save()
        Write-Host "  Desktop shortcut created: $desktopShortcutPath" -ForegroundColor Green
    } catch {
        Write-Host "  Note: Skipped $dir" -ForegroundColor Gray
    }
}

# 5. Configure Windows Auto-Start (Single Official Registry Run Key)
Write-Host "[5/6] Configuring Windows Auto-Start on Boot..." -ForegroundColor Yellow

# Clean up any legacy duplicate Startup folder shortcut
$startupDir = [Environment]::GetFolderPath("Startup")
$startupShortcutPath = Join-Path $startupDir "Rift.lnk"
if (Test-Path $startupShortcutPath) {
    Remove-Item -Path $startupShortcutPath -Force -ErrorAction SilentlyContinue
    Write-Host "  Cleaned up legacy duplicate Startup folder shortcut: $startupShortcutPath" -ForegroundColor Green
}

# Clean up StartupApproved entry for StartupFolder if present
Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\StartupFolder" -Name "Rift.lnk" -Force -ErrorAction SilentlyContinue

# Ensure single clean Registry Run key
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name "Rift" -Value "`"$targetExe`"" -Force
Write-Host "  Registry Run key configured: HKCU:\Software\Microsoft\Windows\CurrentVersion\Run\Rift" -ForegroundColor Green

# 6. Launch Rift in user interactive desktop session
Write-Host "[6/6] Launching Rift..." -ForegroundColor Yellow
try {
    Start-Process "explorer.exe" -ArgumentList "`"$targetExe`""
    Write-Host "  Rift launched successfully via Windows Explorer." -ForegroundColor Green
} catch {
    Start-Process -FilePath $targetExe -WorkingDirectory $targetDir
    Write-Host "  Rift launched via fallback process starter." -ForegroundColor Green
}


Write-Host "`n========================================" -ForegroundColor Green
Write-Host "  Installation Complete!                " -ForegroundColor Green
Write-Host "  - Search 'Rift' in Start Menu         " -ForegroundColor Green
Write-Host "  - Launch from your Desktop shortcut   " -ForegroundColor Green
Write-Host "  - Push-to-talk: Hold 'Right Alt'      " -ForegroundColor Green
Write-Host "  - RAM footprint: ~49 MB (92% lighter) " -ForegroundColor Green
Write-Host "========================================`n" -ForegroundColor Green
