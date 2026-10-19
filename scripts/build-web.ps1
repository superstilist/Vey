param([switch]$Debug)
$ErrorActionPreference='Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    rustup target add wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'WASM target installation failed' }
    $profile = if ($Debug) { 'debug' } else { 'release' }
    $buildArgs = @('build','--lib','--target','wasm32-unknown-unknown')
    if (-not $Debug) { $buildArgs += '--release' }
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) { throw 'WASM compilation failed' }
    if (-not (Get-Command wasm-bindgen -ErrorAction SilentlyContinue)) {
        $lock = Get-Content Cargo.lock -Raw
        $version = [regex]::Match($lock,'name = "wasm-bindgen"\r?\nversion = "([^"]+)"').Groups[1].Value
        cargo install wasm-bindgen-cli --version $version --locked
        if ($LASTEXITCODE -ne 0) { throw 'wasm-bindgen installation failed' }
    }
    wasm-bindgen "target/wasm32-unknown-unknown/$profile/playervf.wasm" --target web --out-dir web/pkg
    if ($LASTEXITCODE -ne 0) { throw 'WASM bindings failed' }
    Write-Output 'Run: cargo run -- --web-server; open http://127.0.0.1:4173'
} finally { Pop-Location }
