# Optional portable GNU tools. Uses the normal PATH on machines with a configured Rust installation.
$portable = Join-Path $env:TEMP 'au05-build-tools'
if (Test-Path "$portable/cargo/bin/cargo.exe") {
    $env:CARGO_HOME = "$portable/cargo"
    $env:RUSTUP_HOME = "$portable/rustup"
    $env:Path = "$portable/cargo/bin;" + $env:Path
    $selfContained = "$portable/rustup/toolchains/stable-x86_64-pc-windows-gnu/lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained"
    $env:Path = "$selfContained;" + $env:Path
    if (Test-Path "$portable/ucrt64/bin/gcc.exe") {
        $env:Path = "$portable/ucrt64/bin;" + $env:Path
        $env:CC = 'gcc'
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = 'gcc'
    }
    if (Test-Path "$portable/ustc/ucrt64/bin/gcc.exe") {
        $env:Path = "$portable/ustc/ucrt64/bin;" + $env:Path
        $env:CC = 'gcc'
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = 'gcc'
    }
    $gnu = Get-ChildItem -Directory "$portable/llvm-mingw-*" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($gnu) {
        $env:Path = "$($gnu.FullName)/bin;" + $env:Path
        $env:CC = 'x86_64-w64-mingw32-clang'
        $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = 'x86_64-w64-mingw32-clang'
    }
}
