$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$frontend = Join-Path $root 'apps\desktop\frontend'
$binaries = Join-Path $root 'apps\desktop\tauri\binaries'

Push-Location $root
try {
    cargo build --locked --target x86_64-pc-windows-msvc -p openchat-local-service --bin openchat-local-service --release
    New-Item -ItemType Directory -Force -Path $binaries | Out-Null
    Copy-Item -LiteralPath (Join-Path $root 'target\x86_64-pc-windows-msvc\release\openchat-local-service.exe') `
        -Destination (Join-Path $binaries 'openchat-local-service-x86_64-pc-windows-msvc.exe') -Force
    npm --prefix $frontend run tauri:build
}
finally {
    Pop-Location
}
