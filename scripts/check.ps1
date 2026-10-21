$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$frontend = Join-Path $root 'apps\desktop\frontend'

Push-Location $frontend
try {
    npx biome check
    npm run build
}
finally {
    Pop-Location
}

Push-Location $root
try {
    cargo fmt --all -- --check
    cargo check --workspace --all-targets
    cargo test -p openchat_service -- --test-threads=1
    cargo test -p openchat-desktop -- --test-threads=1
}
finally {
    Pop-Location
}
