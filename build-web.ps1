# Builds the wasm module and the static web UI (web/dist/index.html).
#
# Usage (from the repo root, or anywhere -- the script locates itself):
#   ./build-web.ps1
#
# Equivalent commands, if you'd rather run them by hand:
#   ~/.cargo/bin/cargo build -p dominion-wasm --release --target wasm32-unknown-unknown
#   node web/build.mjs

$ErrorActionPreference = "Stop"
$repoRoot = $PSScriptRoot

$cargo = Join-Path $HOME ".cargo\bin\cargo.exe"
if (-not (Test-Path $cargo)) {
    # Fall back to whatever's on PATH if the usual rustup location isn't there.
    $cargo = "cargo"
}

Write-Host "==> cargo build -p dominion-wasm --release --target wasm32-unknown-unknown"
& $cargo build -p dominion-wasm --release --target wasm32-unknown-unknown --manifest-path (Join-Path $repoRoot "Cargo.toml")
if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }

Write-Host "==> node web/build.mjs"
node (Join-Path $repoRoot "web\build.mjs")
if ($LASTEXITCODE -ne 0) { throw "node web/build.mjs failed (exit $LASTEXITCODE)" }

Write-Host "==> node web/test.mjs (smoke test)"
node (Join-Path $repoRoot "web\test.mjs")
if ($LASTEXITCODE -ne 0) { throw "web/test.mjs reported failures (exit $LASTEXITCODE)" }

Write-Host "`nDone. Open web\dist\index.html directly in a browser."
