<#
.SYNOPSIS
    Downloads and installs Quicksand, JetBrains Mono, and Cascadia Code TrueType fonts on Windows.
.DESCRIPTION
    Installs fonts into user font directory ($env:LOCALAPPDATA\Microsoft\Windows\Fonts)
    and registers them in the current user's registry (HKCU\Software\Microsoft\Windows NT\CurrentVersion\Fonts).
#>

$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "  Lide IDE Font Installer: Quicksand, JetBrains Mono, Cascadia Code" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

$userFontsDir = Join-Path $env:LOCALAPPDATA "Microsoft\Windows\Fonts"
if (-not (Test-Path $userFontsDir)) {
    New-Item -ItemType Directory -Path $userFontsDir -Force | Out-Null
}

$assetsFontsDir = Join-Path $PSScriptRoot "..\assets\fonts"
if (-not (Test-Path $assetsFontsDir)) {
    New-Item -ItemType Directory -Path $assetsFontsDir -Force | Out-Null
}

$fontSources = @(
    @{
        Name = "Quicksand"
        FileName = "Quicksand.ttf"
        RegistryName = "Quicksand (TrueType)"
        Url = "https://raw.githubusercontent.com/google/fonts/main/ofl/quicksand/Quicksand%5Bwght%5D.ttf"
    },
    @{
        Name = "JetBrainsMono-Regular"
        FileName = "JetBrainsMono-Regular.ttf"
        RegistryName = "JetBrains Mono Regular (TrueType)"
        Url = "https://raw.githubusercontent.com/JetBrains/JetBrainsMono/master/fonts/ttf/JetBrainsMono-Regular.ttf"
    },
    @{
        Name = "JetBrainsMono-Bold"
        FileName = "JetBrainsMono-Bold.ttf"
        RegistryName = "JetBrains Mono Bold (TrueType)"
        Url = "https://raw.githubusercontent.com/JetBrains/JetBrainsMono/master/fonts/ttf/JetBrainsMono-Bold.ttf"
    },
    @{
        Name = "CascadiaCode-Regular"
        FileName = "CascadiaCode-Regular.ttf"
        RegistryName = "Cascadia Code Regular (TrueType)"
        Url = "https://github.com/microsoft/cascadia-code/releases/download/v2407.24/CascadiaCode-2407.24.zip"
    }
)

$regKey = "HKCU:\Software\Microsoft\Windows NT\CurrentVersion\Fonts"

$workspaceFontsDir = Join-Path $PSScriptRoot "..\fonts"
if (-not (Test-Path $workspaceFontsDir)) {
    New-Item -ItemType Directory -Path $workspaceFontsDir -Force | Out-Null
}

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
$sysFontsDir = Join-Path $env:windir "Fonts"
$sysRegKey = "HKLM:\Software\Microsoft\Windows NT\CurrentVersion\Fonts"

foreach ($font in $fontSources) {
    $targetPath = Join-Path $userFontsDir $font.FileName
    $assetPath = Join-Path $assetsFontsDir $font.FileName
    $workspacePath = Join-Path $workspaceFontsDir $font.FileName
    Write-Host "[Installing] $($font.Name)..." -ForegroundColor Yellow

    try {
        if ($font.Url.EndsWith(".zip")) {
            $zipPath = Join-Path $env:TEMP "$($font.Name).zip"
            Invoke-WebRequest -Uri $font.Url -OutFile $zipPath -UseBasicParsing
            $extractDir = Join-Path $env:TEMP "$($font.Name)_extracted"
            Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force
            
            $ttfFound = Get-ChildItem -Path $extractDir -Recurse -Filter "CascadiaCode*.ttf" | Select-Object -First 1
            if ($ttfFound) {
                Copy-Item -Path $ttfFound.FullName -Destination $targetPath -Force
                Copy-Item -Path $ttfFound.FullName -Destination $assetPath -Force
                Copy-Item -Path $ttfFound.FullName -Destination $workspacePath -Force
                Set-ItemProperty -Path $regKey -Name $font.RegistryName -Value $targetPath -Force

                if ($isAdmin) {
                    $sysTargetPath = Join-Path $sysFontsDir $font.FileName
                    Copy-Item -Path $ttfFound.FullName -Destination $sysTargetPath -Force
                    Set-ItemProperty -Path $sysRegKey -Name $font.RegistryName -Value $font.FileName -Force
                }
                Write-Host "  -> Successfully installed $($font.Name)" -ForegroundColor Green
            }
            Remove-Item -Path $zipPath -Force -ErrorAction SilentlyContinue
            Remove-Item -Path $extractDir -Recurse -Force -ErrorAction SilentlyContinue
        } else {
            Invoke-WebRequest -Uri $font.Url -OutFile $targetPath -UseBasicParsing
            Copy-Item -Path $targetPath -Destination $assetPath -Force
            Copy-Item -Path $targetPath -Destination $workspacePath -Force
            Set-ItemProperty -Path $regKey -Name $font.RegistryName -Value $targetPath -Force

            if ($isAdmin) {
                $sysTargetPath = Join-Path $sysFontsDir $font.FileName
                Copy-Item -Path $targetPath -Destination $sysTargetPath -Force
                Set-ItemProperty -Path $sysRegKey -Name $font.RegistryName -Value $font.FileName -Force
            }
            Write-Host "  -> Successfully installed $($font.Name)" -ForegroundColor Green
        }
    } catch {
        Write-Warning "  Failed to download/install $($font.Name): $_"
    }
}

if ($isAdmin) {
    Write-Host "`nInstalled system-wide to C:\Windows\Fonts! Floem will detect all fonts immediately." -ForegroundColor Green
} else {
    Write-Host "`nInstalled to user font directory. For system-wide availability in fontdb, run install_fonts.bat as Administrator." -ForegroundColor Yellow
}
