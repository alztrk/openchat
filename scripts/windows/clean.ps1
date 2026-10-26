$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

$targets = @(
    (Join-Path $root 'target'),
    (Join-Path $root 'services\openchat-local-service\target'),
    (Join-Path $root 'apps\desktop\tauri\target'),
    (Join-Path $root 'apps\desktop\frontend\dist'),
    (Join-Path $root 'apps\desktop\tauri\binaries'),
    (Join-Path $root 'apps\desktop\tauri\gen')
)

foreach ($target in $targets) {
    if (Test-Path -LiteralPath $target) {
        Remove-Item -LiteralPath $target -Recurse -Force
    }
}
