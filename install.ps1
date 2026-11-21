<#
.SYNOPSIS
    Natywny instalator PowerShell dla programu Huzar Commander.
.DESCRIPTION
    Instaluje program w profilu użytkownika (%LOCALAPPDATA%\Programs\HuzarCommander)
    bez konieczności posiadania uprawnień administratora ani zewnętrznych narzędzi.
    - Tworzy skrót na Pulpicie i w Menu Start
    - Rejestruje program w "Aplikacje i funkcje" systemu Windows (panel deinstalacji)
    - Integruje program z menu kontekstowym Eksploratora Windows ("Otwórz w programie Huzar Commander")
    - Dodaje polecenie huzar / huzar-commander do zmiennej PATH użytkownika
#>

[CmdletBinding()]
param(
    [switch]$NoShortcuts,
    [switch]$NoContextMenu,
    [switch]$NoPath
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "      Instalator programu Huzar Commander (Windows)       " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Sprawdzenie/zbudowanie pliku binarnego
$ExeCandidates = @(
    (Join-Path $ScriptDir "target\release\huzar-commander.exe"),
    (Join-Path $ScriptDir "target\x86_64-pc-windows-gnullvm\release\huzar-commander.exe"),
    (Join-Path $ScriptDir "target\debug\huzar-commander.exe"),
    (Join-Path $ScriptDir "target\x86_64-pc-windows-gnullvm\debug\huzar-commander.exe")
)

$SourceExe = $ExeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $SourceExe) {
    Write-Host "`n[1/5] Budowanie wersji produkcyjnej (cargo build --release)..." -ForegroundColor Yellow
    Push-Location $ScriptDir
    try {
        cargo build --release
    } finally {
        Pop-Location
    }
    $SourceExe = $ExeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $SourceExe) {
        throw "Nie znaleziono skompilowanego pliku huzar-commander.exe!"
    }
} else {
    Write-Host "`n[1/5] Użyto pliku wykonywalnego: $SourceExe" -ForegroundColor Green
}

# 2. Katalog docelowy instalacji
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\HuzarCommander"
Write-Host "`n[2/5] Kopiowanie plików do: $InstallDir" -ForegroundColor Yellow

# Zatrzymanie ewentualnie działającego procesu
Stop-Process -Name huzar-commander -ErrorAction SilentlyContinue -Force
Start-Sleep -Milliseconds 200

if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

$DestExe = Join-Path $InstallDir "huzar-commander.exe"
Copy-Item -Path $SourceExe -Destination $DestExe -Force

# Kopiowanie zasobów (ikona, locales, readme)
$IconIco = Join-Path $ScriptDir "assets\huzar_icon.ico"
$IconPng = Join-Path $ScriptDir "assets\huzar_icon.png"
if (Test-Path $IconIco) { Copy-Item -Path $IconIco -Destination $InstallDir -Force }
if (Test-Path $IconPng) { Copy-Item -Path $IconPng -Destination $InstallDir -Force }

$LocalesDir = Join-Path $ScriptDir "locales"
if (Test-Path $LocalesDir) {
    Copy-Item -Path $LocalesDir -Destination $InstallDir -Recurse -Force
}

$Readme = Join-Path $ScriptDir "README.md"
$ReadmePl = Join-Path $ScriptDir "README_PL.md"
if (Test-Path $Readme) { Copy-Item -Path $Readme -Destination $InstallDir -Force }
if (Test-Path $ReadmePl) { Copy-Item -Path $ReadmePl -Destination $InstallDir -Force }

# Utworzenie skryptu huzar.cmd (szybkie wywołanie w cmd/powershell/Win+R)
$HuzarCmd = Join-Path $InstallDir "huzar.cmd"
"@echo off`r`nstart `"`" `"%~dp0huzar-commander.exe`" %*" | Set-Content -Path $HuzarCmd -Encoding ASCII

# 3. Tworzenie skrótów (Pulpit i Menu Start)
if (-not $NoShortcuts) {
    Write-Host "`n[3/5] Tworzenie skrótów z husarską ikoną..." -ForegroundColor Yellow
    $WshShell = New-Object -ComObject WScript.Shell

    $InstalledIcon = Join-Path $InstallDir "huzar_icon.ico"

    # Skrót w Menu Start
    $StartMenuPrograms = [Environment]::GetFolderPath([Environment+SpecialFolder]::Programs)
    $StartShortcutPath = Join-Path $StartMenuPrograms "Huzar Commander.lnk"
    $Shortcut = $WshShell.CreateShortcut($StartShortcutPath)
    $Shortcut.TargetPath = $DestExe
    $Shortcut.WorkingDirectory = $InstallDir
    $Shortcut.Description = "Dwupanelowy menedżer plików Huzar Commander"
    if (Test-Path $InstalledIcon) {
        $Shortcut.IconLocation = "$InstalledIcon,0"
    }
    $Shortcut.Save()
    Write-Host "  -> Menu Start: $StartShortcutPath" -ForegroundColor Gray

    # Skrót na Pulpicie
    $DesktopFolder = [Environment]::GetFolderPath([Environment+SpecialFolder]::Desktop)
    $DesktopShortcutPath = Join-Path $DesktopFolder "Huzar Commander.lnk"
    $Shortcut = $WshShell.CreateShortcut($DesktopShortcutPath)
    $Shortcut.TargetPath = $DestExe
    $Shortcut.WorkingDirectory = $InstallDir
    $Shortcut.Description = "Dwupanelowy menedżer plików Huzar Commander"
    if (Test-Path $InstalledIcon) {
        $Shortcut.IconLocation = "$InstalledIcon,0"
    }
    $Shortcut.Save()
    Write-Host "  -> Pulpit: $DesktopShortcutPath" -ForegroundColor Gray
}

