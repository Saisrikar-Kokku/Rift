# Rift (Rust + Tauri) - Uninstaller Script
$ErrorActionPreference = "SilentlyContinue"

Write-Host "Uninstalling Rift..." -ForegroundColor Yellow

# 1. Stop running processes
Get-Process -Name "rift-rust", "Rift" | Stop-Process -Force

# 2. Remove Shortcuts
$startMenuDir = [Environment]::GetFolderPath("Programs")
$startShortcutPath = Join-Path $startMenuDir "Rift.lnk"
if (Test-Path $startShortcutPath) { Remove-Item $startShortcutPath -Force }

$desktopDir = [Environment]::GetFolderPath("Desktop")
$desktopShortcutPath = Join-Path $desktopDir "Rift.lnk"
if (Test-Path $desktopShortcutPath) { Remove-Item $desktopShortcutPath -Force }

$startupDir = [Environment]::GetFolderPath("Startup")
$startupShortcutPath = Join-Path $startupDir "Rift.lnk"
if (Test-Path $startupShortcutPath) { Remove-Item $startupShortcutPath -Force }

# 3. Remove Program Files in LocalAppData
$targetDir = Join-Path $env:LOCALAPPDATA "Programs\Rift"
if (Test-Path $targetDir) {
    Remove-Item -Path $targetDir -Recurse -Force
}

Write-Host "Rift uninstalled successfully." -ForegroundColor Green
