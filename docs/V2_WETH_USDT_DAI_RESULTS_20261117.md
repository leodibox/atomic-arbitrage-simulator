# WETH/USDT y WETH/DAI: resultados reales — 2026-11-17

Rama exclusiva: `research/v2-weth-usdt-dai-depth-20261117`, PR #12 DRAFT. Commit de código evaluado: `049cba1a04729475d390fab23c663220250d95ab`. No se detectaron defectos demostrados: no se modificaron scripts ni lógica de producción.

**0 de 20 márgenes positivos después del gas estimado. Todas las observaciones NO AUTORIZADAS PARA EJECUCIÓN.** Son cálculos sobre reservas, no swaps ejecutados ni resultados realizados.

## Evidencia y reproducción

- `node --test scripts/assess_v2_weth_pairs.test.mjs`: **10 PASSED, 0 FAILED, 0 SKIPPED**, Node 24.22.1.
- `node scripts/assess_v2_weth_pairs.mjs`: ejecución completada, chain ID **1**, ambos mercados disponibles, 10 resultados por mercado.
- No había HTTPS_URL exportada, .env local ni bindings de variables/secretos. Se configuró temporalmente un endpoint público PublicNode HTTPS solo en el proceso, como permite la guía del repositorio. No se utilizó ni descubrió un endpoint privado autenticado; no se imprimieron credenciales ni se cambiaron claves/configuración del entorno.
- En este runtime se necesitó `NODE_USE_ENV_PROXY=1`, conservando TLS. El RPC solo recibió eth_chainId, eth_getBlockByNumber, eth_getCode y eth_call.
- Selector: `finalized`; bloque **26436921**, hash `0xfc593213a2c1067d3f125f96e77cf8797e505d2d2bb2875e557233756e9ba908`. Todas las consultas de estado quedaron fijadas al número hexadecimal del mismo bloque.
- Fecha del bloque en Bogotá: **2026-11-17T14:41:11-05:00**; timestamp Unix `1794944471`.

[JSON completo: snapshot, contratos, reservas y 20 resultados](research/v2-weth-pairs-mainnet-26436921.json).

Para repetir, configurar HTTPS_URL de forma segura fuera del historial/comando y ejecutar:

```sh
node --test scripts/assess_v2_weth_pairs.test.mjs
NODE_USE_ENV_PROXY=1 V2_BLOCK_TAG=0x1936539 node scripts/assess_v2_weth_pairs.mjs
```

El replay histórico requiere que el proveedor admita ese estado. Esta ejecución consultó finalized; el replay histórico adicional no se ejecutó ni se presenta como aprobado. Otra muestra finalized puede producir resultados diferentes.

## Verificación de contratos y reservas

Factories consultadas por getPair(WETH, token): Uniswap V2 `0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f`; SushiSwap V2 `0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac`. Los cuatro pares no son cero y tienen bytecode. Se comprobaron factory(), token0(), token1(), reservas positivas y orden de tokens. No se dedujeron direcciones por nombre ni se alteraron balances/reservas.

Tokens: WETH `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` (18 decimales), USDT `0xdAC17F958D2ee523a2206206994597C13D831ec7` (6), DAI `0x6B175474E89094C44Da98b954EedeAC495271d0F` (18). Código y decimales consultados al bloque.

| Mercado | Venue | Contrato del par | Reserva WETH | Reserva token |
| --- | --- | --- | ---: | ---: |
| WETH/USDT | Uniswap V2 | `0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852` | 3462.040471724921196218 | 6201177.18274 |
| WETH/USDT | SushiSwap V2 | `0x06da0fd433c1a5d7a4faa01111c044910a184553` | 148.4633147998024329 | 265933.477058 |
| WETH/DAI | Uniswap V2 | `0xa478c2975ab1ea89e8196811f51a7b7ade33eb11` | 1830.476142977840810469 | 3275930.395735257836625984 |
| WETH/DAI | SushiSwap V2 | `0xc3d03e4f041fd4cd388c549ee2a29a9e5075882f` | 47.211827528598825248 | 84514.714913344691610065 |

## Profundidad y supuestos de coste

Profundidad = mínimo de reservas WETH entre venues, no TVL ni tamaño ejecutable garantizado. La proporción entrada/profundidad es un indicador de tamaño, no un cálculo completo de slippage.

