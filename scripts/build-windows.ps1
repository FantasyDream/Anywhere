# Run on Windows x64 with Rust, Flutter and Visual Studio's Desktop C++ workload.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if ($env:OS -ne 'Windows_NT') { throw 'The Windows bundle must be built on a Windows host.' }
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() -ne 'X64') {
    throw 'This script packages x64 and requires a Windows x64 host.'
}
Push-Location $repo
try {
    & rustup target add x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Installing the Rust Windows target failed.' }
    & cargo build --locked --release --target x86_64-pc-windows-msvc -p anywhere-bridge-flutter
    if ($LASTEXITCODE -ne 0) { throw 'Rust build failed.' }
    Push-Location apps/flutter
    try {
        & flutter pub get
        if ($LASTEXITCODE -ne 0) { throw 'Flutter dependency resolution failed.' }
        & flutter build windows --release --verbose
        if ($LASTEXITCODE -ne 0) { throw 'Flutter Windows build failed.' }
    } finally { Pop-Location }
    $bundle = Join-Path $repo 'apps/flutter/build/windows/x64/runner/Release'
    $archive = Join-Path $repo 'build/Anywhere-Windows-x64.zip'
    New-Item -ItemType Directory -Force (Split-Path $archive) | Out-Null
    Compress-Archive -Path (Join-Path $bundle '*') -DestinationPath $archive -Force
    Write-Host "Built $archive"
    Write-Host 'Extract the entire ZIP and run anywhere.exe; keep DLLs and data beside it.'
} finally { Pop-Location }
