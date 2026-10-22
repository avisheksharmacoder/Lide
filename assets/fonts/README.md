# Lide IDE Typography & Font Assets

This directory contains configuration, scripts, and documentation for the typography system used across **Lide**.

---

## 1. Installed Font Families

| Role | Primary Font | Fallbacks | Source / License |
| :--- | :--- | :--- | :--- |
| **IDE / UI Font** | `Quicksand` | `Segoe UI, system-ui, sans-serif` | [Google Fonts / OFL](https://github.com/google/fonts/tree/main/ofl/quicksand) |
| **Code Editor Font** | `JetBrains Mono` | `Cascadia Code, Consolas, monospace` | [JetBrains / OFL](https://github.com/JetBrains/JetBrainsMono) |
| **Terminal Font** | `JetBrains Mono` | `Cascadia Code, Consolas, monospace` | [Microsoft / OFL](https://github.com/microsoft/cascadia-code) |

---

## 2. One-Click Windows Installation

To install all three fonts into your Windows user font directory (`%LOCALAPPDATA%\Microsoft\Windows\Fonts`) and register them:

```cmd
scripts\install_fonts.bat
```

Or via PowerShell:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\install_fonts.ps1
```

Once installed, Windows and Floem's `cosmic-text` engine will automatically discover and load the TrueType files directly.
