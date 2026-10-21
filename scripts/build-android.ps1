param([switch]$SkipBuild, [ValidateSet('arm64','x64')][string]$Architecture = 'arm64')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$targetTriple = @{ arm64 = 'aarch64-linux-android'; x64 = 'x86_64-linux-android' }[$Architecture]
$expectedAbi = @{ arm64 = 'arm64-v8a'; x64 = 'x86_64' }[$Architecture]
$androidSdkPath = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { $env:ANDROID_SDK_ROOT }
if (-not $androidSdkPath) { throw 'Set ANDROID_HOME to the installed Android SDK.' }
if (-not $env:JAVA_HOME) { throw 'Set JAVA_HOME to an installed JDK (17 or newer).' }
$androidNdkPath = $env:ANDROID_NDK_HOME
if (-not $androidNdkPath) {
    $androidNdkPath = Get-ChildItem -LiteralPath (Join-Path $androidSdkPath 'ndk') -Directory |
        Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $androidNdkPath) { throw 'Install an Android NDK in the SDK, or set ANDROID_NDK_HOME.' }
$buildToolsPath = Get-ChildItem -LiteralPath (Join-Path $androidSdkPath 'build-tools') -Directory |
    Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1 -ExpandProperty FullName
if (-not $buildToolsPath) { throw 'Install Android SDK build-tools.' }
$savedAndroidEnv = @{}
foreach ($name in @('ANDROID_HOME','ANDROID_NDK_HOME','Path','CARGO_APK_RELEASE_KEYSTORE','CARGO_APK_RELEASE_KEYSTORE_PASSWORD')) {
    $savedAndroidEnv[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}
Push-Location $projectRoot
try {
    $env:ANDROID_HOME = $androidSdkPath
    $env:ANDROID_NDK_HOME = $androidNdkPath
    $env:Path = "$env:JAVA_HOME/bin;$env:Path"
    if (-not $SkipBuild) {
        if (-not (Get-Command cargo-apk -ErrorAction SilentlyContinue)) {
            throw 'Install the packager: cargo install cargo-apk --version 0.10.0 --locked'
        }
        if (-not $env:CARGO_APK_RELEASE_KEYSTORE) {
            $env:CARGO_APK_RELEASE_KEYSTORE = Join-Path $env:USERPROFILE '.android/debug.keystore'
            $env:CARGO_APK_RELEASE_KEYSTORE_PASSWORD = 'android'
            Write-Output 'Using the existing Android development signing key.'
        }
        if (-not (Test-Path -LiteralPath $env:CARGO_APK_RELEASE_KEYSTORE)) {
            throw 'Set CARGO_APK_RELEASE_KEYSTORE and CARGO_APK_RELEASE_KEYSTORE_PASSWORD to an existing signing key.'
        }
        cargo apk build --lib --release --target $targetTriple
        if ($LASTEXITCODE -ne 0) { throw 'Android release compilation failed.' }
    }
    $sourceApk = Join-Path $projectRoot 'target/release/apk/playervf.apk'
    if (-not (Test-Path -LiteralPath $sourceApk)) { throw 'The signed APK is missing.' }
    & (Join-Path $buildToolsPath 'apksigner.bat') verify --verbose $sourceApk
    if ($LASTEXITCODE -ne 0) { throw 'APK signature verification failed.' }
    & (Join-Path $buildToolsPath 'zipalign.exe') -c -P 16 4 $sourceApk
    if ($LASTEXITCODE -ne 0) { throw 'APK alignment verification failed.' }
    $apkBadging = & (Join-Path $buildToolsPath 'aapt.exe') dump badging $sourceApk
    $apkBadging | Write-Output
    if ($LASTEXITCODE -ne 0) { throw 'APK manifest inspection failed.' }
    $manifest = Get-Content Cargo.toml -Raw
    $version = [regex]::Match($manifest, '(?m)^version = "([^"]+)"').Groups[1].Value
    $packageLine = $apkBadging | Where-Object { $_ -like 'package:*' } | Select-Object -First 1
    if (-not $version -or $packageLine -notmatch ("versionName='" + [regex]::Escape($version) + "'")) {
        throw 'APK version does not match Cargo.toml; refusing to package a stale APK.'
    }
    if ($packageLine -notmatch "name='com.playervf.rust'") { throw 'Unexpected APK package identity.' }
    if ($apkBadging -match '^application-debuggable') { throw 'Refusing a debuggable APK in the release package.' }
    $nativeCode = $apkBadging | Where-Object { $_ -like 'native-code:*' } | Select-Object -First 1
    if ($nativeCode -notmatch ("'" + [regex]::Escape($expectedAbi) + "'")) {
        throw "APK ABI does not match requested architecture $Architecture."
    }
    New-Item -ItemType Directory -Force -Path dist | Out-Null
    $outputApk = Join-Path $projectRoot "dist/Vey-$version-android-$Architecture.apk"
    Copy-Item -LiteralPath $sourceApk -Destination $outputApk -Force
    $hash = (Get-FileHash -LiteralPath $outputApk -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath "$outputApk.sha256" -Value "$hash  $(Split-Path $outputApk -Leaf)" -Encoding ascii
    Write-Output "APK: $outputApk"
    Write-Output "SHA256: $hash"
} finally {
    Pop-Location
    foreach ($name in $savedAndroidEnv.Keys) {
        [Environment]::SetEnvironmentVariable($name, $savedAndroidEnv[$name], 'Process')
    }
}
