# Plan de consolidación de PR

Base `origin/main` en `d98c783acdc69b7334c094e022b320d7857ae8e2`. Git 2.52.0. El inventario procede de `refs/remotes/pr/*` y los asuntos proceden de Git. La API de GitHub quedó bloqueada, por lo que el estado, las revisiones y los checks de GitHub siguen sin verificar.

## Clasificación

| PR | Estado | Categoría | Evidencia y motivo |
| --- | --- | --- | --- |
| [#1](#pr-1) `77591d1` | Estado no verificado | LAND | 3 files changed, 358 insertions(+). Ancestro=1, diff=1, merge=0. Documentación y skill de Alchemy; tres archivos, sin código del scanner. |
| [#2](#pr-2) `f02d1ce` | Estado no verificado | KEEP_OPEN_CONFLICT | 9 files changed, 345 insertions(+), 64 deletions(-). Ancestro=1, diff=1, merge=1. Cambios del scanner y del gate de despliegue con cinco conflictos contra main. |
| [#4](#pr-4) `a1a2e51` | Estado no verificado | KEEP_OPEN_CONFLICT | 9 files changed, 341 insertions(+), 29 deletions(-). Ancestro=1, diff=1, merge=1. Escaneo V2 por eventos y sondeo sobre los mismos archivos del scanner; dos conflictos. |
| [#6](#pr-6) `6b5e4aa` | Estado no verificado | KEEP_OPEN_CONFLICT | 2 files changed, 59 insertions(+), 27 deletions(-). Ancestro=1, diff=1, merge=1. Corrección de cotización V3 y filtro de gas; dos conflictos. |
| [#7](#pr-7) `f6bcf8e` | Estado no verificado | NO_NET_CHANGE | 1 file changed, 2 insertions(+), 2 deletions(-). Ancestro=1, diff=1, merge=0. La anotación U256 está absorbida en main; el merge simulado conserva el árbol de main. |
| [#8](#pr-8) `87800fe` | Estado no verificado | NO_NET_CHANGE | 4 files changed, 300 insertions(+), 1 deletion(-). Ancestro=1, diff=1, merge=0. Preflight V2 y contabilidad V3 ya reflejados en el árbol de main; la historia de la ref es distinta. |
| [#9](#pr-9) `a912121` | Estado no verificado | KEEP_OPEN_CONFLICT | 1 file changed, 40 insertions(+), 30 deletions(-). Ancestro=1, diff=1, merge=1. Reporte bruto y tratamiento de gas; conflicto en src/main.rs. |
| [#10](#pr-10) `64581b0` | Estado no verificado | LAND | 11 files changed, 1474 insertions(+), 12 deletions(-). Ancestro=1, diff=1, merge=0. Ejecutor Aave aditivo con pruebas Solidity, verificador Node y workflow propio. |
| [#11](#pr-11) `fee1de7` | Estado no verificado | DOCUMENT_ONLY | 5 files changed, 649 insertions(+). Ancestro=1, diff=1, merge=0. Conservar los resultados de costes V2 en docs/research/; los scripts auxiliares quedan fuera de la consolidación. |
| [#12](#pr-12) `955dbf5` | Estado no verificado | DOCUMENT_ONLY | 5 files changed, 746 insertions(+). Ancestro=1, diff=1, merge=0. Conservar la evidencia sobre pares WETH/USDT y WETH/DAI en docs/research/; los scripts auxiliares quedan fuera. |
| [#13](#pr-13) `3069306` | Estado no verificado | NO_NET_CHANGE | 3 files changed, 175 insertions(+), 22 deletions(-). Ancestro=1, diff=1, merge=0. Modo de solo lectura y selección por dirección absorbidos en main; el merge simulado conserva su árbol. |
| [#14](#pr-14) `be5b78c` | Estado no verificado | DOCUMENT_ONLY | 18 files changed, 1986 insertions(+). Ancestro=1, diff=1, merge=0. Conservar la comparación Morpho/Aave y su evidencia en docs/research/; el arnés experimental queda fuera. |
| [#16](#pr-16) `3f944b5` | Estado no verificado | DOCUMENT_ONLY | 43 files changed, 33785 insertions(+). Ancestro=1, diff=1, merge=0. Conservar los resultados y el bloqueo de replay en docs/research/; no incorporar un segundo árbol long-tail. |
| [#17](#pr-17) `f565d68` | Estado no verificado | LAND | 29 files changed, 31571 insertions(+). Ancestro=1, diff=1, merge=0. Candidato del par long-tail para revisión independiente. Aporta diagnóstico RPC reproducible y queda fuera de la secuencia #1 → #10. |
| [#19](#pr-19) `e384653` | Estado no verificado | DOCUMENT_ONLY | 18 files changed, 882 insertions(+). Ancestro=1, diff=1, merge=0. Conservar la conclusión del gate económico en Base en docs/research/; el código experimental queda fuera. |

`git diff --quiet origin/main...pr/N` compara desde el ancestro común. En #7, #8 y #13 devuelve 1, pero el árbol resultante de `git merge-tree --write-tree origin/main pr/N` coincide exactamente con el de main. Se aplica la regla explícita de no aterrizar trabajo ya absorbido. Ninguna de las 15 puntas es ancestro de main.

#2, #4, #6 y #9 comparten `src/main.rs`. #2, #4 y #6 también comparten `src/arbitrage/state.rs`; #2 y #4 comparten `.env.example`. #16 y #17 comparten 19 rutas, 18 bajo `research/long-tail/` y `docs/ISSUE_15_LONG_TAIL_DISCOVERY.md`. Su merge mutuo simulado sale limpio.

V3 en main usa una aproximación de un solo tick sin cruce de ticks (`src/arbitrage/state.rs:46`). #8 y #13 conservan rutas V3 del scanner y heredan esa limitación. #7 cambia el tipado de esa misma cotización. #10 valida un ejecutor V2 y no depende de la cotización V3 del scanner. #19 usa cotizadores RPC propios, sin dependencia de esa función de Rust.

## Gates locales

Se probaron copias temporales de los árboles de merge simulados. Formato y Clippy son informativos y se registran ambos después de superar los gates bloqueantes. Forge 1.7.1, forge-std `15b2df92a05d4d2f76a06653872fefb287e0e01c` y Solc 0.8.24 verificado por SHA-256 contra el manifiesto oficial. No había `HTTPS_URL` configurada para los forks de Ethereum.

| PR | `cargo +stable test --locked --all-targets` | `forge test -vvv` | `cargo +stable fmt --check` | `cargo +stable clippy --all-targets -- -D warnings` |
| --- | --- | --- | --- | --- |
| #1 | Salida 0. `test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` | No aplica. El diff no toca `contracts/`. | Salida 1. `Diff in /tmp/merge-plan.ymGlmbMy/1/src/arbitrage/cycles.rs:94:` | Salida 101. ``error: could not compile `sandooo` (lib) due to 8 previous errors`` ``error: could not compile `sandooo` (lib test) due to 8 previous errors`` |
| #10 | Salida 0. `test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` | Salida 0. `Ran 5 test suites in 55.88ms (52.65ms CPU time): 29 tests passed, 0 failed, 14 skipped (43 total tests)` | Salida 1. `Diff in /tmp/merge-plan.ymGlmbMy/10/src/arbitrage/cycles.rs:94:` | Salida 101. ``error: could not compile `sandooo` (lib) due to 8 previous errors`` ``error: could not compile `sandooo` (lib test) due to 8 previous errors`` |
| #17 | Salida 0. `test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s` | No aplica. El diff no toca `contracts/`. | Salida 1. `Diff in /tmp/merge-plan.ymGlmbMy/17/src/arbitrage/cycles.rs:94:` | Salida 101. ``error: could not compile `sandooo` (lib) due to 8 previous errors`` ``error: could not compile `sandooo` (lib test) due to 8 previous errors`` |

- **#1**. Sin tests añadidos. No toca `.env.example`, Dockerfile, contratos ni workflows. Las tres cadenas que coinciden con el patrón de URL con clave son marcadores explícitos de ejemplo en `docs/ALCHEMY_SETUP.md`. La revisión de los añadidos no encontró indicios de una credencial real.
- **#10**. Añade y modifica tests Solidity que requieren Forge, además de tests del verificador que se ejecutan con `node --test scripts/check_aave_wiring.test.mjs`. Toca `contracts/` y añade `.github/workflows/guarded-aave-foundry.yml`, que incluye Forge. El workflow Rust de main solo cubre Cargo. No toca `.env.example` ni Dockerfile. La revisión de los añadidos no encontró indicios de una credencial real.
- **#17**. Los tests del diagnóstico son Node y el harness de fork está bajo `research/long-tail/`. Cargo no los ejecuta. El gate de Forge solicitado se activa solo con cambios bajo `contracts/`. No toca `.env.example`, Dockerfile, `contracts/` ni workflows. La revisión de los añadidos no encontró indicios de una credencial real.

El verificador Node de #10 pasó 6 de 6. En #17, el primer intento directo encontró `ERR_MODULE_NOT_FOUND` para `ethers`. Tras `npm ci --ignore-scripts` con el lockfile, `npm test` pasó 24 de 24.

## Secuencia de aterrizaje

1. **#1**. Documentación de Alchemy aislada de contratos y scanner.
2. **#10**. Ejecutor V2 y pruebas aislados del scanner. Confirmar la cobertura de fork antes de confiar en el comportamiento con Aave en mainnet.

La secuencia depende de que GitHub confirme el estado y las revisiones de cada PR, y de los gates bloqueantes arriba. #17 queda reservado a una revisión separada junto con el cierre documental de #16.

## Pendientes

- **#2, #4, #6 y #9**. Conservar para resolución manual si siguen abiertos. El scanner queda fuera de esta consolidación y el plan no acredita un flujo de punta a punta.
- **#7, #8 y #13**. Revisar el cierre administrativo si siguen abiertos, porque sus merges no cambian el árbol de main.
- **#11, #12, #14, #16 y #19**. Seleccionar las conclusiones y evidencias que correspondan para `docs/research/` en una revisión aparte.
- **#17**. Revisar por separado los resultados de su diagnóstico y el solapamiento con #16.
- **#10**. Las pruebas de fork omitidas requieren un RPC Ethereum autorizado. Su omisión no verifica el comportamiento de mainnet.

## Evidencia por referencia

### PR 1

Estado no verificado. SHA `77591d15446c66e7a2507d4821a2631aba34257b`.

Estadística

```text
.agents/skills/alchemy-api/LICENSE.txt |  21 ++++++++++
 .agents/skills/alchemy-api/SKILL.md    | 281 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/ALCHEMY_SETUP.md                  |  56 +++++++++++++++++++++++++++
 3 files changed, 358 insertions(+)
```

Archivos

```text
.agents/skills/alchemy-api/LICENSE.txt
.agents/skills/alchemy-api/SKILL.md
docs/ALCHEMY_SETUP.md
```

Cinco asuntos más recientes

```text
docs: preserve active Render worker settings during Alchemy setup
docs: explain Alchemy Free RPC connection and verification
docs: retain upstream Alchemy skill license
docs: add official Alchemy API agent skill
```


### PR 2

Estado no verificado. SHA `f02d1ceccc340b8bae1ebe6fb17685a1d51949fa`.

Estadística

```text
.env.example                         |   7 +++--
 .github/workflows/rust-discovery.yml |  28 ++++++++++++++++++++
 Dockerfile                           |   6 ++++-
 docs/LIVE_ARBITRAGE_READINESS.md     |  32 ++++++++++++++++++++++
 src/arbitrage/mod.rs                 |   1 +
 src/arbitrage/rpc.rs                 |  22 ++++++++++++++++
 src/arbitrage/state.rs               |  24 +++++++++++++++++
 src/arbitrage/topology.rs            |  93 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 src/main.rs                          | 196 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++------------------------------------------
 9 files changed, 345 insertions(+), 64 deletions(-)
```

Archivos

```text
.env.example
.github/workflows/rust-discovery.yml
Dockerfile
docs/LIVE_ARBITRAGE_READINESS.md
src/arbitrage/mod.rs
src/arbitrage/rpc.rs
src/arbitrage/state.rs
src/arbitrage/topology.rs
src/main.rs
```

Cinco asuntos más recientes

```text
fix: ignore unrealistic token precisions during auto amount probes
fix: reject stale pool Sync events; test out-of-order reserve notifications
build: gate Render release Docker image on locked Rust unit tests
ci: run locked Rust tests on discovery pull requests
fix: fail startup for mismatched allowlisted pool metadata
```

Archivos en conflicto

```text
.env.example
docs/LIVE_ARBITRAGE_READINESS.md
src/arbitrage/state.rs
src/arbitrage/topology.rs
src/main.rs
```

### PR 4

Estado no verificado. SHA `a1a2e51a7d6ea39ab7b09b26430023908a762b00`.

Estadística

```text
.env.example                         |   1 +
 .github/workflows/rust-discovery.yml |  28 ++++++++++++++++++++++++++++
 Dockerfile                           |   6 +++++-
 docs/LIVE_ARBITRAGE_READINESS.md     |  32 ++++++++++++++++++++++++++++++++
 src/arbitrage/mod.rs                 |   1 +
 src/arbitrage/rpc.rs                 |  31 +++++++++++++++++++++++++++++++
 src/arbitrage/state.rs               |  50 ++++++++++++++++++++++++++++++++++++++++++++++++--
 src/arbitrage/topology.rs            |  89 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 src/main.rs                          | 132 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++--------------------------
 9 files changed, 341 insertions(+), 29 deletions(-)
```

Archivos

```text
.env.example
.github/workflows/rust-discovery.yml
Dockerfile
docs/LIVE_ARBITRAGE_READINESS.md
src/arbitrage/mod.rs
src/arbitrage/rpc.rs
src/arbitrage/state.rs
src/arbitrage/topology.rs
src/main.rs
```

Cinco asuntos más recientes

```text
docs: auto-size scanner probes and label gas-cost limitation
ci/docs: Rust build tests and safe-scanner release checklist
ci/docs: Rust build tests and safe-scanner release checklist
build: fail Render Docker image if Rust tests fail
feat: fast Sync-triggered V2 scanning, preserve V3 polling and smarter sized probes
```

Archivos en conflicto

```text
src/arbitrage/state.rs
src/main.rs
```

### PR 6

Estado no verificado. SHA `6b5e4aaf27c095b16f18840ebf3640ed77c67635`.

Estadística

```text
src/arbitrage/state.rs | 50 ++++++++++++++++++++++++++++++++------------------
 src/main.rs            | 36 +++++++++++++++++++++++++++---------
 2 files changed, 59 insertions(+), 27 deletions(-)
```

Archivos

```text
src/arbitrage/state.rs
src/main.rs
```

Cinco asuntos más recientes

```text
fix(sim): apply supplied start-token gas estimate filter and near-miss tracking
fix(v3): correct one_for_zero quote conversion; add regression test
```

Archivos en conflicto

```text
src/arbitrage/state.rs
src/main.rs
```

### PR 7

Estado no verificado. SHA `f6bcf8efdb3a3583fde0128e421ec2bcd803d77b`.

Estadística

```text
src/arbitrage/state.rs | 4 ++--
 1 file changed, 2 insertions(+), 2 deletions(-)
```

Archivos

```text
src/arbitrage/state.rs
```

Cinco asuntos más recientes

```text
fix(v3): annotate U256 intermediate types for Alloy inference
```

Historial de la referencia fuera de main

```text
f6bcf8e fix(v3): annotate U256 intermediate types for Alloy inference
```


### PR 8

Estado no verificado. SHA `87800fef134fd423c38401ceb75118e90be0bfb6`.

Estadística

```text
docs/LIVE_EXECUTION_PREFLIGHT.md |  39 ++++++++++++++++++++++
 src/arbitrage/mod.rs             |   1 +
 src/arbitrage/preflight.rs       | 254 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 src/main.rs                      |   7 +++-
 4 files changed, 300 insertions(+), 1 deletion(-)
```

Archivos

```text
docs/LIVE_EXECUTION_PREFLIGHT.md
src/arbitrage/mod.rs
src/arbitrage/preflight.rs
src/main.rs
```

Cinco asuntos más recientes

```text
docs: define transaction preflight milestones and strict go-live release gates
fix(metrics): count fresh V3 pools instead of mislabeling them stale
feat: expose executable-route preflight and contract calldata preparation
feat: add fail-closed V2 Aave calldata preflight without transaction submission
```

Historial de la referencia fuera de main

```text
87800fe docs: define transaction preflight milestones and strict go-live release gates
9ebc2ca fix(metrics): count fresh V3 pools instead of mislabeling them stale
3c221a4 feat: expose executable-route preflight and contract calldata preparation
87b9e3a feat: add fail-closed V2 Aave calldata preflight without transaction submission
```


### PR 9

Estado no verificado. SHA `a912121aaa27ada2b18f039e894b8736965b5c71`.

Estadística

```text
src/main.rs | 70 ++++++++++++++++++++++++++++++++++++++++------------------------------
 1 file changed, 40 insertions(+), 30 deletions(-)
```

Archivos

```text
src/main.rs
```

Cinco asuntos más recientes

```text
chore: clarify unverified configured ETH-gas amount and remove unused mut
fix: stop misleading native-ETH gas and net-profit mixing; label gross-only route candidates
```

Archivos en conflicto

```text
src/main.rs
```

### PR 10

Estado no verificado. SHA `64581b07fb2eb786ee8221a890233ca210db66c2`.

Estadística

```text
.github/workflows/guarded-aave-foundry.yml |  33 ++++++++++++++
 contracts/src/FlashLoanArbGuarded.sol      | 260 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 contracts/test/AaveMainnetFork.t.sol       | 209 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 contracts/test/FlashLoanArb.t.sol          |  31 ++++++++-----
 contracts/test/FlashLoanArbGuarded.t.sol   | 309 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 contracts/test/Sandooo.t.sol               |  15 ++++++-
 docs/AAVE_INTEGRATION_TEST_RESULTS.md      | 138 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/AAVE_MAINNET_FORK_CERTIFICATION.md    | 155 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/AAVE_WALLET_WIRING.md                 |  93 +++++++++++++++++++++++++++++++++++++++
 scripts/check_aave_wiring.mjs              | 148 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 scripts/check_aave_wiring.test.mjs         |  95 ++++++++++++++++++++++++++++++++++++++++
 11 files changed, 1474 insertions(+), 12 deletions(-)
```

Archivos

```text
.github/workflows/guarded-aave-foundry.yml
contracts/src/FlashLoanArbGuarded.sol
contracts/test/AaveMainnetFork.t.sol
contracts/test/FlashLoanArb.t.sol
contracts/test/FlashLoanArbGuarded.t.sol
contracts/test/Sandooo.t.sol
docs/AAVE_INTEGRATION_TEST_RESULTS.md
docs/AAVE_MAINNET_FORK_CERTIFICATION.md
docs/AAVE_WALLET_WIRING.md
scripts/check_aave_wiring.mjs
scripts/check_aave_wiring.test.mjs
```

Cinco asuntos más recientes

```text
Validate canonical Aave fork payouts and real V2 route rejection
Fix guarded executor compilation and verify atomic Aave payouts
ci: run Aave verifier tests on script or contract changes
docs: distinguish actual Aave fork tests from skipped tests and simulated profitability
ci: test read-only Aave and wallet verification before Foundry compilation
```


### PR 11

Estado no verificado. SHA `fee1de7dce1f72028ac8fb37bf153f60c93eb57e`.

Estadística

```text
docs/V2_MAINNET_COST_RESULTS_20261117.md     | 104 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/V2_READONLY_PROFITABILITY.md            |  41 ++++++++++++++++++++++++++
 docs/research/v2-costs-mainnet-26436857.json | 201 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 scripts/assess_v2_costs.mjs                  | 175 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 scripts/assess_v2_costs.test.mjs             | 128 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 5 files changed, 649 insertions(+)
```

Archivos

```text
docs/V2_MAINNET_COST_RESULTS_20261117.md
docs/V2_READONLY_PROFITABILITY.md
docs/research/v2-costs-mainnet-26436857.json
scripts/assess_v2_costs.mjs
scripts/assess_v2_costs.test.mjs
```

Cinco asuntos más recientes

```text
Record real V2 costs and preserve block-pinned research inputs
test: exercise read-only RPC snapshot against mainnet contract identity and reject mismatches
docs: render command examples and contract addresses correctly
docs: explain independent fee, gas, oracle and V2 read-only research checks
test: cover fee units, stale oracle, negative and theoretical routes
```


### PR 12

Estado no verificado. SHA `955dbf504827931bae40abec9a8624d81f4b553b`.

Estadística

```text
docs/V2_WETH_USDT_DAI_RESEARCH.md                 |  45 ++++++++++++++++
 docs/V2_WETH_USDT_DAI_RESULTS_20261117.md         | 114 ++++++++++++++++++++++++++++++++++++++++
 docs/research/v2-weth-pairs-mainnet-26436921.json | 345 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 scripts/assess_v2_weth_pairs.mjs                  | 151 +++++++++++++++++++++++++++++++++++++++++++++++++++++
 scripts/assess_v2_weth_pairs.test.mjs             |  91 ++++++++++++++++++++++++++++++++
 5 files changed, 746 insertions(+)
```

Archivos

```text
docs/V2_WETH_USDT_DAI_RESEARCH.md
docs/V2_WETH_USDT_DAI_RESULTS_20261117.md
docs/research/v2-weth-pairs-mainnet-26436921.json
scripts/assess_v2_weth_pairs.mjs
scripts/assess_v2_weth_pairs.test.mjs
```

Cinco asuntos más recientes

```text
Document real WETH USDT and DAI pair depth and cost results
docs: define safe WETH/USDT and WETH/DAI V2 research and limitations
test: verify V2 WETH pair discovery, decimals, fees, factory checks and no execution
research: discover and assess canonical WETH/USDT and WETH/DAI V2 routes without execution
```


### PR 13

Estado no verificado. SHA `30693068e9cf8f581a9c37d2b447b17214b949cb`.

Estadística

```text
.env.example                        |   7 +++++--
 docs/SCANNER_DISCOVERY_INTEGRITY.md |  36 ++++++++++++++++++++++++++++++++
 src/main.rs                         | 154 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++------------------
 3 files changed, 175 insertions(+), 22 deletions(-)
```

Archivos

```text
.env.example
docs/SCANNER_DISCOVERY_INTEGRITY.md
src/main.rs
```

Cinco asuntos más recientes

```text
Make read-only scanner default explicit in environment example
docs: clarify after-Aave before-gas route selection
fix(scanner): rank same-route discovery probes by pre-gas after-Aave margin
docs: explain fail-closed scanner selection and direction-aware discovery integrity
feat(scanner): separate V2-compatible gross candidates from V3/other scanner routes
```

Historial de la referencia fuera de main

```text
3069306 Make read-only scanner default explicit in environment example
9005b72 docs: clarify after-Aave before-gas route selection
c6facd8 fix(scanner): rank same-route discovery probes by pre-gas after-Aave margin
8d9dbfc docs: explain fail-closed scanner selection and direction-aware discovery integrity
67699c9 feat(scanner): separate V2-compatible gross candidates from V3/other scanner routes
62fb685 fix(scanner): default to read-only and preserve best per ordered arbitrage route
```


### PR 14

Estado no verificado. SHA `be5b78cff4d9ed7f817eb56d671b2a4e6c4e2750`.

Estadística

```text
docs/MORPHO_V3_MAINNET_CERTIFICATION.md            | 213 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/MORPHO_V3_MAINNET_CERTIFICATION_26437049.md   | 206 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/MORPHO_V3_RESEARCH_PLAN.md                    |  54 +++++++++++++++++++++++++++++++
 docs/research/aave-morpho-comparison-26437081.json | 127 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/research/morpho-v3-fork-output-26437049.txt   | 178 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/research/morpho-v3-fork-output-26437081.txt   | 176 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/research/morpho-v3-fork-results-26437049.json |   1 +
 docs/research/morpho-v3-fork-results-26437081.json |   1 +
 docs/research/morpho-v3-mainnet-26437049.json      |  66 ++++++++++++++++++++++++++++++++++++++
 docs/research/morpho-v3-mainnet-26437081.json      |  66 ++++++++++++++++++++++++++++++++++++++
 docs/research/morpho-v3-observations-26437049.json | 165 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/research/morpho-v3-observations-26437081.json | 165 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/morpho-v3/.gitignore                      |   2 ++
 research/morpho-v3/foundry.toml                    |   8 +++++
 research/morpho-v3/src/ResearchReceiver.sol        | 152 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/morpho-v3/test/MainnetCertification.t.sol | 193 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 scripts/research_morpho_v3_readonly.mjs            | 126 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 scripts/research_morpho_v3_readonly.test.mjs       |  87 ++++++++++++++++++++++++++++++++++++++++++++++++++
 18 files changed, 1986 insertions(+)
```

Archivos

```text
docs/MORPHO_V3_MAINNET_CERTIFICATION.md
docs/MORPHO_V3_MAINNET_CERTIFICATION_26437049.md
docs/MORPHO_V3_RESEARCH_PLAN.md
docs/research/aave-morpho-comparison-26437081.json
docs/research/morpho-v3-fork-output-26437049.txt
docs/research/morpho-v3-fork-output-26437081.txt
docs/research/morpho-v3-fork-results-26437049.json
docs/research/morpho-v3-fork-results-26437081.json
docs/research/morpho-v3-mainnet-26437049.json
docs/research/morpho-v3-mainnet-26437081.json
docs/research/morpho-v3-observations-26437049.json
docs/research/morpho-v3-observations-26437081.json
research/morpho-v3/.gitignore
research/morpho-v3/foundry.toml
research/morpho-v3/src/ResearchReceiver.sol
research/morpho-v3/test/MainnetCertification.t.sol
scripts/research_morpho_v3_readonly.mjs
scripts/research_morpho_v3_readonly.test.mjs
```

Cinco asuntos más recientes

```text
Document same-block Aave versus Morpho financing comparison
Revalidate Morpho V3 evidence at one coherent finalized block
Certify scoped Morpho loans and canonical V3 swaps on mainnet fork
docs: separate verified Morpho 0-fee potential and V3 flash swap from trade approval
test: mock provider and V3 pool identity, fees, liquidity and fail-closed RPC
```


### PR 16

Estado no verificado. SHA `3f944b5e922b7dadfd9e786b5c2991e7baf0aeb9`.

Estadística

```text
docs/ISSUE_15_LONG_TAIL_DISCOVERY.md                                          |   262 ++
 research/long-tail/.gitignore                                                 |     3 +
 research/long-tail/FIRST_SWAP_REPLAY_RESULTS_20261117.md                      |    81 +
 research/long-tail/INTRA_UNISWAP_MEV_EVIDENCE_20261117.md                     |    58 +
 research/long-tail/INTRA_UNISWAP_ROUTE_RESULTS_20261117.md                    |    55 +
 research/long-tail/README.md                                                  |    43 +
 research/long-tail/RECENT_SWAP_TRIGGER_PILOT.md                               |    53 +
 research/long-tail/RECENT_SWAP_TRIGGER_PILOT_RESULTS_20261117.md              |    25 +
 research/long-tail/TRANSACTION_TRIGGERED_EXPERIMENT.md                        |    42 +
 research/long-tail/evidence/first-swap-replay-20261117/final-tests.txt        |    56 +
 research/long-tail/evidence/first-swap-replay-20261117/preservation-tests.txt |    52 +
 research/long-tail/evidence/first-swap-replay-20261117/preservation.json      |    23 +
 research/long-tail/evidence/first-swap-replay-20261117/recheck-ff5005f.json   |    28 +
 research/long-tail/evidence/first-swap-replay-20261117/replay-capability.json |   133 +
 research/long-tail/evidence/first-swap-replay-20261117/selected-block.json    |   490 +++
 research/long-tail/evidence/first-swap-replay-20261117/selected-receipt.json  |    65 +
 research/long-tail/evidence/recent-swap-sample.json                           |   290 ++
 research/long-tail/evidence/run-26437145-full/discovery.json                  |  1860 +++++++++
 research/long-tail/evidence/run-26437145-full/fork-candidates.json            |   106 +
 research/long-tail/evidence/run-26437145-full/fork-output.txt                 |    25 +
 research/long-tail/evidence/run-26437145-full/fork-results.json               |     1 +
 research/long-tail/evidence/run-26437145-full/intra_uniswap_routes.json       |   675 +++
 research/long-tail/evidence/run-26437145-full/inventory.json                  |  7379 +++++++++++++++++++++++++++++++++
 research/long-tail/evidence/run-26437145-full/results.json                    | 20813 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/long-tail/evidence/run-26437145-full/unit-output.txt                 |    31 +
 research/long-tail/fork/foundry.toml                                          |     9 +
 research/long-tail/fork/test/Settlement.t.sol                                 |    90 +
 research/long-tail/package-lock.json                                          |   119 +
 research/long-tail/package.json                                               |     1 +
 research/long-tail/src/chain.mjs                                              |    80 +
 research/long-tail/src/first_swap_replay.mjs                                  |    20 +
 research/long-tail/src/inspect_intra_uniswap.mjs                              |    30 +
 research/long-tail/src/intra_uniswap_routes.mjs                               |   126 +
 research/long-tail/src/math.mjs                                               |    32 +
 research/long-tail/src/probe.mjs                                              |    77 +
 research/long-tail/src/report.mjs                                             |    22 +
 research/long-tail/src/run_first_swap_replay.mjs                              |    45 +
 research/long-tail/src/run_recent_swap_sample.mjs                             |   103 +
 research/long-tail/src/swap_trigger_candidates.mjs                            |   154 +
 research/long-tail/test/first_swap_replay.test.mjs                            |     7 +
 research/long-tail/test/intra_uniswap_routes.test.mjs                         |    94 +
 research/long-tail/test/math.test.mjs                                         |    19 +
 research/long-tail/test/swap_trigger_candidates.test.mjs                      |   108 +
 43 files changed, 33785 insertions(+)
```

Archivos

```text
docs/ISSUE_15_LONG_TAIL_DISCOVERY.md
research/long-tail/.gitignore
research/long-tail/FIRST_SWAP_REPLAY_RESULTS_20261117.md
research/long-tail/INTRA_UNISWAP_MEV_EVIDENCE_20261117.md
research/long-tail/INTRA_UNISWAP_ROUTE_RESULTS_20261117.md
research/long-tail/README.md
research/long-tail/RECENT_SWAP_TRIGGER_PILOT.md
research/long-tail/RECENT_SWAP_TRIGGER_PILOT_RESULTS_20261117.md
research/long-tail/TRANSACTION_TRIGGERED_EXPERIMENT.md
research/long-tail/evidence/first-swap-replay-20261117/final-tests.txt
research/long-tail/evidence/first-swap-replay-20261117/preservation-tests.txt
research/long-tail/evidence/first-swap-replay-20261117/preservation.json
research/long-tail/evidence/first-swap-replay-20261117/recheck-ff5005f.json
research/long-tail/evidence/first-swap-replay-20261117/replay-capability.json
research/long-tail/evidence/first-swap-replay-20261117/selected-block.json
research/long-tail/evidence/first-swap-replay-20261117/selected-receipt.json
research/long-tail/evidence/recent-swap-sample.json
research/long-tail/evidence/run-26437145-full/discovery.json
research/long-tail/evidence/run-26437145-full/fork-candidates.json
research/long-tail/evidence/run-26437145-full/fork-output.txt
research/long-tail/evidence/run-26437145-full/fork-results.json
research/long-tail/evidence/run-26437145-full/intra_uniswap_routes.json
research/long-tail/evidence/run-26437145-full/inventory.json
research/long-tail/evidence/run-26437145-full/results.json
research/long-tail/evidence/run-26437145-full/unit-output.txt
research/long-tail/fork/foundry.toml
research/long-tail/fork/test/Settlement.t.sol
research/long-tail/package-lock.json
research/long-tail/package.json
research/long-tail/src/chain.mjs
research/long-tail/src/first_swap_replay.mjs
research/long-tail/src/inspect_intra_uniswap.mjs
research/long-tail/src/intra_uniswap_routes.mjs
research/long-tail/src/math.mjs
research/long-tail/src/probe.mjs
research/long-tail/src/report.mjs
research/long-tail/src/run_first_swap_replay.mjs
research/long-tail/src/run_recent_swap_sample.mjs
research/long-tail/src/swap_trigger_candidates.mjs
research/long-tail/test/first_swap_replay.test.mjs
research/long-tail/test/intra_uniswap_routes.test.mjs
research/long-tail/test/math.test.mjs
research/long-tail/test/swap_trigger_candidates.test.mjs
```

Cinco asuntos más recientes

```text
Record unchanged replay blocker and fresh preservation checks
Record first Swap transaction-boundary replay access blocker
Preserve verified Swap pilot and correct canonical event topics
docs: bounded recent Swap pilot and fail-closed transaction receipt provenance
research: revalidate saved inventory anchor hash before recent Swap sampling
```


### PR 17

Estado no verificado. SHA `f565d680eecdaeb2e1641e34cc8babd8364f5f45`.

Estadística

```text
docs/ISSUE_15_LONG_TAIL_DISCOVERY.md                                      |   262 ++
 research/long-tail/.gitignore                                             |     3 +
 research/long-tail/README.md                                              |    43 +
 research/long-tail/RPC_CAPABILITY.md                                      |    72 +
 research/long-tail/RPC_CAPABILITY_RESULTS_20261117.md                     |    77 +
 research/long-tail/evidence/rpc-capability-20261117/diagnostic.json       |   144 +
 research/long-tail/evidence/rpc-capability-20261117/install-output.txt    |     2 +
 research/long-tail/evidence/rpc-capability-20261117/regression-output.txt |    41 +
 research/long-tail/evidence/rpc-capability-20261117/unit-output.txt       |    18 +
 research/long-tail/evidence/run-26437145-full/discovery.json              |  1860 +++++++++
 research/long-tail/evidence/run-26437145-full/fork-candidates.json        |   106 +
 research/long-tail/evidence/run-26437145-full/fork-output.txt             |    25 +
 research/long-tail/evidence/run-26437145-full/fork-results.json           |     1 +
 research/long-tail/evidence/run-26437145-full/inventory.json              |  7379 ++++++++++++++++++++++++++++++++++
 research/long-tail/evidence/run-26437145-full/results.json                | 20813 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/long-tail/evidence/run-26437145-full/unit-output.txt             |    31 +
 research/long-tail/fork/foundry.toml                                      |     9 +
 research/long-tail/fork/test/Settlement.t.sol                             |    90 +
 research/long-tail/package-lock.json                                      |   119 +
 research/long-tail/package.json                                           |     1 +
 research/long-tail/src/chain.mjs                                          |    80 +
 research/long-tail/src/math.mjs                                           |    32 +
 research/long-tail/src/probe.mjs                                          |    77 +
 research/long-tail/src/report.mjs                                         |    22 +
 research/long-tail/src/rpc_capability.mjs                                 |   105 +
 research/long-tail/src/rpc_state_capability.mjs                           |     9 +
 research/long-tail/src/run_rpc_preflight.mjs                              |    29 +
 research/long-tail/test/math.test.mjs                                     |    19 +
 research/long-tail/test/rpc_capability.test.mjs                           |   102 +
 29 files changed, 31571 insertions(+)
```

Archivos

```text
docs/ISSUE_15_LONG_TAIL_DISCOVERY.md
research/long-tail/.gitignore
research/long-tail/README.md
research/long-tail/RPC_CAPABILITY.md
research/long-tail/RPC_CAPABILITY_RESULTS_20261117.md
research/long-tail/evidence/rpc-capability-20261117/diagnostic.json
research/long-tail/evidence/rpc-capability-20261117/install-output.txt
research/long-tail/evidence/rpc-capability-20261117/regression-output.txt
research/long-tail/evidence/rpc-capability-20261117/unit-output.txt
research/long-tail/evidence/run-26437145-full/discovery.json
research/long-tail/evidence/run-26437145-full/fork-candidates.json
research/long-tail/evidence/run-26437145-full/fork-output.txt
research/long-tail/evidence/run-26437145-full/fork-results.json
research/long-tail/evidence/run-26437145-full/inventory.json
research/long-tail/evidence/run-26437145-full/results.json
research/long-tail/evidence/run-26437145-full/unit-output.txt
research/long-tail/fork/foundry.toml
research/long-tail/fork/test/Settlement.t.sol
research/long-tail/package-lock.json
research/long-tail/package.json
research/long-tail/src/chain.mjs
research/long-tail/src/math.mjs
research/long-tail/src/probe.mjs
research/long-tail/src/report.mjs
research/long-tail/src/rpc_capability.mjs
research/long-tail/src/rpc_state_capability.mjs
research/long-tail/src/run_rpc_preflight.mjs
research/long-tail/test/math.test.mjs
research/long-tail/test/rpc_capability.test.mjs
```

Cinco asuntos más recientes

```text
Isolate RPC range from age limits and record six-read diagnosis
test: isolate factory fixture to avoid mutating canonical registry
docs: distinguish historical access denial from eth_getLogs range cap before spending
test: historical one-block denial vs range limit without credentials or state edits
research: bounded read-only historical log access diagnostic for Issue 15
```


### PR 19

Estado no verificado. SHA `e3846532e00507b90172bba791cf5d61aac78bc1`.

Estadística

```text
research/base/CANDIDATE_A.md               | 29 +++++++++++++++++++++++++++++
 research/base/ECONOMIC_GATE_20261119.md    | 21 +++++++++++++++++++++
 research/base/README.md                    | 26 ++++++++++++++++++++++++++
 research/base/VALIDATION_20261119.md       | 17 +++++++++++++++++
 research/base/aero_pancake_quotes.mjs      | 90 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/base/aero_pancake_quotes.test.mjs | 56 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/base/cross_venue_quotes.mjs       | 87 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/base/cross_venue_quotes.test.mjs  | 56 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/base/diagnose_call.mjs            | 37 +++++++++++++++++++++++++++++++++++++
 research/base/probe_rpc.mjs                | 49 +++++++++++++++++++++++++++++++++++++++++++++++++
 research/base/recent_swap_events.mjs       | 77 +++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/base/recent_swap_events.test.mjs  | 51 +++++++++++++++++++++++++++++++++++++++++++++++++++
 research/base/run_event_sample.ps1         | 17 +++++++++++++++++
 research/base/run_readonly.ps1             | 34 ++++++++++++++++++++++++++++++++++
 research/base/uniswap_quote.mjs            | 66 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/base/uniswap_quote.test.mjs       | 45 +++++++++++++++++++++++++++++++++++++++++++++
 research/base/verify_pools.mjs             | 82 ++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++
 research/base/verify_pools.test.mjs        | 42 ++++++++++++++++++++++++++++++++++++++++++
 18 files changed, 882 insertions(+)
```

Archivos

```text
research/base/CANDIDATE_A.md
research/base/ECONOMIC_GATE_20261119.md
research/base/README.md
research/base/VALIDATION_20261119.md
research/base/aero_pancake_quotes.mjs
research/base/aero_pancake_quotes.test.mjs
research/base/cross_venue_quotes.mjs
research/base/cross_venue_quotes.test.mjs
research/base/diagnose_call.mjs
research/base/probe_rpc.mjs
research/base/recent_swap_events.mjs
research/base/recent_swap_events.test.mjs
research/base/run_event_sample.ps1
research/base/run_readonly.ps1
research/base/uniswap_quote.mjs
research/base/uniswap_quote.test.mjs
research/base/verify_pools.mjs
research/base/verify_pools.test.mjs
```

Cinco asuntos más recientes

```text
Add one-command Windows read-only Base Swap-event sample with summary
Test bounded Base event sampler, independent topics and fail-closed RPC errors
Add bounded read-only recent Base Swap log sampler with venue-specific event topics
Correct expected read count: 2 chain+block, 6 pool reads, 12 quote calls
Add mock tests for Aerodrome tickSpacing quotes and cross-venue fail-closed behavior
```
