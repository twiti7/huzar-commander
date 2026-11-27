<#
.SYNOPSIS
    Kompilator instalatora Inno Setup dla Huzar Commander.
.DESCRIPTION
    1. Buduje wersję produkcyjną (cargo build --release).
    2. Szuka kompilatora Inno Setup (ISCC.exe).
    3. Kompiluje instalator do target\installer\HuzarCommander-Setup-v0.3.0.exe.
#>

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "   Budowanie instalatora Huzar Commander (Inno Setup)     " -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Zatrzymanie działającego procesu jeśli był uruchomiony
Stop-Process -Name huzar-commander -ErrorAction SilentlyContinue -Force

# 2. Kompilacja wersji release
Write-Host "`n[1/3] Kompilacja wersji Release (cargo build --release)..." -ForegroundColor Yellow
Push-Location $ScriptDir
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "Kompilacja cargo build --release nie powiodła się."
    }
} finally {
    Pop-Location
}

# 3. Poszukiwanie ISCC.exe (Inno Setup Compiler)
Write-Host "`n[2/3] Wyszukiwanie kompilatora Inno Setup (ISCC.exe)..." -ForegroundColor Yellow

$CandidatePaths = @(
    (Get-Command iscc -ErrorAction SilentlyContinue).Source,
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 6\ISCC.exe",
    "${env:LOCALAPPDATA}\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 5\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 5\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) }

$IsccPath = $CandidatePaths | Select-Object -First 1

if (-not $IsccPath) {
    Write-Host "`n[UWAGA] Nie znaleziono Inno Setup (ISCC.exe) na tym komputerze." -ForegroundColor Yellow
    Write-Host "Aby skompilować instalator .exe za pomocą Inno Setup:" -ForegroundColor White
    Write-Host "  1. Zainstaluj Inno Setup poleceniem:" -ForegroundColor Cyan
    Write-Host "     winget install JRSoftware.InnoSetup" -ForegroundColor Green
    Write-Host "  2. Następnie uruchom ten skrypt ponownie: .\build_installer.ps1" -ForegroundColor Cyan
    Write-Host "`n[WSKAZÓWKA] Możesz także zainstalować program natychmiastowo bez Inno Setup:" -ForegroundColor White
    Write-Host "  Uruchom wbudowany instalator PowerShell: .\install.ps1" -ForegroundColor Green
    exit 0
}

Write-Host "Znaleziono Inno Setup: $IsccPath" -ForegroundColor Green

# 4. Uruchomienie kompilacji instalatora
Write-Host "`n[3/3] Kompilacja instalatora Inno Setup..." -ForegroundColor Yellow
$IssFile = Join-Path $ScriptDir "installer\huzar_setup.iss"

& $IsccPath $IssFile
if ($LASTEXITCODE -ne 0) {
    throw "Błąd podczas kompilacji Inno Setup."
}

$OutputDir = Join-Path $ScriptDir "target\installer"
$Installer = Get-ChildItem -Path $OutputDir -Filter "HuzarCommander-Setup-*.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1

if ($Installer) {
    Write-Host "`nSUKCES! Gotowy instalator znajduje się w:" -ForegroundColor Green
    Write-Host "  $($Installer.FullName)" -ForegroundColor White
    Write-Host "  Rozmiar: $([math]::Round($Installer.Length / 1MB, 2)) MB" -ForegroundColor Gray
} else {
    Write-Host "`nKompilacja zakończona." -ForegroundColor Green
}
