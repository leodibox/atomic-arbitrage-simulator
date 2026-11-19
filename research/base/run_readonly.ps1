# Run Base proof-of-price locally, without new hosting or any transaction
# From the repository root: powershell -NoProfile -File .\research\base\run_readonly.ps1
# Requires Node.js >=20 and an internet connection. Does not read keys or wallets.
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $root
try {
  $nodeVersion = & node --version
  if ($LASTEXITCODE -ne 0) { throw "Node.js is unavailable" }
} catch { throw "Node.js is required; no RPC operations have been attempted" }
Write-Host "Node: $nodeVersion"
Write-Host "Running all local mock tests first (not live evidence)..."
& node --test research/base/verify_pools.test.mjs research/base/uniswap_quote.test.mjs research/base/cross_venue_quotes.test.mjs
if ($LASTEXITCODE -ne 0) { throw "Local tests failed; refusing network reads" }
Write-Host "Running bounded, read-only cross-venue Base quotes (max 16 RPC calls)..."
$out = Join-Path $root 'base_quote_result.json'
# The comparator only prints public pool data and quote amounts; it never reads keys.
& node research/base/cross_venue_quotes.mjs > $out
if ($LASTEXITCODE -ne 0) {
  Remove-Item $out -ErrorAction SilentlyContinue
  throw "Base RPC check failed. No valid price result, no transactions sent."
}
try {
  $report = Get-Content $out -Raw | ConvertFrom-Json
  if ($report.chainId -ne 8453 -or !$report.canonicalFactoryPoolsVerified -or $report.txSubmitted -ne 0) { throw "Invalid or unverified Base result" }
  if ($report.results.Count -ne 6) { throw "Expected six spot quote results" }
  Write-Host "Base RPC response collected. Preliminary quotes:"
  $report.results | Select-Object first,second,amountInUSDC,amountOutUSDC,pnlBeforeGasUSDC | Format-Table
  Write-Host "Only pre-gas spot quotes. NOT verified execution or net profit."
  Write-Host "Saved: $out"
} catch {
  Remove-Item $out -ErrorAction SilentlyContinue
  throw "Unexpected output; quote file removed, no trade approved."
}