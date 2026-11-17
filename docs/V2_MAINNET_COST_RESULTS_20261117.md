# Evaluación V2 de costes y rentabilidad — 2026-11-17

Rama exclusiva: `research/v2-mainnet-cost-assessment-20261117`. PR #11 DRAFT. **Investigación: todas las rutas NO AUTORIZADAS PARA EJECUCIÓN.**

## Ejecución y procedencia

Las ocho pruebas originales pasaron; después de corregir la falta de trazabilidad y permitir consulta histórica, las diez pruebas pasan: **10 PASSED, 0 FAILED, 0 SKIPPED**. Se ejecutó `node --test scripts/assess_v2_costs.test.mjs`.

HTTPS_URL no estaba exportada, no había .env local y el entorno no tenía bindings de variables/secretos. Se usó un endpoint público PublicNode HTTPS temporal, sin claves ni persistir configuración privada. El script verificó chain ID 1 y solo utilizó eth_chainId, eth_getBlockByNumber, eth_getCode y eth_call. La primera ejecución falló por conectividad de fetch: en este entorno Node 24 necesita `NODE_USE_ENV_PROXY=1`; activarlo resolvió la consulta sin desactivar TLS.

Se ejecutó `node scripts/assess_v2_costs.mjs` con HTTPS_URL en el proceso. La primera consulta latest completa muestreó 26436929. Para el informe reproducible se repitió con `V2_BLOCK_TAG=finalized` y se consultó el bloque **26436857**, hash `0xe55b82d4bdcd1b2af42d7dcf1beac94452f1f0be971c7506b6f900f84c5bd32c`. Todas las lecturas de reservas, código, prima y feeds usaron su número hexadecimal explícito. Fecha del bloque: **2026-11-17T14:28:23-05:00**. No se crearon ni enviaron transacciones.

[Snapshot completo y ocho resultados JSON](research/v2-costs-mainnet-26436857.json).

## Reservas reales e identidad

Se verificaron token0=USDC, token1=WETH, código de ambos pares, factory canónica y factory.getPair(USDC,WETH). No se manipularon reservas ni balances.

| Pool | USDC | WETH |
| --- | ---: | ---: |
| Uniswap V2 `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | 8604526.027589 | 4807.916900177160098235 |
| Sushi V2 `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | 114927.085155 | 64.217290036663168938 |

La prima consultada en el Pool Aave V3 Ethereum `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2` fue **5 bps (0,05%)**. Los dos swaps incluyen cada uno la comisión V2 del 0,30% y el impacto por reservas, calculados con enteros y redondeo hacia abajo. La prima se redondea hacia arriba de forma conservadora; en estos cuatro tamaños coincide exactamente con la prima porcentual.

## Chainlink y gas estimado

| Feed | Dirección | Precio USD | UpdatedAt | Edad en el bloque |
| --- | --- | ---: | ---: | ---: |
| ETH/USD | `0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419` | 1785.64181066 | 1794941231 | 2472 s |
| USDC/USD | `0x8fFfFfd4AfB6115b954Bd326cbe7B4BA576818f6` | 0.99979041 | 1794869747 | 73956 s |

Ambos feeds reportaron 8 decimales y superaron las comprobaciones de signo, round, updatedAt y antigüedad respecto al bloque. Umbrales configurados: ETH/USD 7.200 s; USDC/USD 172.800 s. El precio USDC no se supuso igual a un dólar. El umbral USDC permite hasta 48 h y no garantiza un precio apropiado durante un depeg; su idoneidad debe revisarse según heartbeat/desviación y condiciones del mercado.

- Base fee del bloque: **0.121267128 gwei**.
- Supuestos: **400.000 gas**, propina **1 gwei**, buffer de coste **20%**.
- Coste ETH: **0.0004485068512 ETH**.
- Conversión ETH/USD ÷ USDC/USD: **0.801041 USDC**; con buffer y redondeo conservador: **0.96125 USDC**.

