@echo off
title Rift (Rust + Tauri)
cd /d "%~dp0\src-tauri"
if exist "target\release\rift-rust.exe" (
    start "" "target\release\rift-rust.exe"
) else (
    start "" "target\debug\rift-rust.exe"
)
echo Rift (Rust + Tauri) started!
