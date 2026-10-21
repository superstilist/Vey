param([switch]$KeepBuildCache, [switch]$RequireAndroid)
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
$version = [regex]::Match((Get-Content -LiteralPath (Join-Path $projectRoot 'Cargo.toml') -Raw), '(?m)^version = "([0-9]+\.[0-9]+\.[0-9]+)"').Groups[1].Value
if (-not $version) { throw 'Cannot determine the current release version.' }
$keep = @("Vey-$version-windows-x64", "Vey-$version-windows-x64.zip", "Vey-$version-windows-x64.zip.sha256", "Vey-$version-android-arm64.apk", "Vey-$version-android-arm64.apk.sha256")
$required = @($keep[1], $keep[2])
if ($RequireAndroid) { $required += @($keep[3], $keep[4]) }
foreach ($name in $required) {
    if (-not (Test-Path -LiteralPath (Join-Path $projectRoot "dist/$name") -PathType Leaf)) {
        throw "Build the new release before removing old artifacts: missing $name"
    }
}
function Remove-Generated([string]$Path) {
    $absolute = [IO.Path]::GetFullPath($Path)
    if (-not $absolute.StartsWith($projectRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Cleanup path is outside this checkout: $absolute"
    }
    if (-not (Test-Path -LiteralPath $absolute)) { return }
    $relative = $absolute.Substring($projectRoot.Length + 1).Replace('\', '/')
    $tracked = @(git -C $projectRoot ls-files -- $relative)
    if ($LASTEXITCODE -ne 0 -or $tracked.Count -gt 0) { throw "Refusing to delete tracked files: $relative" }
    if ((Get-Item -LiteralPath $absolute -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Refusing recursive cleanup of a link: $relative"
    }
    Write-Output "Removing $relative"
    Remove-Item -LiteralPath $absolute -Recurse -Force
}
Get-ChildItem -LiteralPath (Join-Path $projectRoot 'dist') -Force |
    Where-Object { $_.Name -notlike ('Vey-' + $version + '-*') } | ForEach-Object { Remove-Generated $_.FullName }
Get-ChildItem -LiteralPath $projectRoot -File -Force |
    Where-Object { $_.Name -match '(?i)(\.log|\.err|\.out|\.bak|-(build|link|results|run|smoke)\.txt)$' -or $_.Name -in @('wasm-tools.txt', 'live-ytm.txt') } |
    ForEach-Object { Remove-Generated $_.FullName }
foreach ($relative in @('docs/smoke-data', 'web/pkg')) {
    Remove-Generated (Join-Path $projectRoot $relative)
}
if (-not $KeepBuildCache) { Remove-Generated (Join-Path $projectRoot 'target') }
Write-Output "Kept release $version; source, reference checkouts and user data are preserved."
