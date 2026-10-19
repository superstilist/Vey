param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Push-Location $projectRoot
try {
    if (-not $SkipBuild) {
        Push-Location tools/braccato-widget
        try {
            npm run build
            if ($LASTEXITCODE -ne 0) { throw 'Widget build failed' }
        } finally { Pop-Location }
        cargo build --release --bin vey --locked
        if ($LASTEXITCODE -ne 0) { throw 'Release compilation failed' }
    }
    $binaryPath = Join-Path $projectRoot 'target/release/vey.exe'
    if (-not (Test-Path -LiteralPath $binaryPath)) { throw 'Release executable is missing' }
    $manifest = Get-Content Cargo.toml -Raw
    $version = [regex]::Match($manifest, '(?m)^version = "([^"]+)"').Groups[1].Value
    if (-not $version) { throw 'Package version is missing' }
    $packageName = "Vey-$version-windows-x64"
    $packagePath = Join-Path $projectRoot "dist/$packageName"
    New-Item -ItemType Directory -Force -Path $packagePath | Out-Null
    Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $packagePath 'vey.exe') -Force
    Copy-Item -LiteralPath docs/RELEASE.md -Destination (Join-Path $packagePath 'README.md') -Force
    Copy-Item -LiteralPath web/braccato/LICENSES.txt -Destination (Join-Path $packagePath 'THIRD-PARTY-LICENSES.txt') -Force
    $archivePath = Join-Path $projectRoot "dist/$packageName.zip"
    Compress-Archive -LiteralPath $packagePath -DestinationPath $archivePath -Force
    $hash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath "$archivePath.sha256" -Value "$hash  $packageName.zip" -Encoding ascii
    Write-Output "Release: $archivePath"
    Write-Output "SHA256: $hash"
} finally { Pop-Location }