| Mercado | Pata limitante | Profundidad WETH | 0,01 WETH / profundidad | 1 WETH / profundidad |
| --- | --- | ---: | ---: | ---: |
| WETH/USDT | SushiSwap V2 | 148.4633147998024329 | 0.006736% | 0.673567% |
| WETH/DAI | SushiSwap V2 | 47.211827528598825248 | 0.021181% | 2.118113% |

Aave V3 Pool `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`: código comprobado y FLASHLOAN_PREMIUM_TOTAL consultado: **5 bps (0,05%)**. No se verificó disponibilidad de flashloan de WETH ni estado operativo de su reserva.

- Base fee del bloque: **0.129103107 gwei**.
- Supuestos: **400.000 gas**, propina **1 gwei**, buffer **20%**.
- Coste sin buffer: **0.0004516412428 ETH**.
- Coste con buffer, redondeado conservadoramente: **0.00054196949136 ETH** por intento, descontado en unidades WETH.
- Cada swap aplica 997/1000 y redondeo entero hacia abajo; la prima se redondea hacia arriba. Los cinco tamaños son 0,01, 0,05, 0,1, 0,5 y 1 WETH.

No se requiere Chainlink para comparar estos costes: entrada/salida y prima están en WETH wei, mientras el gas se paga en ETH wei, con paridad de denominación 1:1. Esto no prueba disponibilidad de ETH para gas ni autoriza envolver/desenvolver fondos.

## Veinte resultados

Valores en WETH. Antes de gas = salida − principal − prima Aave, tras las dos comisiones y el impacto V2. Después de gas = anterior − coste hipotético con buffer. Todos incluyen `approvedForExecution=false`.

| Mercado | Ruta | Entrada | Salida | Prima | Antes de gas | Después de gas | Ejecución |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| USDT | Uni → Sushi | 0.01 | 0.009939124505571475 | 0.000005 | -0.000065875494428525 | -0.000607844985788525 | NO AUTORIZADA |
| USDT | Uni → Sushi | 0.05 | 0.049681745909214464 | 0.000025 | -0.000343254090785536 | -0.000885223582145536 | NO AUTORIZADA |
| USDT | Uni → Sushi | 0.1 | 0.099328822856663529 | 0.00005 | -0.000721177143336471 | -0.001263146634696471 | NO AUTORIZADA |
| USDT | Uni → Sushi | 0.5 | 0.495261693780089881 | 0.00025 | -0.004988306219910119 | -0.005530275711270119 | NO AUTORIZADA |
| USDT | Uni → Sushi | 1 | 0.987088901795522831 | 0.0005 | -0.013411098204477169 | -0.013953067695837169 | NO AUTORIZADA |
| USDT | Sushi → Uni | 0.01 | 0.009939664909306335 | 0.000005 | -0.000065335090693665 | -0.000607304582053665 | NO AUTORIZADA |
| USDT | Sushi → Uni | 0.05 | 0.049684411071034852 | 0.000025 | -0.000340588928965148 | -0.000882558420325148 | NO AUTORIZADA |
| USDT | Sushi → Uni | 0.1 | 0.099334054684270446 | 0.00005 | -0.000715945315729554 | -0.001257914807089554 | NO AUTORIZADA |
| USDT | Sushi → Uni | 0.5 | 0.495283936648766501 | 0.00025 | -0.004966063351233499 | -0.005508032842593499 | NO AUTORIZADA |
| USDT | Sushi → Uni | 1 | 0.987123722184253222 | 0.0005 | -0.013376277815746778 | -0.013918247307106778 | NO AUTORIZADA |
| DAI | Uni → Sushi | 0.01 | 0.009935406050234703 | 0.000005 | -0.000069593949765297 | -0.000611563441125297 | NO AUTORIZADA |
| DAI | Uni → Sushi | 0.05 | 0.049634168436351646 | 0.000025 | -0.000390831563648354 | -0.000932801055008354 | NO AUTORIZADA |
| DAI | Uni → Sushi | 0.1 | 0.099161390132935756 | 0.00005 | -0.000888609867064244 | -0.001430579358424244 | NO AUTORIZADA |
| DAI | Uni → Sushi | 0.5 | 0.491570205494910975 | 0.00025 | -0.008679794505089025 | -0.009221763996449025 | NO AUTORIZADA |
| DAI | Uni → Sushi | 1 | 0.972750041227365842 | 0.0005 | -0.027749958772634158 | -0.028291928263994158 | NO AUTORIZADA |
| DAI | Sushi → Uni | 0.01 | 0.009940476010861212 | 0.000005 | -0.000064523989138788 | -0.000606493480498788 | NO AUTORIZADA |
| DAI | Sushi → Uni | 0.05 | 0.049659362987639425 | 0.000025 | -0.000365637012360575 | -0.000907606503720575 | NO AUTORIZADA |
| DAI | Sushi → Uni | 0.1 | 0.099211392506517405 | 0.00005 | -0.000838607493482595 | -0.001380576984842595 | NO AUTORIZADA |
| DAI | Sushi → Uni | 0.5 | 0.491805024045318932 | 0.00025 | -0.008444975954681068 | -0.008986945446041068 | NO AUTORIZADA |
| DAI | Sushi → Uni | 1 | 0.973183033882941453 | 0.0005 | -0.027316966117058547 | -0.027858935608418547 | NO AUTORIZADA |

