# Morpho Blue + Uniswap V3: certificación en un único bloque

Validación 2026-11-17 · PR #14 DRAFT · rama `research/morpho-v3-feasibility-20261117`.
HEAD de código inspeccionado y probado: `46979e3bca4db624368924c3053d1663545b6f1e`, coincidente con HEAD remoto del PR al inicio. Working tree limpio al empezar. No se encontró un defecto demostrado ni se modificó código.

## Tres conclusiones separadas

1. **Flash loan técnicamente viable para los importes probados.** Morpho entregó WETH/USDC y recuperó exactamente el principal, sin prima observada, con callbacks autenticados y rollback en fallos.
2. **Swaps V3 técnicamente viables en los casos probados.** Ambos pools 1/5 bp están registrados en la factory canónica; salida, pago, balances y precio final coinciden con QuoterV2. Los casos de cruce, callback falso, slippage y pago insuficiente se ejecutaron.
3. **Rentabilidad no demostrada; las cinco rutas muestreadas no son rentables.** Pierden antes de gas; añadir gas o riesgo de reversión no las vuelve positivas. No se autoriza ejecución ni se presenta una viabilidad funcional como prueba de beneficio.

Alcance: lector de Mainnet, capacidad observable del singleton, seis importes de flash loan, swaps V3/callbacks y cinco recorridos económicos. No se certifica un executor de producción.

## Bloque único y reproducibilidad

**Todas las observaciones y tests on-chain de esta ejecución usan exactamente el bloque 26437081.**

- Selección inicial: `finalized` en Mainnet; chain ID verificado **1**.
- Hash: `0x004d642a9bac97acd90b62a46bbfc44245f91273cd2512bb379311fe5c2807e3`.
- Timestamp Unix: `1794946391`; Bogotá: 2026-11-17T15:13:11-05:00.
- Snapshot: todas las consultas eth_call y eth_getCode fijadas al número hexadecimal `0x19365d9`.
- Fork: createSelectFork(HTTPS_URL, CERT_FORK_BLOCK=26437081); no latest ni cambio de bloque entre tests.
- Header adicional consultado por ese mismo número: hash y timestamp comprobados contra snapshot; base fee **0.115566956 gwei**.
- Las dos ejecuciones Foundry (verbose/JSON) usan el mismo bloque. No se reutilizaron balances, liquidez, precios o gas del informe previo.

No había HTTPS_URL exportada. Se usó temporalmente PublicNode HTTPS sin credenciales, configurado solo en el proceso, con NODE_USE_ENV_PROXY=1 para Node y TLS verificado. No se publicaron claves. El acceso reciente funcionó en esta ejecución; el proveedor puede exigir un token para archivo histórico, por lo que el replay futuro no está garantizado. Se depende de una fuente RPC, no de consenso independiente de varios proveedores ni de una atestación criptográfica completa de toda respuesta.

### Evidencia actual

- [Snapshot](research/morpho-v3-mainnet-26437081.json).
- [Resultados Foundry JSON](research/morpho-v3-fork-results-26437081.json).
- [Salida verbose](research/morpho-v3-fork-output-26437081.txt).
- [Observaciones y header](research/morpho-v3-observations-26437081.json).

[Certificación anterior, bloque 26437049](MORPHO_V3_MAINNET_CERTIFICATION_26437049.md) se conserva únicamente como historial; sus datos **no** forman parte de esta medición.

## Tests: resultados exactos

| Ejecución | Aprobadas | Fallidas | Omitidas |
| --- | ---: | ---: | ---: |
| Node: research_morpho_v3_readonly.test.mjs | 8 | 0 | 0 |
| Foundry: MorphoV3MainnetCertificationTest, verbose | 33 | 0 | 0 |
| Foundry: misma suite, JSON | 33 | 0 | 0 |

No se suman las dos ejecuciones Foundry como 66 tests distintos. Herramientas: Node 24.22.1, Foundry 1.7.1, Solc 0.8.24, forge-std `15b2df92a05d4d2f76a06653872fefb287e0e01c`. No hubo fallos que corregir en esta revalidación.

## Morpho: singleton, cash y disponibilidad observada

