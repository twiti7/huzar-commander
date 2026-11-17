<#
.SYNOPSIS
    Deinstalator programu Huzar Commander.
.DESCRIPTION
    Usuwa skróty, wpisy z rejestru, menu kontekstowe oraz pliki programu.
#>

[CmdletBinding()]
param(
    [switch]$Silent
)

$ErrorActionPreference = "SilentlyContinue"

if (-not $Silent) {
    Write-Host "==========================================================" -ForegroundColor Cyan
    Write-Host "     Deinstalator programu Huzar Commander                " -ForegroundColor Cyan
    Write-Host "==========================================================" -ForegroundColor Cyan
    $Confirm = Read-Host "Czy na pewno chcesz odinstalować program Huzar Commander? (t/N)"
    if ($Confirm -notmatch '^[tTyY]') {
        Write-Host "Anulowano deinstalację." -ForegroundColor Yellow
        exit 0
    }
}

# 1. Zatrzymanie procesu
Stop-Process -Name huzar-commander -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 300

# 2. Usunięcie skrótów
$StartMenuPrograms = [Environment]::GetFolderPath([Environment+SpecialFolder]::Programs)
$StartShortcutPath = Join-Path $StartMenuPrograms "Huzar Commander.lnk"
if (Test-Path $StartShortcutPath) { Remove-Item -Path $StartShortcutPath -Force }

$DesktopFolder = [Environment]::GetFolderPath([Environment+SpecialFolder]::Desktop)
$DesktopShortcutPath = Join-Path $DesktopFolder "Huzar Commander.lnk"
if (Test-Path $DesktopShortcutPath) { Remove-Item -Path $DesktopShortcutPath -Force }

# 3. Usunięcie wpisów z rejestru (menu kontekstowe)
Remove-Item -Path "HKCU:\Software\Classes\Directory\shell\HuzarCommander" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path "HKCU:\Software\Classes\Directory\Background\shell\HuzarCommander" -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item -Path "HKCU:\Software\Classes\Drive\shell\HuzarCommander" -Recurse -Force -ErrorAction SilentlyContinue

# 4. Usunięcie wpisu z Zainstalowane Aplikacje
Remove-Item -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\HuzarCommander" -Recurse -Force -ErrorAction SilentlyContinue

# 5. Usunięcie z PATH
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\HuzarCommander"
$UserEnvKey = "HKCU:\Environment"
$CurrentPath = (Get-ItemProperty -Path $UserEnvKey -Name "Path" -ErrorAction SilentlyContinue).Path
if ($CurrentPath) {
    $Paths = $CurrentPath -split ';' | Where-Object { $_ -and ($_ -ne $InstallDir) }
    $NewPath = $Paths -join ';'
    Set-ItemProperty -Path $UserEnvKey -Name "Path" -Value $NewPath
}

# 6. Usunięcie plików programu
if (Test-Path $InstallDir) {
    # Skrypt może być uruchomiony z wnętrza $InstallDir, więc usuwamy w tle po wyjściu lub kasujemy zawartość
    $Files = Get-ChildItem -Path $InstallDir -Exclude "uninstall.ps1"
    foreach ($f in $Files) {
        Remove-Item -Path $f.FullName -Recurse -Force -ErrorAction SilentlyContinue
    }

    # Zaplanowanie usunięcia pozostałości folderu po zamknięciu procesu PowerShell
    Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList "/c timeout /t 1 /nobreak >nul & rmdir /s /q `"$InstallDir`""
}

if (-not $Silent) {
    Write-Host "`nProgram Huzar Commander został pomyślnie odinstalowany z systemu." -ForegroundColor Green
    Write-Host ""
}
