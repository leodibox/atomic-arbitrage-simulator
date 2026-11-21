# Guarded USDT real-pair Ethereum-mainnet fork audit

Status: **test code prepared, NOT YET EXECUTED**. No live transaction,
no wallet signing, no contract deployed. Do not activate a USDT pair
or remove the legacy Rust USDT exclusion from preflight based on
the existence of tests alone.

## Fork fixtures (real on-chain state)
- Aave V3 mainnet pool: `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`.
- WETH: `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2`.
- USDT: `0xdAC17F958D2ee523a2206206994597C13D831ec7`.
- Uniswap V2 WETH/USDT: `0x0d4a11d5EEaaC28EC3F61D100daF4d40471f1852`.
- SushiSwap V2 WETH/USDT: `0x06da0fd433C1A5D7a4faa01111c044910A184553`.
- Factory, token ordering and reserves are independently verified at the fork.
- Neither MockPair, `deal()`, `mint()` nor synthetic price differences
  are used in these five tests.

## Five independent fork tests
1. 0.05 WETH Aave loan, Uniswap WETH->USDT, Sushi USDT->WETH.
2. 0.05 WETH Aave loan, Sushi WETH->USDT, Uniswap USDT->WETH.
3. 100 USDT Aave loan, Uniswap USDT->WETH, Sushi WETH->USDT.
4. 100 USDT Aave loan, Sushi USDT->WETH, Uniswap WETH->USDT.
5. Real USDT zero/nonzero/zero `approve` success, empty return-data
   and exact on-chain allowance, on fork only.

The four routes compute exact 997/1000 on-chain V2 constant-product quotes
and the Aave premium, then attempt the **real** Guarded flash-loan call
on the fork. Unprofitable routes must revert `Unprofitable` without
changing real pair reserves, Aave aToken liquidity, wallet balances or
contract balance. If a route happens to be profitable at the fork block,
Aave repayment and retained owner-controlled surplus must match exactly.
These tests never assert that the route is profitable or net profitable
after **ETH gas**; they record gas **units** only. There is no automatic
execution policy or net USD claim.

Caveat: in a negative route `_safeApprove(USDT,...)` is not reached.
The separate real-USDT `approve` test verifies empty-return behavior,
not success through the contract's private helper. Further focused tests
are needed before approving USDT for live loans, particularly successful
Aave USDT repayment, price/slippage/gas, token-specific transfer quirks,
and contract guard configuration.

## Windows PowerShell 5.1, no broadcasting

```powershell
cd "$HOME\Downloads\atomic-arbitrage-simulator"
git fetch origin
git switch research/guarded-usdt-mainnet-fork-20261120
git pull --ff-only origin research/guarded-usdt-mainnet-fork-20261120
cd contracts
$env:PATH = "$HOME\Downloads\atomic-arbitrage-simulator\tools\foundry;$env:PATH"
$env:HTTPS_URL = "https://ethereum-rpc.publicnode.com"
Remove-Item Env:MAINNET_FORK_BLOCK -ErrorAction SilentlyContinue
forge test --match-contract GuardedUsdtMainnetForkTest -vvv
```

**Passing this suite is necessary, but not sufficient, to lift the Rust
executor's USDT exclusions or fund/deploy/activate on mainnet.**
GitHub Actions unavailable due account billing lock. No further
Render service or paid endpoint required.
