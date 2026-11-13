$cargoBin = "$env:USERPROFILE\.cargo\bin"
$llvmBin = "C:\Users\piotr\AppData\Local\Microsoft\WinGet\Packages\MartinStorsjo.LLVM-MinGW.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\llvm-mingw-20261020-ucrt-x86_64\bin"
$winlibsBin = "C:\Users\piotr\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$env:PATH = "$cargoBin;$llvmBin;$winlibsBin;$env:PATH"

Stop-Process -Name huzar-commander -ErrorAction SilentlyContinue -Force

Write-Host "Kompilacja wersji RELEASE (optymalizacje O3)..." -ForegroundColor Cyan
cargo build --release

if ($LASTEXITCODE -eq 0) {
    New-Item -ItemType Directory -Force -Path target\release | Out-Null
    Copy-Item target\x86_64-pc-windows-gnullvm\release\huzar-commander.exe -Destination target\release\huzar-commander.exe -Force
    Write-Host "`n[SUKCES] Skompilowano wersje RELEASE:" -ForegroundColor Green
    Write-Host " -> target\release\huzar-commander.exe" -ForegroundColor Yellow
}
