$cargoBin = "$env:USERPROFILE\.cargo\bin"
$llvmBin = "C:\Users\piotr\AppData\Local\Microsoft\WinGet\Packages\MartinStorsjo.LLVM-MinGW.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\llvm-mingw-20261020-ucrt-x86_64\bin"
$winlibsBin = "C:\Users\piotr\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$env:PATH = "$cargoBin;$llvmBin;$winlibsBin;$env:PATH"

$exe = "target\release\huzar-commander.exe"
if (-not (Test-Path $exe)) {
    $exe = "target\x86_64-pc-windows-gnullvm\release\huzar-commander.exe"
}

if (Test-Path $exe) {
    Start-Process $exe
} else {
    cargo run --release
}
