$ErrorActionPreference = 'Stop'

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$releaseDirectory = Join-Path $projectRoot 'build/windows/x64/runner/Release'
$payloadArchive = Join-Path $projectRoot 'build/openchat-portable-payload.zip'
$launcherManifest = Join-Path $projectRoot 'native/zihora-launcher/Cargo.toml'
$launcherTarget = Join-Path $projectRoot 'build/launcher-target'
$outputDirectory = Join-Path $projectRoot 'build/outputs'
$outputExecutable = Join-Path $outputDirectory 'OpenChat.exe'

Push-Location $projectRoot
try {
    & flutter build windows --release
    if ($LASTEXITCODE -ne 0) {
        throw "flutter build windows --release exited with code $LASTEXITCODE."
    }

    $requiredFiles = @(
        'openchat.exe',
        'zihora_service.exe',
        'data/flutter_assets/AssetManifest.bin'
    )
    foreach ($relativePath in $requiredFiles) {
        $path = Join-Path $releaseDirectory $relativePath
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "The Windows release is missing required file: $relativePath"
        }
    }

    if (-not (Test-Path -LiteralPath $outputDirectory -PathType Container)) {
        New-Item -ItemType Directory -Path $outputDirectory | Out-Null
    }

    $releaseContents = @(
        Get-ChildItem -LiteralPath $releaseDirectory -Force |
            Where-Object { $_.Name -ne 'zihora.exe' }
    )
    Compress-Archive `
        -Path $releaseContents.FullName `
        -DestinationPath $payloadArchive `
        -CompressionLevel Optimal `
        -Force

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($payloadArchive)
    try {
        $entryNames = @($archive.Entries | ForEach-Object FullName)
        $missingEntries = @(
            $requiredFiles | Where-Object { $_ -notin $entryNames }
        )
        if ($missingEntries.Count -gt 0) {
            throw "The release archive is missing required entries: $($missingEntries -join ', ')"
        }
    }
    finally {
        $archive.Dispose()
    }

    & cargo build `
        --manifest-path $launcherManifest `
        --target-dir $launcherTarget `
        --locked `
        --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build for the portable launcher exited with code $LASTEXITCODE."
    }

    $launcherExecutable = Join-Path $launcherTarget 'release/zihora_launcher.exe'
    if (-not (Test-Path -LiteralPath $launcherExecutable -PathType Leaf)) {
        throw 'The portable launcher build did not produce zihora_launcher.exe.'
    }

    Copy-Item -LiteralPath $launcherExecutable -Destination $outputExecutable -Force
    Get-Item -LiteralPath $outputExecutable | Select-Object Length, LastWriteTime, FullName
}
finally {
    Pop-Location
}
