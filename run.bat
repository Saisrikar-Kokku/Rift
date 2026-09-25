@echo off
title Rift (Rust + Tauri)
cd /d "%~dp0\src-tauri"
start "" "target\debug\rift-rust.exe"
echo Rift (Rust + Tauri) started!
