[CmdletBinding()]
param(
    [string]$CertificateThumbprint,

    [Parameter(Mandatory)]
    [uri]$TimestampUrl
)

$ErrorActionPreference = 'Stop'

if (-not $TimestampUrl.IsAbsoluteUri -or $TimestampUrl.Scheme -notin @('http', 'https')) {
    throw 'TimestampUrl must be an absolute HTTP or HTTPS RFC 3161 timestamp URL.'
}

$now = Get-Date
$certificates = @(Get-ChildItem -Path Cert:\CurrentUser\My -CodeSigningCert |
    Where-Object { $_.HasPrivateKey -and $_.NotBefore -le $now -and $now -lt $_.NotAfter })
if ($CertificateThumbprint) {
    $CertificateThumbprint = $CertificateThumbprint -replace '\s', ''
    $certificates = @($certificates | Where-Object Thumbprint -EQ $CertificateThumbprint)
}
if ($certificates.Count -eq 0) {
    throw 'No usable code-signing certificate was found in Cert:\CurrentUser\My. Connect the signing token or provision the publisher certificate first.'
}
if ($certificates.Count -ne 1) {
    throw 'More than one code-signing certificate was found. Choose one with -CertificateThumbprint.'
}
$certificate = $certificates[0]

$project = Join-Path $PSScriptRoot 'src-tauri'
$settings = Get-Content -Raw (Join-Path $project 'tauri.conf.json') | ConvertFrom-Json
$overlay = @{
    bundle = @{
        windows = @{
            certificateThumbprint = $certificate.Thumbprint
            digestAlgorithm = 'sha256'
            timestampUrl = $TimestampUrl.AbsoluteUri
            tsp = $true
        }
    }
} | ConvertTo-Json -Depth 5

$cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
$cargoPath = if ($cargo) { $cargo.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
$originalPath = $env:Path
$overlayPath = [System.IO.Path]::GetTempFileName()
$overlayJson = "$overlayPath.json"
try {
    Move-Item -LiteralPath $overlayPath -Destination $overlayJson
    [System.IO.File]::WriteAllText($overlayJson, $overlay)
    $env:Path = "$(Split-Path -Parent $cargoPath);$originalPath"
    Push-Location $PSScriptRoot
    try {
        $metadataOutput = & $cargoPath metadata --locked --no-deps --format-version 1 --manifest-path (Join-Path $project 'Cargo.toml')
        if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata could not be read.' }
        $target = ($metadataOutput | ConvertFrom-Json).target_directory
        & $cargoPath tauri build --ci --bundles nsis --config $overlayJson -- --locked
        if ($LASTEXITCODE -ne 0) { throw "Tauri build or signing failed (exit code $LASTEXITCODE)." }
    } finally {
        Pop-Location
    }

    $release = Join-Path $target 'release'
    $application = Join-Path $release 'daymark.exe'
    $installer = Join-Path $release "bundle\nsis\$($settings.productName)_$($settings.version)_x64-setup.exe"
    foreach ($file in @($application, $installer)) {
        $signature = Get-AuthenticodeSignature -FilePath $file
        if ($signature.Status -ne 'Valid' -or
            $signature.SignerCertificate.Thumbprint -ne $certificate.Thumbprint -or
            -not $signature.TimeStamperCertificate) {
            throw "A valid, timestamped publisher signature could not be verified for $file."
        }
        Write-Output $file
    }
} finally {
    $env:Path = $originalPath
    Remove-Item -LiteralPath $overlayPath, $overlayJson -ErrorAction SilentlyContinue
}
