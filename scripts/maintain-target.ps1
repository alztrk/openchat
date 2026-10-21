[CmdletBinding()]
param (
    [ValidateRange(1, 1024)]
    [int]$MaxGiB = 20,

    [switch]$Apply
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $workspaceRoot 'Cargo.toml'
$workspacePrefix = [System.IO.Path]::GetFullPath($workspaceRoot).TrimEnd('\') + '\'
$relativeTargets = @(
    'target',
    'services\openchat-local-service\target',
    'apps\desktop\tauri\target'
)

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "Workspace manifest was not found: $manifestPath"
}

$targets = [System.Collections.Generic.List[object]]::new()
foreach ($relativeTarget in $relativeTargets) {
    $candidatePath = Join-Path $workspaceRoot $relativeTarget
    if (-not (Test-Path -LiteralPath $candidatePath -PathType Container)) {
        continue
    }

    $directory = Get-Item -LiteralPath $candidatePath -Force
    if (($directory.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        Write-Warning "Skipping reparse-point target directory: $candidatePath"
        continue
    }

    $targetPath = [System.IO.Path]::GetFullPath($directory.FullName)
    if (-not $targetPath.StartsWith($workspacePrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Target directory resolves outside the workspace: $targetPath"
    }

    $hasCargoMarker = (Test-Path -LiteralPath (Join-Path $targetPath '.rustc_info.json') -PathType Leaf) -or
        (Test-Path -LiteralPath (Join-Path $targetPath 'CACHEDIR.TAG') -PathType Leaf)
    if (-not $hasCargoMarker) {
        Write-Warning "Skipping directory without a Cargo marker: $targetPath"
        continue
    }

    $fileBytes = (Get-ChildItem -LiteralPath $targetPath -File -Recurse -Force | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $fileBytes) {
        $fileBytes = 0
    }

    $targets.Add([pscustomobject]@{
        Path = $targetPath
        Bytes = [long]$fileBytes
    })
}

$totalBytes = [long](($targets | Measure-Object -Property Bytes -Sum).Sum)
$totalGiB = $totalBytes / 1GB
$thresholdBytes = [long]$MaxGiB * 1GB

Write-Output ("Cargo target total: {0:N2} GiB; limit: {1} GiB." -f $totalGiB, $MaxGiB)
foreach ($target in $targets) {
    Write-Output ("{0:N2} GiB  {1}" -f ($target.Bytes / 1GB), $target.Path)
}

if ($totalBytes -le $thresholdBytes) {
    Write-Output 'Target usage is within the limit; no cleanup is needed.'
    return
}

if (-not $Apply) {
    Write-Output 'Preview only. Pass -Apply to clean these Cargo target directories.'
    return
}

$activeBuildProcesses = Get-Process -Name cargo, rustc -ErrorAction SilentlyContinue
if ($null -ne $activeBuildProcesses) {
    Write-Output 'A Cargo or rustc process is running; cleanup was skipped.'
    return
}

$cargo = Get-Command cargo.exe -CommandType Application -ErrorAction Stop
foreach ($target in $targets) {
    Write-Output "Previewing Cargo cleanup for $($target.Path)"
    & $cargo.Source clean --manifest-path $manifestPath --target-dir $target.Path --locked --dry-run
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo cleanup preview failed for $($target.Path) with exit code $LASTEXITCODE."
    }
}

foreach ($target in $targets) {
    Write-Output "Cleaning Cargo target directory $($target.Path)"
    & $cargo.Source clean --manifest-path $manifestPath --target-dir $target.Path --locked
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo cleanup failed for $($target.Path) with exit code $LASTEXITCODE."
    }
}
