$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$frontend = Join-Path $root 'apps\desktop\frontend'
$binaries = Join-Path $root 'apps\desktop\tauri\binaries'

Push-Location $root
try {
    cargo build -p openchat-local-service --bin openchat-local-service
    if ($LASTEXITCODE -ne 0) {
        throw 'The local service build failed.'
    }

    New-Item -ItemType Directory -Force -Path $binaries | Out-Null
    Copy-Item -LiteralPath (Join-Path $root 'target\debug\openchat-local-service.exe') `
        -Destination (Join-Path $binaries 'openchat-local-service-x86_64-pc-windows-msvc.exe') -Force

    if (-not (Test-Path (Join-Path $frontend 'node_modules'))) {
        npm --prefix $frontend ci
        if ($LASTEXITCODE -ne 0) {
            throw 'Frontend dependency installation failed.'
        }
    }

    npm --prefix $frontend run tauri:dev
    if ($LASTEXITCODE -ne 0) {
        throw 'The Tauri development application failed.'
    }
}
finally {
    Pop-Location
}