Contrato oficial: `0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb`, dirección contrastada anteriormente con [documentación oficial](https://docs.morpho.org/developers/contracts/addresses/), código presente al bloque actual y flashLoan ejecutado sobre ese bytecode. No se afirma una comparación completa del runtime con una compilación auditada.

La [implementación canónica](https://github.com/morpho-org/morpho-blue/blob/339a885801972740eb27f51da110a4675b839d42/src/Morpho.sol#L422) transfiere assets al caller, llama onMorphoFlashLoan(uint256,bytes) y recupera exactamente assets con transferFrom. No incluye prima en esa ruta. **No se llama flashLoanFee(): no es una función de esta API.** Tampoco se llama flashLoanSimple ni se supone compatibilidad ERC-3156.

| Asset | Contrato | Cash balanceOf(singleton) | Cantidades prestadas y devueltas en el fork |
| --- | --- | ---: | --- |
| WETH | `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` | 14133.150364749505004435 | 0,01; 1; 10 WETH |
| USDC | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` | 135619681.244758 | 100; 10.000; 1.000.000 USDC |

Cada préstamo comprobó transfer y transferFrom por el principal exacto, callback con el importe, saldo Morpho final igual al inicial, receiver sin subsidio/sobrante y allowance cero. **Prima real observada: cero para esas seis cantidades.** Cash es capacidad observable, no garantía futura ni límite universal certificado. El test cash+1 WETH falla íntegramente.

No se consultaron balances de mercados individuales ni se usó totalSupplyAssets−totalBorrowAssets para limitar flash loans. Callback fallido, devolución insuficiente y llamadas no autorizadas se probaron de forma independiente. Un fallo deja balances/estado/allowances restaurados y permite otra solicitud válida.

## Aave y pools V3 en el mismo bloque

Aave V3 Ethereum Pool `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`: código presente y **FLASHLOAN_PREMIUM_TOTAL = 5 bps**. Es una lectura de prima, no un nuevo préstamo Aave ni modificación de PR #10.

Factory V3 `0x1F98431c8aD98523631AE4a59f267346ea31F984`: getPool(USDC,WETH,fee), código, factory(), tokens y fee verificados.

| Tarifa | Pool | Liquidez activa L raw |
| --- | --- | ---: |
| 1 bp | `0xe0554a476a092703abdb3ef35c80e0d76d32939f` | 308286723960335127 |
| 5 bp | `0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640` | 4190859854518499285 |

L activa no es WETH, USDC, TVL ni liquidez total prestable. Se verifican intercambios reales en ambos sentidos con swap + uniswapV3SwapCallback; no se confunde con flash + uniswapV3FlashCallback. El pago se autentica por pool activo y registro de factory. Getters inventados, callbacks falsos/inactivos, underpayment, minOut excesivo y partial fill dentro de un límite de precio estrecho revierten.

QuoterV2 canónico `0x61fFE014bA17989E743c5F6cB21bF9697530B21e` es la referencia independiente de salida y sqrt final. La pata V2 mixta usa `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc`, con factory/tokens comprobados. No se adaptaron contratos V2 a V3.

Los tests individuales usan financiación sintética de wallets del fork, nunca reservas artificiales. Los roundtrips usan Morpho real en el fork sin float para cubrir pérdidas. No hubo despliegues ni fondos reales.

### Swaps, impacto y comisiones

| Caso | Input raw | Output raw | Ticks inicializados cruzados | Cambio spot USDC/WETH bps | Gas warm local |
| --- | ---: | ---: | ---: | ---: | ---: |
| `testV3_5bpWethToUsdcExactOutputAndPayment` | 10000000000000000 | 17884239 | 1 | -0.002017685 | 208680 |
| `testV3_5bpUsdcToWethExactOutputAndPayment` | 100000000 | 55859216648036764 | 1 | 0.011276282 | 211040 |
| `testV3_1bpWethToUsdcExactOutputAndPayment` | 10000000000000000 | 17883154 | 0 | -0.027433102 | 211458 |
| `testV3_1bpUsdcToWethExactOutputAndPayment` | 100000000 | 55906858986203487 | 0 | 0.153387276 | 210036 |
| `testV3_initializedTickCrossing` | 100000000000000000000 | 178661948859 | 3 | -20.223894105 | 262886 |

Raw USDC=6 decimales; WETH=18. El cambio spot se calcula `(sqrtBefore²/sqrtAfter²−1)×10.000`, no es un beneficio. En rango único se reconcilia fee con L, Δsqrt y feeGrowthGlobal1X128, descontando el protocolo real:

- `testV3_exactSingleRangeFee1bp`: input 1000000000000 wei WETH; comisión exacta 100000000 wei.
- `testV3_exactSingleRangeFee5bp`: input 1000000000000 wei WETH; comisión exacta 500000000 wei.

El caso de cruce valida transición de L y salida exacta contra Quoter. La descomposición independiente de comisión por cada tick cruzado y agotamiento global de liquidez no se presentan como certificados; la insuficiencia probada es dentro del rango permitido por precio.

## Economía y riesgo de reversión

Las barreras ilustrativas son 65 bp V2+V2+Aave, 35 bp V3 5bp+V2+Morpho, 10 bp V3 5bp+5bp+Morpho, 2 bp V3 1bp+1bp+Morpho. No son beneficios ni costes lineales exactos. La factory solo tiene un pool por par/tarifa: las rutas de tarifa repetida son ida/vuelta sobre el mismo pool, no dos mercados. Los pools distintos 1bp/5bp permiten investigar la barrera nominal de 6 bp.

Los márgenes siguientes se extraen de la ejecución secuencial, incluyendo estado modificado para el segundo salto sobre el mismo pool. En pools distintos coinciden con las referencias de cotización. Cuando son negativos, el fixture revierte con el margen codificado y revierte todo el préstamo y swaps.

| Ruta, préstamo 0,1 WETH | Margen realizado antes de gas WETH | Gas warm hasta rechazo | Margen diagnóstico tras gas WETH |
| --- | ---: | ---: | ---: |
| 5bp→V2 | -0.000370919545914475 | 466999 | -0.000891888198799519 |
| 5bp→mismo 5bp | -0.000099975739812267 | 515494 | -0.000675043812228531 |
| 1bp→mismo 1bp | -0.000019999304545897 | 519058 | -0.000599043257593345 |
| 5bp→1bp | -0.000015532787652663 | 535813 | -0.000613268065047891 |
| 1bp→5bp | -0.000107380591871308 | 539595 | -0.000709334943494128 |

El coste diagnóstico usa gas medido×(base fee de este bloque+1 gwei supuesto). No es eth_estimateGas ni beneficio neto certificado: faltan gas intrínseco/calldata, estado cold, refunds, inclusión y política real de fees. El gasleft del test incluye el rechazo protector/catch; no es un recibo de una transacción enviada.

Riesgo de reversión: en las cinco muestras la condición protectora falla por pérdida incluso antes de gas. El rollback conserva tokens, pero una transacción real fallida quemaría gas. No se conoce una probabilidad de inclusión/reversión de mercado y no se inventa una para calcular valor esperado. En general EV requiere p(éxito)×(margen−coste éxito) − p(reversión)×coste fallo, además de selección adversa/MEV. Aquí no existe candidato positivo que justifique esa etapa.

**Todas las rutas NO AUTORIZADAS PARA EJECUCIÓN.** Viabilidad de préstamo y swap no demuestra rentabilidad después de gas/riesgo.

## Seguridad, límites y siguiente paso

Fixture de investigación, no contrato listo para producción: incorpora fallos deliberados y carece de política operativa completa de pausa/caps/deadlines/payout/nonce/pérdida diaria. Faltan auditoría, fuzz/invariantes extensos, tokens adversariales, reentrancia avanzada, runtime/proxy attestation y seguridad de custodia/sender. Los flags conservadores del lector permanecen false: su lectura sola no certifica callbacks o disponibilidad global; la prueba es el fork para cantidades/casos concretos.

No se modificaron scripts, scanner Rust, Render, contratos Aave ni PR #10–#13. No hubo claves privadas, firma, broadcast, despliegue Mainnet, merge ni activación de trading. Continuar únicamente con muestreo tick-exact coherente de tamaños/bloques y simulación cold completa; considerar diseño de executor separado solo tras margen neto positivo y revisión independiente. PR #14 permanece DRAFT.

## Reproducir

HTTPS_URL debe inyectarse de forma segura, sin valor en código, logs o historial. Con Foundry 1.7.1, Solc 0.8.24 verificado y forge-std instalado en contracts/lib/forge-std al commit indicado:

```sh
node --test scripts/research_morpho_v3_readonly.test.mjs
NODE_USE_ENV_PROXY=1 V3_BLOCK_TAG=0x19365d9 node scripts/research_morpho_v3_readonly.mjs
cd research/morpho-v3
CERT_FORK_BLOCK=26437081 forge test --threads 2 --match-contract MorphoV3MainnetCertificationTest -vvv
```

Cada replay debe verificar chain ID, header/hash y acceso al estado de ese mismo bloque. Cambiar a otro finalized exige repetir snapshot, fork y economía completos; no combinar datos. El replay futuro no se ejecutó por anticipado ni se garantiza acceso archive.

## Estado individual de pruebas Node

| Test | Resultado |
| --- | --- |
| cost-only fee math distinguishes 1bp, 5bp and Aave premium | PASSED |
| snapshot authenticates canonical providers, chain, pools and all pinned reads | PASSED |
| wrong chain fails closed | PASSED |
| wrong factory fails closed | PASSED |
| missing 1bp pool does not fabricate 1bp price route | PASSED |
| zero active liquidity is reported but never approved | PASSED |
| wrong fixed historical block fails closed | PASSED |
| HTTP RPC rejects non-HTTPS and sends JSON-RPC POST only | PASSED |

## Estado individual de pruebas de fork

| Test | Resultado |
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
