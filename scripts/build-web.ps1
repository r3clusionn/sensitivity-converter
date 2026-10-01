# Builds the WebAssembly module and refreshes web/ (the page loads sensconv.wasm and games.json).
Set-Location (Split-Path $PSScriptRoot)
cargo build --release --lib --no-default-features --target wasm32-unknown-unknown
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Copy-Item target/wasm32-unknown-unknown/release/sensconv.wasm web/sensconv.wasm -Force
Copy-Item data/games.json web/games.json -Force
Write-Host ('web/sensconv.wasm: {0} bytes' -f (Get-Item web/sensconv.wasm).Length)
