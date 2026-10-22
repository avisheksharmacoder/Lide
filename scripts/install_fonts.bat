@echo off
title Lide IDE Font Installer
echo Installing Quicksand, JetBrains Mono, and Cascadia Code...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install_fonts.ps1"
pause
