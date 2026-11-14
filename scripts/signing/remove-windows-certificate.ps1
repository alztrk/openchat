param(
    [Parameter(Mandatory = $true)]
    [string] $Thumbprint
)

$ErrorActionPreference = 'Stop'
$normalizedThumbprint = $Thumbprint -replace '\s', ''
if ($normalizedThumbprint -notmatch '^[A-Fa-f0-9]{40}$') {
    throw 'The Windows signing certificate thumbprint is invalid.'
}

$certificatePath = "Cert:\CurrentUser\My\$normalizedThumbprint"
if (Test-Path -LiteralPath $certificatePath) {
    Remove-Item -LiteralPath $certificatePath -Force
}
