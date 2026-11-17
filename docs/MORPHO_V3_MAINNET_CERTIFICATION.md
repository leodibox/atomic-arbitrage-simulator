# Morpho Blue + Uniswap V3: certificación funcional de Mainnet

2026-11-17 · Rama `research/morpho-v3-feasibility-20261117` · PR #14 **DRAFT**.
HEAD inicial inspeccionado: `9fa30f8149e23d5250d3f0cf60ea9c76e6c1a5cd`. La rama inicial contenía el lector Node, ocho tests y el plan; no contenía un ejecutor V3 certificado.

**Conclusión:** base técnicamente viable para continuar investigación: Morpho prestó y recuperó exactamente el principal de WETH y USDC; ambos pools V3 realizaron swaps reales en el fork con callbacks autenticados. **No se encontró una ruta rentable:** los cinco roundtrips probados pierden antes de gas. El fixture no es un ejecutor de producción ni una autorización de despliegue/trading.

## Resultados exactos

| Comprobación | Resultado final |
| --- | --- |
| `node --test scripts/research_morpho_v3_readonly.test.mjs` | 8 PASSED, 0 FAILED, 0 SKIPPED |
| `node scripts/research_morpho_v3_readonly.mjs` | completado: chain ID 1, bloque finalizado y pools auténticos |
| Compilación del proyecto Foundry independiente | PASSED, Solc 0.8.24 |
| Suite Morpho/V3 sobre fork Mainnet, salida verbose | 33 PASSED, 0 FAILED, 0 SKIPPED |
| Misma suite en salida JSON | 33 PASSED, 0 FAILED, 0 SKIPPED |

Node 24.22.1; Foundry 1.7.1; forge-std commit `15b2df92a05d4d2f76a06653872fefb287e0e01c`. Se utilizó Solc 0.8.24 previamente verificado por SHA256 `fb03a29a517452b9f12bcf459ef37d0a543765bb3bbc911e70a87d6a37c30d5f` contra el manifiesto oficial ethereum/solc-bin.

- [Snapshot de lectura](research/morpho-v3-mainnet-26437049.json).
- [Resultados individuales Foundry JSON](research/morpho-v3-fork-results-26437049.json).
- [Salida completa de fork con importes y gas](research/morpho-v3-fork-output-26437049.txt).
- [Observaciones estructuradas y metadata del bloque](research/morpho-v3-observations-26437049.json).

## Bloque y proveedor

Bloque finalizado **26437049**, hash `0x752b8f00acb7b104aca58119d5d9707caa192c68b892884c92b3ca72ca91e653`; timestamp `1794946007` (2026-11-17T15:06:47-05:00). Snapshot y fork exitoso utilizan el mismo número. El header consultado separadamente coincide en hash.

No se suministraron claves privadas ni se cambiaron credenciales. Al no existir HTTPS_URL configurada, se utilizó temporalmente PublicNode por HTTPS en variables del proceso. Node necesitó `NODE_USE_ENV_PROXY=1`; TLS permaneció verificado. El lector fija cada eth_call/eth_getCode al número del bloque; el fork usa createSelectFork con ese mismo número. No se firmaron ni enviaron transacciones.

La primera lectura completó el bloque 26437017, pero el primer fork no pudo cargar su estado: HTTP 403, `Archive requests require a personal token`. Resultado de ese intento: **0 PASSED, 1 FAILED (setUp), 0 SKIPPED; los casos no se ejecutaron**. Una nueva lectura finalized obtuvo 26437049 y permitió ejecutar las pruebas. Un RPC público que permite lecturas recientes no garantiza archivo histórico reproducible; el replay futuro requiere acceso al bloque, potencialmente un endpoint archive autenticado introducido de forma segura. No se eludió la restricción.

## Contratos y evidencia oficial

| Contrato | Dirección y comprobación |
| --- | --- |
| Morpho Blue | `0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb`; documentación oficial de addresses contiene esta dirección, código presente y flashLoan ejecutado en fork |
| Aave V3 Ethereum Pool | `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`; código y FLASHLOAN_PREMIUM_TOTAL: **5 bps** |
| Uniswap V3 factory | `0x1F98431c8aD98523631AE4a59f267346ea31F984`; getPool(USDC,WETH,fee) y factory/token0/token1/fee de cada pool |
| WETH | `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` |
| USDC | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` |
| QuoterV2 de referencia | `0x61fFE014bA17989E743c5F6cB21bF9697530B21e`; código presente, quoteExactInputSingle real y salida/sqrt final comparados con swap |
| Uniswap V2 USDC/WETH, control mixto | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc`; token order, factory canónica y registro getPair comprobados por fixture |