Todos los márgenes ya son negativos antes de gas. No hay oportunidades positivas que requieran clasificación adicional: **20 negativas/no autorizadas, 0 positivas y 0 mercados ausentes**. Un resultado positivo futuro sería solo un candidato teórico, nunca una operación verificada.

El menor déficit absoluto después de gas es DAI, Sushi→Uni, 0,01 WETH: **−0,000606493480498788 WETH**, de los que −0,000064523989138788 corresponden al margen previo a gas. No es una recomendación de ejecución. Para 1 WETH, USDT tiene menos pérdida que DAI por mayor profundidad relativa de la pata Sushi; ningún tamaño evaluado resulta rentable.

## Límites y decisiones

- No hay fallos de tests ni defectos aritméticos demostrados que corregir. Se guardan solo informe y evidencia JSON; scripts, scanner Rust, contratos Aave, Render, claves y PR #10/#11 permanecen sin cambios.
- Gas 400k, tip y buffer son supuestos; `gasEstimateVerified=false`, `executableCalldataVerified=false`, `transactionSubmissionEnabled=false`. No hay estimateGas ni simulación de calldata del executor, pares reales en un fork, ni prueba de repago/payout para estos mercados en esta tarea.
- La prima global Aave y la presencia de código no demuestran capacidad de préstamo de WETH, habilitación del asset ni liquidez disponible. El capital del préstamo no es un coste, pero la capacidad y los límites deben verificarse separadamente.
- Identidad de factories/tokens y reservas no certifica características de transferencia, pausas/blacklists, reglas especiales de USDT, aprobaciones, permisos del executor, usuario/nonce/custodia o seguridad del contrato. DAI y USDT no se asumen equivalentes a USD para esta evaluación, que está enteramente denominada en WETH.
- Las lecturas se fijan a un número y hash finalizado; no se usó EIP-1898 con requireCanonical en cada llamada ni una segunda fuente independiente. La procedencia depende del proveedor público.
- Estado, reservas, base fee y prima cambian; no se midieron MEV, frontrunning, expiración, probabilidad de inclusión ni costes de un revert. Un cálculo positivo podría no ser ejecutable; aquí ninguno lo es siquiera bajo la hipótesis de coste.
- La muestra de un bloque y cinco tamaños no prueba ausencia global de oportunidades. Investigar primero series temporales y profundidad de ambas patas, priorizando USDT frente a DAI para tamaños mayores, antes de ampliar capital o mercados. No se recomienda habilitar trading con estos resultados.
- No hubo firma, envío, despliegue, aprobación de gasto ni movimiento real de fondos. PR #12 debe mantenerse DRAFT.

## Resultados individuales de tests

| Test | Resultado |
| --- | --- |
| equal V2 reserve quotes lose money to two 30bps fees | PASSED |
| gas and Aave premium deducted in WETH wei | PASSED |
| even synthetic positive quote never approves execution | PASSED |
| missing factory pair emits unavailable not fabricated opportunities | PASSED |
| 10 sizes and directions per market, never a trade signal | PASSED |
| requires chain ID 1 | PASSED |
| pinned RPC reads discover two canonical markets and true token decimals | PASSED |
| reject mismatched V2 factory | PASSED |
| missing pair remains unavailable for both markets | PASSED |
| fail closed for invalid block and wrong historical block | PASSED |