Es un coste hipotético, no eth_estimateGas para calldata rentable. Se mantienen `gasEstimateVerified=false`, `executableCalldataVerified=false` y `transactionSubmissionEnabled=false`.

## Ocho resultados

Antes de gas = salida − principal − prima Aave; después de gas = anterior − coste estimado con buffer. Las comisiones e impacto AMM ya están incorporados en la salida.

| Ruta | Entrada USDC | Salida USDC | Prima USDC | Antes de gas USDC | Después de gas USDC | Ejecución |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Uniswap V2 → Sushi V2 | 100.000000 | 99.313947 | 0.05 | -0.736053 | -1.697303 | NO AUTORIZADA |
| Uniswap V2 → Sushi V2 | 1000.000000 | 985.373240 | 0.5 | -15.126760 | -16.088010 | NO AUTORIZADA |
| Uniswap V2 → Sushi V2 | 5000.000000 | 4761.384076 | 2.5 | -241.115924 | -242.077174 | NO AUTORIZADA |
| Uniswap V2 → Sushi V2 | 10000.000000 | 9139.067769 | 5 | -865.932231 | -866.893481 | NO AUTORIZADA |
| Sushi V2 → Uniswap V2 | 100.000000 | 99.313501 | 0.05 | -0.736499 | -1.697749 | NO AUTORIZADA |
| Sushi V2 → Uniswap V2 | 1000.000000 | 985.346271 | 0.5 | -15.153729 | -16.114979 | NO AUTORIZADA |
| Sushi V2 → Uniswap V2 | 5000.000000 | 4760.789543 | 2.5 | -241.710457 | -242.671707 | NO AUTORIZADA |
| Sushi V2 → Uniswap V2 | 10000.000000 | 9136.893828 | 5 | -868.106172 | -869.067422 | NO AUTORIZADA |

**0 de 8 oportunidades superan costes.** Todas ya pierden antes de gas. La mejor muestra es Uniswap→Sushi con 100 USDC: −0,736053 antes y −1,697303 después del gas estimado. Un préstamo de 10.000 USDC equivale a ~8,7% de las reservas USDC de Sushi; el impacto de esa pata domina al aumentar el tamaño. La prima y el gas no rescatan un roundtrip negativo.

## Comparación con el fork del PR #10

Se consultó en modo lectura el informe de PR #10 en commit `64581b07fb2eb786ee8221a890233ca210db66c2`, sin cambiar esa rama, PR ni contratos. Allí, al bloque 26436156, los dos swaps reales y el revert por falta de beneficio se ejecutaron en un fork. Para entrada de 100 USDC:

| Ruta | Salida fork PR #10 | Antes de gas fork | Salida muestra actual | Antes de gas actual |
| --- | ---: | ---: | ---: | ---: |
| Uniswap→Sushi | 99,270077 | −0,779923 | 99,313947 | −0,736053 |
| Sushi→Uniswap | 99,357077 | −0,692923 | 99,313501 | −0,736499 |

Coinciden la fórmula V2 997/1000 por salto, las unidades USDC/WETH, la prima de 5 bps y la conclusión negativa; los importes varían porque el estado es de otro bloque. **No se afirma igualdad exacta en el mismo bloque.** Intentar repetir el bloque 26436156 con PublicNode devolvió HTTP 403 en eth_call; el acceso al header histórico sí funcionó. Dos endpoints públicos alternativos también devolvieron HTTP 403. No se atribuye una causa no demostrada al rechazo ni se alteraron precios/reservas para obtener un resultado. La repetición exacta queda BLOQUEADA por acceso RPC histórico; no está aprobada ni omitida silenciosamente. Una primera consulta histórica usó por error 26435644; fue rechazada y se excluyó del análisis, luego se solicitó correctamente 26436156.

