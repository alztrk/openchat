$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$frontend = Join-Path $root 'apps\desktop\frontend'

if (-not (Test-Path (Join-Path $frontend 'node_modules'))) {
    npm --prefix $frontend ci
}

npm --prefix $frontend run tauri:dev
