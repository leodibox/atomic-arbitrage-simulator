# No-cost local test runner for Sandwich Bot's Aave Guarded deployment.
# Windows PowerShell 5.1 compatible; requires Foundry forge on PATH.
# Does NOT ask for a private key, deploy a contract, or spend gas.
$ErrorActionPreference = "Stop"
$owner = "0x49826Aa476d6388fcdccAD7b3661597986a53132"
$root = Split-Path -Parent $PSScriptRoot
$contractDir = Join-Path $root "contracts"
$contract = Join-Path $contractDir "src\FlashLoanArbGuarded.sol"
$test = Join-Path $contractDir "test\GuardedDeploymentSafety.t.sol"

if (-not (Test-Path $contract) -or -not (Test-Path $test)) {
    throw "Run this file from a complete checkout of atomic-arbitrage-simulator."
}
$code = [System.IO.File]::ReadAllText($contract)
$testCode = [System.IO.File]::ReadAllText($test)
if (-not $code.Contains("owner = msg.sender;") -or
    -not $code.Contains("bool public paused = true;") -or
    -not $testCode.Contains($owner)) {
    throw "Contract owner/paused deployment assumptions differ from reviewed source."
}
$foundry = Get-Command forge -ErrorAction SilentlyContinue
if (-not $foundry) {
    Write-Host "BLOCKED: Foundry forge is not installed. Install official Foundry locally:"
    Write-Host "https://github.com/foundry-rs/foundry/releases"
    Write-Host "Then rerun this script. GitHub Actions is not needed."
    exit 2
}
Push-Location $contractDir
try {
    if (-not (Test-Path (Join-Path $contractDir "lib\forge-std\src\Test.sol"))) {
        Write-Host "Installing test dependency forge-std (local checkout only)..."
        & forge install foundry-rs/forge-std --no-git
        if ($LASTEXITCODE -ne 0) { throw "Failed to install local forge-std dependency." }
    }
    Write-Host "Running deployment safety suite: expected owner $owner"
    & forge test --match-contract GuardedDeploymentSafetyTest -vvv
    if ($LASTEXITCODE -ne 0) { throw "Guarded deployment safety suite FAILED; stop before deployment." }
    Write-Host "Running local non-fork regression suite..."
    & forge test --no-match-contract AaveMainnetForkTest -vv
    if ($LASTEXITCODE -ne 0) { throw "Guarded local regression suite FAILED; stop before deployment." }
    Write-Host "PASS: Local Foundry tests completed. This is NOT a mainnet contract audit."
    if (-not $env:HTTPS_URL) {
        Write-Host "NOTE: Real Aave mainnet-fork tests were NOT run (HTTPS_URL unavailable)."
    } else {
        Write-Host "Running canonical Aave mainnet fork tests, RPC kept private..."
        & forge test --match-contract AaveMainnetForkTest -vvv
        if ($LASTEXITCODE -ne 0) { throw "Aave mainnet-fork suite FAILED; stop before deployment." }
    }
    Write-Host "No wallet signature, deployment, unpause or blockchain transaction occurred."
} finally {
    Pop-Location
}
