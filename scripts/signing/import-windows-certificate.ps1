$ErrorActionPreference = 'Stop'

$encodedCertificate = $env:WINDOWS_CERTIFICATE
$certificatePassword = $env:WINDOWS_CERTIFICATE_PASSWORD
if ([string]::IsNullOrWhiteSpace($encodedCertificate) -or [string]::IsNullOrEmpty($certificatePassword)) {
    throw 'Windows stable signing requires WINDOWS_CERTIFICATE and WINDOWS_CERTIFICATE_PASSWORD secrets.'
}

if ($encodedCertificate.Length -gt 16 * 1024 * 1024) {
    throw 'The Windows signing certificate exceeds the supported size limit.'
}

$certificatePath = Join-Path ([System.IO.Path]::GetTempPath()) ("openchat-signing-{0}.pfx" -f [guid]::NewGuid())
$keepCertificate = $false
$importedCertificates = @()
try {
    $certificateBytes = [Convert]::FromBase64String($encodedCertificate)
    [System.IO.File]::WriteAllBytes($certificatePath, $certificateBytes)
    $securePassword = ConvertTo-SecureString -String $certificatePassword -AsPlainText -Force
    $importedCertificates = @(Import-PfxCertificate `
        -FilePath $certificatePath `
        -CertStoreLocation 'Cert:\CurrentUser\My' `
        -Password $securePassword)

    $codeSigningOid = '1.3.6.1.5.5.7.3.3'
    $signingCertificate = $importedCertificates |
        Where-Object {
            $_.HasPrivateKey -and
            $_.NotBefore -le [DateTime]::Now -and
            $_.NotAfter -gt [DateTime]::Now -and
            ($_.EnhancedKeyUsageList.ObjectId.Value -contains $codeSigningOid)
        } |
        Select-Object -First 1

    if ($null -eq $signingCertificate) {
        throw 'The imported certificate is not a currently valid Authenticode code-signing certificate with a private key.'
    }

    if ([string]::IsNullOrWhiteSpace($env:GITHUB_OUTPUT)) {
        throw 'GitHub Actions did not provide a step output file for the signing certificate.'
    }

    $outputRecord = "thumbprint=$($signingCertificate.Thumbprint)$([Environment]::NewLine)"
    [System.IO.File]::AppendAllText(
        $env:GITHUB_OUTPUT,
        $outputRecord,
        [System.Text.UTF8Encoding]::new($false))
    $keepCertificate = $true
}
finally {
    if (-not $keepCertificate) {
        foreach ($certificate in $importedCertificates) {
            $storePath = "Cert:\CurrentUser\My\$($certificate.Thumbprint)"
            if (Test-Path -LiteralPath $storePath) {
                Remove-Item -LiteralPath $storePath -Force
            }
        }
    }
    if (Test-Path -LiteralPath $certificatePath) {
        Remove-Item -LiteralPath $certificatePath -Force
    }
    $certificateBytes = $null
    $securePassword = $null
    $certificatePassword = $null
    $encodedCertificate = $null
}
