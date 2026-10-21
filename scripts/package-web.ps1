param([switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
Push-Location $root
$stage = Join-Path $root ('dist/.web-package-' + [guid]::NewGuid().ToString('N'))
try {
    if (-not $SkipBuild) { & (Join-Path $PSScriptRoot 'build-web.ps1') }
    foreach ($file in @('web/pkg/playervf.js', 'web/pkg/playervf_bg.wasm', 'web/braccato/widget.js')) {
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing web asset: $file" }
    }
    $version = [regex]::Match((Get-Content Cargo.toml -Raw), '(?m)^version = "([0-9.]+)"').Groups[1].Value
    if (-not $version) { throw 'Missing package version.' }
    $name = "Vey-$version-web"
    $package = Join-Path $stage $name
    New-Item -ItemType Directory -Force -Path $package | Out-Null
    Copy-Item -LiteralPath web -Destination (Join-Path $package 'web') -Recurse
    Copy-Item -LiteralPath docs/WEB-RELEASE.md -Destination (Join-Path $package 'README.md')
    Copy-Item -LiteralPath web/braccato/LICENSES.txt -Destination (Join-Path $package 'THIRD-PARTY-LICENSES.txt')
    Set-Content -LiteralPath (Join-Path $package 'SOURCE-COMMIT.txt') -Value (git rev-parse HEAD) -Encoding ascii
    @'
param([string]$Binary = (Join-Path $PSScriptRoot 'vey.exe'))
$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Binary -PathType Leaf)) { throw 'Place the Windows release vey.exe beside this script, or pass -Binary with its full path.' }
$Binary = (Resolve-Path -LiteralPath $Binary).Path
Push-Location $PSScriptRoot
try { & $Binary --web-server } finally { Pop-Location }
'@ | Set-Content -LiteralPath (Join-Path $package 'start-web.ps1') -Encoding utf8
    $launcher = @'
#!/usr/bin/env sh
set -eu
cd "$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
exec "${VEY_BINARY:-vey}" --web-server
'@
    [IO.File]::WriteAllText((Join-Path $package 'start-web.sh'), $launcher.Replace("`r`n", "`n") + "`n", [Text.UTF8Encoding]::new($false))
    $archive = Join-Path $root "dist/$name.zip"
    Compress-Archive -LiteralPath $package -DestinationPath $archive -Force
    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath "$archive.sha256" -Value "$hash  $name.zip" -Encoding ascii
    Write-Output "Web release: $archive"
    Write-Output "SHA256: $hash"
} finally {
    $absolute = [IO.Path]::GetFullPath($stage)
    if (-not $absolute.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) { throw 'Stage directory escaped the checkout.' }
    if (Test-Path -LiteralPath $absolute) { Remove-Item -LiteralPath $absolute -Recurse -Force }
    Pop-Location
}
