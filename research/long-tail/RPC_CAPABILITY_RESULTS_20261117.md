# PR #17 — diagnóstico RPC acotado

Rama: `research/issue15-rpc-capability-20261117`. HEAD inicial verificado: `0606c804b88950a58faaf297564cc28af26a2274`; PR #17 DRAFT. Checkpoint heredado de PR #16, que permanece sin cambios.

## Ejecución y resultado principal

**HTTP 403 ocurre incluso al consultar un único bloque antiguo. No es exclusivo de rangos grandes.** El proveedor también rechaza dos métodos de estado histórico en ese mismo bloque. En los tres casos respondió HTTP 403, error JSON-RPC `-32602` y un mensaje clasificado como `ARCHIVE_PERSONAL_TOKEN_REQUIRED` (el proveedor informa que archive requiere un token personal). No se atribuye por inferencia a poda, cuota agotada o un límite numérico de rango.

Se ejecutaron **6 consultas reales en total**: 4 del diagnóstico de logs (presupuesto máximo 6) y 2 de estado histórico (presupuesto separado 2). Sin reintentos, otros proveedores, backfill, compras ni altas. Fuente: fallback público PublicNode HTTPS, sin credenciales; no se publicó ni guardó una URL con claves. Node utilizó NODE_USE_ENV_PROXY=1, manteniendo TLS.

| # | Método | Objetivo | Resultado |
| --- | --- | --- | --- |
| 1 | eth_chainId | verificar red | HTTP 200, chain ID 1 |
| 2 | eth_getBlockByNumber | checkpoint 26437145 | HTTP 200, hash idéntico al checkpoint |
| 3 | eth_getLogs | Uniswap V2 PairCreated, un bloque antes accesible: **26428892** | HTTP 200, 0 eventos; éxito de consulta, no un pool descubierto |
| 4 | eth_getLogs | misma factory/topic, **un solo bloque antiguo 26426892** | HTTP 403; JSON-RPC -32602; proveedor requiere token archive |
| 5 | eth_call | WETH totalSupply(), **26426892** | HTTP 403; JSON-RPC -32602; misma categoría archive |
| 6 | eth_getStorageAt | WETH slot 0, **26426892** | HTTP 403; JSON-RPC -32602; misma categoría archive |

Factory: `0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f`; topic PairCreated `0x0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9`. Estado: WETH canónico `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2`; eth_call selector `0x18160ddd`, gas máximo 100.000. No se confunde un header disponible con estado histórico disponible.

El llamado bloque «recent» del diagnóstico es el primer bloque de una ventana **previamente** accesible en PR #16, no una nueva selección latest. El bloque antiguo es el inicio del último intervalo fallido, no un bloque arbitrario de otra ventana.

Los tests de **10 bloques** y de la **ventana completa de 2.000** quedaron **NOT_TESTED** porque falló el bloque único; no son PASS ni se intentaron. No se determinó un rango máximo admitido. No hay base para iniciar 12.600 consultas de diez bloques: fragmentar no elimina el rechazo observado de una sola altura antigua.

## Cuotas y checkpoint

Ninguna respuesta incluyó las cabeceras inspeccionadas `Retry-After`, `RateLimit-*` o `X-RateLimit-*`; las cuotas, precio por llamada, límites diarios y umbral exacto de antigüedad son **UNKNOWN**, no ilimitados ni gratuitos. No se observó HTTP 429 en estas seis solicitudes. No se compró servicio ni se creó/cambió token alguno.

Checkpoint: `evidence/run-26437145-full/discovery.json`.

- Chain ID: 1.
- Bloque final original: **26437145**, hash `0x46eff74c6c5fdc7555c6f966f464352fcebf421e49dacb91575d48da13682dc9`.
- SHA256 antes y después: `a8ba86237df096e632721b2b8eb25265bc4bb133fea6b55ba0f5501519708885`.
- Cobertura original conservada: 42.000 bloques pendientes por factory; 126.000 factory-bloques entre tres factories. No se marca ningún hueco como reparado por un diagnóstico.

[Registro exacto de las seis consultas, respuestas normalizadas y checksums](evidence/rpc-capability-20261117/diagnostic.json). Los mensajes arbitrarios del servidor y valores de credenciales no se retienen.

## Pruebas y correcciones demostradas

