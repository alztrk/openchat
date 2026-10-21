$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$frontend = Join-Path $root 'apps\desktop\frontend'
$binaries = Join-Path $root 'apps\desktop\tauri\binaries'

Push-Location $root
try {
    cargo build -p openchat_service --release
    New-Item -ItemType Directory -Force -Path $binaries | Out-Null
    Copy-Item -LiteralPath (Join-Path $root 'target\release\openchat_service.exe') `
        -Destination (Join-Path $binaries 'openchat_service-x86_64-pc-windows-msvc.exe') -Force
    npm --prefix $frontend run tauri:build
}
finally {
    Pop-Location
}
