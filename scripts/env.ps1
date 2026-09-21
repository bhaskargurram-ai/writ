# Source this before any cargo command in this environment:
#   . .\scripts\env.ps1
# Adds cargo + the GNU toolchain's self-contained binutils (dlltool/ld/gcc)
# to PATH. Needed because this machine has no MSVC Build Tools (ADR-006).
$env:Path = "$env:USERPROFILE\.cargo\bin;" +
    "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained;" +
    $env:Path
