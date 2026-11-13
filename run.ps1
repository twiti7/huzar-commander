$cargoBin = "$env:USERPROFILE\.cargo\bin"
$llvmBin = "C:\Users\piotr\AppData\Local\Microsoft\WinGet\Packages\MartinStorsjo.LLVM-MinGW.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\llvm-mingw-20261020-ucrt-x86_64\bin"
$winlibsBin = "C:\Users\piotr\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin"
$env:PATH = "$cargoBin;$llvmBin;$winlibsBin;$env:PATH"

cargo run