1. `npm ci` ejecutado desde `research/long-tail`: **completado**, nueve paquetes instalados; manifiesto/lockfile sin cambios.
2. `node --test test/rpc_capability.test.mjs`, versión inicial: **7 PASS, 0 FAIL, 0 SKIP**.
3. Tras las correcciones y pruebas adicionales: **10 PASS, 0 FAIL, 0 SKIP**. No se suman como 17 casos distintos.
4. Regresión completa `npm test`: **24 PASS, 0 FAIL, 0 SKIP**, incluidos los diez casos de capability, no 24 adicionales.

La comparación original cambiaba a la vez la longitud del rango y el bloque más antiguo: consultaba el final de la ventana fallida como bloque único y el inicio al ampliar rango. Se corrigió para conservar el mismo inicio en 1/10/2.000 bloques. La nueva prueba comprueba exactamente esos tres parámetros y el máximo de seis llamadas del diagnóstico.

También se dejó de interpretar cualquier error JSON-RPC -32xxx como autorización denegada; por ejemplo -32602 normalmente significa invalid params y no basta por sí mismo para inferir permisos. En la ejecución real sí constan HTTP 403 y la categoría específica del mensaje del proveedor, por separado.

Una prueba adicional verifica que eth_call y eth_getStorageAt son independientes, usan el mismo bloque y hacen exactamente dos llamadas aunque la primera falle.

| Test | Estado |
| --- | --- |
| finds exact denial boundary; refuses incomplete coverage accounting | PASS |
| one-block 403: cannot claim shrinking ranges will fix historical access | PASS |
| old one block and ten work but old full window fails: range restriction | PASS |
| old 10 block fails even when one block succeeds | PASS |
| full old window recovered: marked possibly transient, not coverage repaired | PASS |
| recent check failed makes diagnostic inconclusive | PASS |
| fail closed on wrong chain, block hash or factory address | PASS |
| one/ten/full old queries keep identical oldest block, isolating range from age | PASS |
| generic JSON-RPC invalid-params error is not labeled authorization denial | PASS |
| historical state probes are independent, pinned and exactly two even after one failure | PASS |

Estos son tests mock. Los errores de las tres consultas históricas reales permanecen FAILED en el registro, aunque el diagnóstico los haya detectado correctamente.

## Alternativa recomendada de menor coste

**No comprar archive ni iniciar un backfill todavía.** Primero comprobar con el proveedor si un token personal ya disponible o una modalidad sin coste incluye logs y los dos métodos de estado a esas alturas. El error indica una vía de autenticación, pero no certifica que sea gratuita ni su cuota; esas condiciones requieren confirmación. Configurar cualquier valor únicamente en el entorno seguro, nunca en chat/PR/código, en una tarea posterior autorizada.

Si ese acceso sin coste existe, reutilizar el checkpoint y verificar una sola consulta antigua por método antes de planificar recuperación. Si se aceptan ventanas de 2.000, los 63 huecos originales implican **63 consultas de logs**, más validaciones y lecturas de estado; esto es aritmética de presupuesto, no un backfill ejecutado o una cuota concedida. Un límite de diez bloques subiría la parte de logs a **12.600 solicitudes**; actualmente ni un bloque antiguo funciona, por lo que no se recomienda esa fragmentación.

Si no hay archive gratuito, la alternativa a evaluar es **un índice/exportación gratuito de logs**, con procedencia de factory/topics/bloque y deduplicación verificables, combinado con **muestreo prospectivo de estados finalized** cuando todavía sean accesibles. Eso puede reducir la necesidad de un nodo archive para investigación futura, pero un índice de eventos no sustituye eth_call, storage histórico ni fork, y no certifica por sí solo la economía del periodo faltante. No se ha verificado precio/cuota de un proveedor alternativo ni se inició su uso.

La opción monetaria absoluta más barata no puede calcularse sin ofertas/cuotas verificadas. El orden recomendado evita gastar: confirmar autenticación/capacidad incluida, luego valorar datos indexados y capturar estados futuros; pagar solo tras presupuesto explícito y una necesidad económica demostrada.

PR #17 permanece DRAFT. Sin cambios a producción, Rust, contratos, Render, claves, fondos o PR #16; sin firmas, transacciones ni backfill. El NO-GO económico previo no cambia por este diagnóstico de infraestructura.