# 4. Integracja z Windows (Menu kontekstowe i Dodaj/Usuń programy)
Write-Host "`n[4/5] Rejestracja w systemie Windows..." -ForegroundColor Yellow

$InstalledIcon = Join-Path $InstallDir "huzar_icon.ico"

# Rejestracja w Aplikacje i funkcje (Add / Remove Programs)
$UninstallRegKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\HuzarCommander"
if (-not (Test-Path $UninstallRegKey)) {
    New-Item -Path $UninstallRegKey -Force | Out-Null
}

$UninstallScriptDest = Join-Path $InstallDir "uninstall.ps1"
Copy-Item -Path (Join-Path $ScriptDir "uninstall.ps1") -Destination $UninstallScriptDest -Force

Set-ItemProperty -Path $UninstallRegKey -Name "DisplayName" -Value "Huzar Commander"
Set-ItemProperty -Path $UninstallRegKey -Name "DisplayVersion" -Value "0.2.0"
Set-ItemProperty -Path $UninstallRegKey -Name "Publisher" -Value "Huzar Commander Team"
Set-ItemProperty -Path $UninstallRegKey -Name "InstallLocation" -Value $InstallDir
Set-ItemProperty -Path $UninstallRegKey -Name "DisplayIcon" -Value $InstalledIcon
Set-ItemProperty -Path $UninstallRegKey -Name "UninstallString" -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$UninstallScriptDest`""
Set-ItemProperty -Path $UninstallRegKey -Name "QuietUninstallString" -Value "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$UninstallScriptDest`" -Silent"
Write-Host "  -> Zarejestrowano w 'Zainstalowane aplikacje' systemu Windows" -ForegroundColor Gray

# Menu kontekstowe folderów w Eksploratorze Windows
if (-not $NoContextMenu) {
    # 1. Prawy przycisk na folderze
    $DirShellKey = "HKCU:\Software\Classes\Directory\shell\HuzarCommander"
    New-Item -Path "$DirShellKey\command" -Force | Out-Null
    Set-ItemProperty -Path $DirShellKey -Name "(Default)" -Value "Otwórz w programie Huzar Commander"
    Set-ItemProperty -Path $DirShellKey -Name "Icon" -Value "`"$InstalledIcon`""
    Set-ItemProperty -Path "$DirShellKey\command" -Name "(Default)" -Value "`"$DestExe`" `"%1`""

    # 2. Prawy przycisk w tle otwartego folderu
    $DirBgShellKey = "HKCU:\Software\Classes\Directory\Background\shell\HuzarCommander"
    New-Item -Path "$DirBgShellKey\command" -Force | Out-Null
    Set-ItemProperty -Path $DirBgShellKey -Name "(Default)" -Value "Otwórz w programie Huzar Commander"
    Set-ItemProperty -Path $DirBgShellKey -Name "Icon" -Value "`"$InstalledIcon`""
    Set-ItemProperty -Path "$DirBgShellKey\command" -Name "(Default)" -Value "`"$DestExe`" `"%V`""

    # 3. Prawy przycisk na dysku
    $DriveShellKey = "HKCU:\Software\Classes\Drive\shell\HuzarCommander"
    New-Item -Path "$DriveShellKey\command" -Force | Out-Null
    Set-ItemProperty -Path $DriveShellKey -Name "(Default)" -Value "Otwórz w programie Huzar Commander"
    Set-ItemProperty -Path $DriveShellKey -Name "Icon" -Value "`"$InstalledIcon`""
    Set-ItemProperty -Path "$DriveShellKey\command" -Name "(Default)" -Value "`"$DestExe`" `"%1`""

    Write-Host "  -> Dodano opcję 'Otwórz w programie Huzar Commander' do menu kontekstowego folderów i dysków" -ForegroundColor Gray
}

# 5. Dodanie do PATH użytkownika
if (-not $NoPath) {
    Write-Host "`n[5/5] Konfiguracja zmiennej środowiskowej PATH..." -ForegroundColor Yellow
    $UserEnvKey = "HKCU:\Environment"
    $CurrentPath = (Get-ItemProperty -Path $UserEnvKey -Name "Path" -ErrorAction SilentlyContinue).Path
    if (-not $CurrentPath) { $CurrentPath = "" }

    $Paths = $CurrentPath -split ';' | Where-Object { $_ -and (Test-Path $_) }
    if ($Paths -notcontains $InstallDir) {
        $NewPath = ($Paths + $InstallDir) -join ';'
        Set-ItemProperty -Path $UserEnvKey -Name "Path" -Value $NewPath
        Write-Host "  -> Dodano $InstallDir do PATH (polecenia: huzar, huzar-commander)" -ForegroundColor Gray
    } else {
        Write-Host "  -> Folder już znajduje się w PATH" -ForegroundColor Gray
    }
}

Write-Host "`n==========================================================" -ForegroundColor Green
Write-Host "     INSTALACJA ZAKOŃCZONA SUKCESEM!                      " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
Write-Host "Program zainstalowany w: $InstallDir" -ForegroundColor White
Write-Host "Możesz uruchomić Huzar Commander:" -ForegroundColor White
Write-Host "  1. Ze skrótu na Pulpicie lub w Menu Start" -ForegroundColor Cyan
Write-Host "  2. Prawym przyciskiem myszy na dowolnym folderze w Windowsie" -ForegroundColor Cyan
Write-Host "  3. Wpisując 'huzar' lub 'huzar-commander' w terminalu lub oknie Uruchom (Win+R)" -ForegroundColor Cyan
Write-Host ""
