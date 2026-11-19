# Windows PowerShell 5.1 — no keys, no trading, no new infrastructure
$ErrorActionPreference = 'Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $root
Write-Host "Testing event parser with mock responses..."
& node --test .\research\base\recent_swap_events.test.mjs
if ($LASTEXITCODE -ne 0) { throw "Mock tests failed; no RPC requests made" }
Write-Host "Sampling bounded recent Base logs via read-only RPC..."
$out=Join-Path $root 'base_recent_events.json'
& node .\research\base\recent_swap_events.mjs > $out
if($LASTEXITCODE -ne 0){Remove-Item $out -ErrorAction SilentlyContinue;throw "RPC denied or unavailable; no usable results saved"}
$report=Get-Content $out -Raw | ConvertFrom-Json
if($report.chainId -ne 8453 -or $report.transactionsSent -ne 0 -or $report.eligibleForProfitClaims -ne $false){throw "Invalid output status"}
Write-Host "Base read-only event sample:"
$report | Select-Object chainId,head,calls,rawLogs,rejectedLogs,validEvents,uniqueBlocks,truncatedAtEventLimit,historyCoverageComplete,eligibleForProfitClaims | Format-List
Write-Host "This sample contains event occurrences only, not backtested profits."
Write-Host "Saved local result: $out"