Fuentes: [Morpho addresses](https://docs.morpho.org/developers/contracts/addresses/), [Morpho.sol oficial](https://github.com/morpho-org/morpho-blue/blob/339a885801972740eb27f51da110a4675b839d42/src/Morpho.sol#L422), [Uniswap V3 Pool](https://github.com/Uniswap/v3-core/blob/main/contracts/UniswapV3Pool.sol), [deployments V3 Ethereum](https://docs.uniswap.org/contracts/v3/reference/deployments/ethereum-deployments).

La implementación oficial Morpho transfiere assets al caller, invoca `onMorphoFlashLoan(uint256,bytes)` y recupera esos mismos assets con transferFrom. No se inventó un getter de fee ni una API ERC-3156/Aave: son interfaces diferentes. Leer documentación y probar comportamiento no equivale a recompilar y atestar todo el runtime bytecode.

## Morpho: cash y capacidad probada

| Asset | Saldo del singleton al bloque | Importes efectivamente prestados y devueltos |
| --- | ---: | --- |
| WETH | 14119.707180402400513058 | 0,01; 1; 10 WETH |
| USDC | 135638714.701115 | 100; 10.000; 1.000.000 USDC |

Cada caso comienza con receiver sin saldo del asset, observa el principal exacto dentro del callback, espera transfer y transferFrom por el mismo importe, comprueba saldo Morpho sin variación, receiver sin sobrante ni subsidio y allowance final cero. **Comisión observada: 0 para esos seis préstamos**. Los saldos no se presentan como límite universal ejecutable: se certifican únicamente esos importes en ese estado. WETH cash+1 se rechazó sin perder fondos.

Callbacks que revierten o aprobaciones insuficientes revierten préstamo, balances, estado del receiver y allowance para WETH/USDC; el receiver puede volver a usar un préstamo normal después. Se rechazan callbacks de terceros y llamadas desde Morpho fuera de una solicitud activa. El fixture es owner-only y ata token/importe a la solicitud.

## Pools V3 y swaps

| Tarifa | Pool canónico USDC/WETH | Liquidez activa raw |
| --- | --- | ---: |
| 1 bp | `0xe0554a476a092703abdb3ef35c80e0d76d32939f` | 308286723960335127 |
| 5 bp | `0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640` | 4191224594395590429 |

`liquidity()` es L activa en el rango actual, no saldo de tokens, TVL ni capacidad prestable. No se dedujo una capacidad global sumando esos valores.

La suite usa `swap(recipient,zeroForOne,amountSpecified,sqrtPriceLimitX96,data)` y **uniswapV3SwapCallback**, no `flash`. En cada swap se comprueban exactamente los deltas devueltos, la salida de QuoterV2 y su sqrtPriceX96 final, el input pagado al pool y los cambios opuestos de balances pool/receiver.

El receiver solo acepta el pool activo y verifica getPool(token0,token1,fee) contra msg.sender; un contrato que se inventa getters canónicos también es rechazado. Los callbacks falsificados o fuera de swap fallan. Pagar un wei menos causa IIA y revierte; un mínimo imposible causa Slippage y revierte. Una sqrtPriceLimit extremadamente estrecha agota la liquidez disponible **dentro de ese rango permitido**, causa partial fill y el fixture revierte íntegramente. No se afirma haber agotado toda la liquidez global del pool.

No se adaptó ni modificó FlashLoanArbGuarded ni el ejecutor V2. Sus llamadas no son compatibles con V3. Tampoco se certificó `UniswapV3Pool.flash()` ni su uniswapV3FlashCallback: es un producto distinto y cobra la tarifa de flash del pool; aquí el préstamo es Morpho y el intercambio es swap.

Fondos de los tests individuales de swap: ETH de wallet creado con vm.deal y depositado en WETH, o saldo USDC de wallet con deal. Son fixtures funcionales **sintéticos de financiación**, nunca reservas alteradas ni una oportunidad fabricada. Los roundtrips económicos usan exclusivamente el préstamo real de Morpho en el fork y no reciben float para cubrir pérdidas.

### Salida, impacto y gas por swap

La variación del precio spot USDC/WETH se calcula como `(sqrtBefore² / sqrtAfter² − 1) × 10.000` bps. Es el cambio de precio marginal, no una predicción de rentabilidad; las salidas incluyen comisión y redondeo.

| Caso | Entrada raw | Salida raw | Ticks inicializados cruzados | Cambio spot USDC/WETH bps | Gas local warm |
| --- | ---: | ---: | ---: | ---: | ---: |
| `testV3_5bpWethToUsdcExactOutputAndPayment` | 10000000000000000 | 17905188 | 1 | -0.002018691 | 208674 |
| `testV3_5bpUsdcToWethExactOutputAndPayment` | 100000000 | 55793859268485271 | 1 | 0.011268702 | 210711 |
| `testV3_1bpWethToUsdcExactOutputAndPayment` | 10000000000000000 | 17919256 | 0 | -0.027460779 | 208644 |
| `testV3_1bpUsdcToWethExactOutputAndPayment` | 100000000 | 55794221724763267 | 0 | 0.153232680 | 210700 |
| `testV3_initializedTickCrossing` | 100000000000000000000 | 178871353537 | 3 | -20.160233983 | 263568 |

USDC raw tiene 6 decimales y WETH raw 18. En la prueba de cruce, 100 WETH se cambian por 178.871,353537 USDC y se cruzan **3 ticks inicializados** con cambio real de L; precio y salida coinciden exactamente con QuoterV2.

Para 0,000001 WETH en un único rango, la comisión exacta observada es **100.000.000 wei** a 1 bp y **500.000.000 wei** a 5 bp. Se deriva input neto de L y Δsqrt redondeado hacia arriba, y se reconcilia la comisión LP con feeGrowthGlobal1X128 después de descontar el feeProtocol real del pool. No se aplica esa fórmula de un rango a cruces de ticks. En los swaps con cruces, la salida total se compara con el Quoter, pero una descomposición independiente paso a paso de sus comisiones queda pendiente.

## Economía: barreras frente a recorridos reales

Las sumas de tarifas son barreras ilustrativas, no una garantía exacta de coste lineal: las bases del segundo swap cambian, hay redondeo, precio de cada venue e impacto.

| Escenario | Barrera solicitada | Existencia/viabilidad del recorrido |
| --- | ---: | --- |
| V2 + V2 + Aave | 65 bp = 30+30+5 | baseline de comparación; no se reejecutan ni modifican PR #10–#13 |
| V3 5 bp + V2 + Morpho | 35 bp | ambos pools reales existen; ejecución atómica de 0,1 WETH probada, revierte por pérdida |
| V3 5 bp + V3 5 bp + Morpho | 10 bp | la factory canónica tiene **un solo pool** por tokens/tarifa; probado ida/vuelta sobre el mismo pool, no dos venues independientes |
| V3 1 bp + V3 1 bp + Morpho | 2 bp | misma restricción de unicidad; ida/vuelta real sobre el mismo pool, sin arbitraje entre venues |
| V3 1 bp + V3 5 bp + Morpho | 6 bp | dos pools distintos reales; ambas direcciones probadas y negativas |

No se inventó un segundo pool de la misma tarifa. No se afirma existencia de un segundo venue V3 1bp/5bp independiente fuera de esta factory.

Cada recorrido usa 0,1 WETH y ejecuta ambos swaps y el callback Morpho real en el fork. Cuando pierde, el receiver revierte con el margen realizado codificado y todo vuelve al estado inicial. En pools distintos el margen coincide con las cotizaciones de referencia; para el mismo pool se usa el resultado de la ejecución secuencial, pues el segundo salto no puede cotizarse contra el estado original.

### Márgenes y gas

Base fee del bloque: **0.121083659 gwei**. La columna después de gas es solo un cálculo diagnóstico: margen realizado menos `gas medido warm × (base fee + 1 gwei de propina asumida)`. Excluye gas intrínseco/calldata, efectos cold, refunds e inclusión; **no es eth_estimateGas ni margen neto certificado**. Como todos pierden antes de gas, ningún coste positivo puede volverlos rentables.

| Ruta Morpho, 0,1 WETH | Margen real antes de gas WETH | Gas medido local | Margen tras coste diagnóstico WETH |
| --- | ---: | ---: | ---: |
| 5bp→V2 | -0.000254212108632923 | 466993 | -0.00077775032980031 |
| 5bp→mismo 5bp | -0.000099975441405755 | 515159 | -0.000677511778092536 |
| 1bp→mismo 1bp | -0.000019999915628694 | 518700 | -0.000601506009551994 |
| 5bp→1bp | -0.000100089065448169 | 536477 | -0.000701524663577512 |
| 1bp→5bp | -0.000022832809004508 | 539238 | -0.00062736371911635 |

La medición usa gasleft alrededor de calldata válida en el fork; en los roundtrips mide la ruta hasta el revert protector y el catch/diagnóstico del test. Los swaps individuales exitosos sí completan pago/salida. Los costes de despliegue/setup del fixture no están dentro de estos deltas; las cotizaciones previas y lecturas pueden calentar estado. No se obtuvo una transacción rentable apta para estimateGas de producción, ni se envió ninguna. Los préstamos simples también registran gas por asset/importe en la evidencia adjunta; son mediciones de integración, no tarifas inmutables.

## Errores encontrados y corregidos

1. **Proveedor:** primer fork bloqueado por archivo con token requerido. Se conservó el resultado fallido y se repitió sobre un finalized reciente coherente; las pruebas no se declararon omitidas/aprobadas durante el fallo de setup.
2. **Primer fixture de certificación:** ejecución inicial funcional: 29 PASSED, 3 FAILED, 0 SKIPPED. Asumía que 10 WETH en 1bp cruzarían ticks, que 0,01 WETH en 5bp se quedarían en un rango, y una expresión de comisión no ensanchaba explícitamente su aritmética. Se seleccionaron casos reales que cumplen las precondiciones, se validan los cambios reales de L/word y se usan uint256 explícitos. No se relajaron comprobaciones ni se manipularon reservas.
3. **Cotización del mismo pool:** el borrador del fixture calculaba el segundo salto contra estado original. Se corrigió para registrar el margen de ejecución secuencial en el revert y comparar Quoter contra ejecución únicamente para pools distintos. No se reportan como definitivos los márgenes de ese borrador.
4. Los ocho tests del lector original pasaron; no se demostró un defecto del scanner o los contratos de producción y no se modificaron.

## Seguridad pendiente y alcance de la certificación

- Fixture solo de investigación: contiene modos deliberados de fallo, configuración de pruebas, sin implementación completa de política de token/pausa/caps/nonce/payout/recipient ni auditoría. **No desplegar ni reutilizar tal cual como executor.**
- Autenticación de callback, repayment, approvals exactas y rollback se probaron para assets/casos concretos. Faltan fuzz/invariantes extensos, reentrancia adversarial avanzada, tokens no estándar y revisión independiente de todo callback/state machine.
- Balance Morpho no prueba préstamo universal; las seis cantidades sí se ejecutaron sin comisión en ese bloque. Liquidez activa V3 no prueba recorrido completo para cualquier tamaño o precio límite. No se certificó agotamiento global de ticks/liquidez.
- El límite de slippage fue comprobado, pero una política de deadlines, exposición máxima, pérdida diaria, oracle sanity/MEV, chain/code pinning, custodia de signer y permisos operativos queda fuera del fixture.
- Lectura de código y documentación oficial no equivale a una atestación reproducible de runtime compilado ni a verificar proxies/implementaciones y riesgos de gobernanza completos.
- Proveedor público único, bloque histórico y quotes reales no garantizan estado en inclusión. Gas, liquidez, USDC transfer rules/blacklist, comisiones efectivas y competencia MEV pueden cambiar. Un revert real consume gas.
- El lector conserva sus flags conservadores: feeWasVerifiedFromContract=false y callbackIntegrationVerified=false porque el lector solo lee. La evidencia de comportamiento 0-fee/callback es la suite independiente, limitada a su bloque y cantidades; no se convierte en autorización global.

## Reproducción

Instalar Foundry 1.7.1 y Solc 0.8.24 con verificación; instalar forge-std en `contracts/lib/forge-std` y fijarlo al commit indicado. El proyecto independiente importa esa dependencia, sin compilar ni modificar los ejecutores Aave/V2. Guardar HTTPS_URL en un binding seguro, nunca en archivos versionados ni en el comando/historial con su valor secreto.

```sh
node --test scripts/research_morpho_v3_readonly.test.mjs
NODE_USE_ENV_PROXY=1 V3_BLOCK_TAG=0x19365b9 node scripts/research_morpho_v3_readonly.mjs
cd research/morpho-v3
CERT_FORK_BLOCK=26437049 forge test --threads 2 --match-contract MorphoV3MainnetCertificationTest -vvv
```

En este workspace el binario nativo está en `/workspace/.tools/bin/forge` y el compiler verificado en `/workspace/.tools/bin/solc-0.8.24`; activar PATH y FOUNDRY_SOLC si no hay instalación estándar. Un replay puede quedar bloqueado por permisos archive aunque estas ejecuciones sí finalizaron. No reemplazarlo por tests sin RPC ni presentar saltos como pases.

## Siguiente paso

Mantener PR #14 DRAFT. Continuar con muestreo coherente de tamaños y bloques de los **dos pools distintos 1bp/5bp**, cotización tick-exact, gas cold y costes de inclusión medidos en una simulación de calldata completa. Solo si aparece margen después de todos esos costes considerar, en una tarea separada, diseño y auditoría de un executor V3 con Morpho. Los resultados actuales no justifican desplegar ni operar.

## Tests individuales Node

| Test | Estado |
| --- | --- |
| cost-only fee math distinguishes 1bp, 5bp and Aave premium | PASSED |
| snapshot authenticates canonical providers, chain, pools and all pinned reads | PASSED |
| wrong chain fails closed | PASSED |
| wrong factory fails closed | PASSED |
| missing 1bp pool does not fabricate 1bp price route | PASSED |
| zero active liquidity is reported but never approved | PASSED |
| wrong fixed historical block fails closed | PASSED |
| HTTP RPC rejects non-HTTPS and sends JSON-RPC POST only | PASSED |

## Tests individuales de fork, resultado final

| Test | Estado |
| --- | --- |
| `testEconomy_1bpThen5bpMorpho()` | PASSED |
| `testEconomy_1bpThenSame1bpMorpho()` | PASSED |
| `testEconomy_5bpThen1bpMorpho()` | PASSED |
| `testEconomy_5bpThenSame5bpMorpho()` | PASSED |
| `testEconomy_5bpThenV2Morpho()` | PASSED |
| `testMorpho_USDC_100()` | PASSED |
| `testMorpho_USDC_10000()` | PASSED |
| `testMorpho_USDC_1000000()` | PASSED |
| `testMorpho_WETH_001()` | PASSED |
| `testMorpho_WETH_1()` | PASSED |
| `testMorpho_WETH_10()` | PASSED |
| `testMorpho_callbackFailureAtomicUsdc()` | PASSED |
| `testMorpho_callbackFailureAtomicWeth()` | PASSED |
| `testMorpho_excessCashRejected()` | PASSED |
| `testMorpho_forgedAndInactiveCallbacksRejected()` | PASSED |
| `testMorpho_repaymentFailureAtomicUsdc()` | PASSED |
| `testMorpho_repaymentFailureAtomicWeth()` | PASSED |
| `testReceiver_ownerAuthentication()` | PASSED |
| `testV3_1bpUsdcToWethExactOutputAndPayment()` | PASSED |
| `testV3_1bpWethToUsdcExactOutputAndPayment()` | PASSED |
| `testV3_5bpUsdcToWethExactOutputAndPayment()` | PASSED |
| `testV3_5bpWethToUsdcExactOutputAndPayment()` | PASSED |
| `testV3_canonical1bpIdentity()` | PASSED |
| `testV3_canonical5bpIdentity()` | PASSED |
| `testV3_exactSingleRangeFee1bp()` | PASSED |
| `testV3_exactSingleRangeFee5bp()` | PASSED |
| `testV3_fabricatedCanonicalGettersRejected()` | PASSED |
| `testV3_forgedAndInactiveCallbacksRejected()` | PASSED |
| `testV3_initializedTickCrossing()` | PASSED |
| `testV3_insufficientLiquidityWithinPriceLimitRollsBack()` | PASSED |
| `testV3_nonCanonicalPoolRejected()` | PASSED |
| `testV3_slippageFailureRollsBack()` | PASSED |
| `testV3_underpaymentRollsBack()` | PASSED |
