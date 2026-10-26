$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
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
    cargo check --workspace
    cargo test --workspace -- --test-threads=1
}
finally {
    Pop-Location
}