El fork previo midió ~281.795 gas de ejecución warm en rutas reales que revertían y ~332.373 en una oportunidad sintética; su gas-report instrumentado arrojó 357.265–362.667. Los 400.000 aquí son una hipótesis, no una calibración exacta: diferencias de cold/warm storage, calldata, eventos, aprobaciones, compilación y resultado afectan el gas. El informe del PR #10 no certificó rentabilidad después del gas; esta muestra sí descuenta un coste convertido por oráculos, pero sigue sin certificar ejecución.

## Errores, cambios y límites

- Fallo inicial de conectividad Node corregido con configuración de proxy del runtime; no se cambió código para eludir TLS o controles de acceso.
- El CLI original descartaba las reservas/precios usados. El informe ahora incluye el snapshot que sustenta sus cifras; no se almacena el RPC.
- Se añadió V2_BLOCK_TAG para finalized/safe/latest o número hexadecimal, validación de coincidencia del bloque devuelto y tests de pinning de todas las lecturas. Permite reproducir un bloque cuando el RPC tenga acceso.
- No hubo defectos aritméticos demostrados; no se modificaron el scanner Rust, Render, contratos, claves ni PR #10.
- Este script no ejecuta swaps ni calldata del executor. No verifica liquidez prestable de Aave, estado de reserva, pausas/blacklists, permisos del executor, custody/nonce/signing ni coste real de inclusión. La observación es histórica, no una promesa para el siguiente bloque.
- Se usan dos pares de un único mercado, ocho tamaños/direcciones y un bloque finalizado: no se demuestra ausencia global de arbitraje. MEV, slippage y estado entre lectura e inclusión pueden empeorar el resultado.
- Todas las muestras y cualquier resultado futuro positivo siguen NO AUTORIZADOS; harían falta simulación exacta fresca, estimación de gas, revisión independiente y autorización separada antes de considerar operaciones reales.

## Pools a investigar después

1. **Inventario canónico WETH/USDT y WETH/DAI en ambas factories**, solo lectura: confirmar getPair/código/tokens, reservas y profundidad de la pata más pequeña. Priorizar únicamente si ambas patas tienen más profundidad relativa para los tamaños de prueba; no se afirma que hoy la tengan ni que sean rentables.
2. **WETH/WBTC**, como candidato con otro mercado subyacente: verificar liquidez de ambos venues, decimales WBTC y feed WBTC/USD antes de calcular costes o comparar precios. No trasladar automáticamente la aritmética de unidades USDC.
3. **Series temporales de USDC/WETH y tamaños menores**, con bloques fijos coherentes y costes actualizados, para medir persistencia de discrepancias y separar ruido de impactos reales. Esta muestra no justifica aumentar el préstamo.
4. **USDC/USDT y USDC/DAI V2**, prioridad baja salvo desalineación observada: dos comisiones de 30 bps más Aave y gas son una barrera alta para pares estables. Requieren feeds/decimales/condiciones de token y riesgo de depeg propios.

Estos son candidatos de investigación, no pools autorizados ni oportunidades verificadas. No se recomienda desplegar ni habilitar operaciones con los resultados actuales.

## Estado individual de tests

| Test | Estado |
| --- | --- |
| V2 fee and slippage decrease roundtrip returns in equal pools | PASSED |
| revenue in USDC is never obtained by treating ETH wei as USDC raw units | PASSED |
| real-like V2 route loses money before and after gas | PASSED |
| synthetically positive route still is never approved for execution | PASSED |
| reject stale Chainlink round and answeredInRound below round | PASSED |
| report rejects invalid gas inputs and never sends transaction | PASSED |
| read-only RPC snapshot uses one mainnet block, canonical factories and real Aave getter | PASSED |
| read-only RPC verification fails closed for wrong chain or wrong V2 factory | PASSED |
| explicit historical block pins all state reads and retains auditable snapshot | PASSED |
| invalid or mismatched historical blocks fail closed | PASSED |
