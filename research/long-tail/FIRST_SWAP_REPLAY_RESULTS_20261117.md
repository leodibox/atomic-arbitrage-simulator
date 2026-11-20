# First Swap: preservación y prueba de replay transaccional

**Estado final: INSUFFICIENT_REPLAY_ACCESS.** PR #16 DRAFT; rama `research/issue-15-long-tail-discovery-20261117`. No se reconstruyó el estado post-transacción ni se declaró oportunidad positiva.

## Trabajo preservado

Se revisaron/corrigieron los dos topics Swap contra Keccak-256 de sus firmas ABI, el test de regresión, el sample y el informe del piloto. Antes del replay, `npm test` dio **35 PASS, 0 FAIL, 0 SKIP**. Se comprobaron **166 logs, ocho candidatos confirmados, cero receipt failures y 20 llamadas RPC** del muestreo original.

Revisión automatizada y manual de los cuatro archivos relevantes: sin claves privadas, tokens, URLs RPC privadas con credenciales ni asignaciones de secretos detectadas. Solo direcciones/hashes públicos y metadatos de investigación. Esto no se presenta como una auditoría exhaustiva de todo el repositorio.

Commit de preservación subido a la rama existente: **b640911**. El sample [recent-swap-sample.json](evidence/recent-swap-sample.json) no se regeneró ni sobrescribió. SHA256 antes/después: `3df6464a1d7c9e34a24508feaa58faae9bba655091bef00c70bfb0af289bde2f`. Registro [preservation.json](evidence/first-swap-replay-20261117/preservation.json).

Tras añadir selección/replay planning, la suite completa dio **39 PASS, 0 FAIL, 0 SKIP**: 35 existentes más cuatro de selección, rechazo de evidencia alterada y frontera Geth. [Salida final](evidence/first-swap-replay-20261117/final-tests.txt). No se confunde ese éxito de unit tests con un replay real exitoso.

## Primer candidato seleccionado

Orden determinista: mayor movimiento WETH de un Swap individual confirmado por bloque; desempate por bloque, índice de transacción y log ascendentes. El ranking se revalidó con los ocho candidatos, incluso invirtiendo su orden de almacenamiento. Se seleccionó un solo candidato.

| Dato | Valor |
| --- | --- |
| Transacción | `0xde4d559fa47f4534a3f44880a89c6609a6378325b776d67768383c314bb681fe` |
| Bloque | **26437505** |
| Hash del bloque | `0x91b88c8e8ab2160eef6abd2b9de92c9c543f4a5405cec310dd362c2b53abe493` |
| Índice de transacción (cero-base) | **48** |
| Índice de log | **146** |
| Pool | `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` |
| Tipo / fee | Uniswap V3 / 500 ppm = **5 bp** |
| Movimiento WETH | **43,567528875890171904 WETH**, WETH_IN |
| Base fee histórica | **185561570 wei/gas = 0,185561570 gwei** |
| Timestamp | 1794951491 |
| Receipt | status=1, bloque/hash/txIndex/log/contenido Swap comprobados otra vez |

[Receipt completo](evidence/first-swap-replay-20261117/selected-receipt.json) y [bloque con orden canónico](evidence/first-swap-replay-20261117/selected-block.json) se guardaron en archivos nuevos, sin reemplazar el sample. La transacción aparece en transactions[48] y existe una siguiente en índice 49. La base fee actual no se consultó ni sustituyó a la histórica. Volumen grande no es spread ni beneficio.

## Frontera correcta: después de la transacción completa

Se pidió Geth debug_traceCall sobre slot0() del pool (`0x3850c7bd`), bloque fijado por **hash**, con callTracer, timeout 5s, gas 100.000 y reexec limitado a 64 bloques.

Se verificó el comportamiento de referencia en [Geth traceCall](https://geth.ethereum.org/docs/interacting-with-geth/rpc/ns-debug) y sus fuentes [eth/tracers/api.go](https://github.com/ethereum/go-ethereum/blob/master/eth/tracers/api.go) y [StateAtTransaction](https://github.com/ethereum/go-ethereum/blob/master/eth/state_accessor.go). StateAtTransaction devuelve estado **antes** del índice solicitado. Por eso el plan usa **txIndex=49 (0x31)** para observar después de completar la transacción 48 y antes de la 49. Usar 48 en esa implementación observaría antes del trigger. El servidor real no ejecutó esta API, así que no se afirma que su semántica/configuración haya sido certificada.

No hubo overrides de estado, reemplazo por precios de cierre, parent state presentado como post-state ni inferencia de balances de otros pools a partir de un solo log.

## Capacidad RPC: cinco consultas, un candidato

Se utilizó temporalmente PublicNode HTTPS sin credenciales al no existir binding privado disponible; TLS verificado y NODE_USE_ENV_PROXY=1. No se compró ni creó servicio. Presupuesto de seis consultas, **cinco realmente realizadas**, sin retry ni escaneo histórico masivo:

| # | Método | Resultado |
| --- | --- | --- |
| 1 | eth_chainId | HTTP 200, chain ID 1 |
| 2 | eth_getTransactionReceipt del trigger | HTTP 200, contenido revalidado |
| 3 | eth_getBlockByNumber del bloque exacto | HTTP 200, hash/orden/base fee revalidados |
| 4 | debug_traceCall con txIndex=0x31 | HTTP 200, **JSON-RPC -32601: the method debug_traceCall does not exist/is not available** |
| 5 | eth_getStorageAt WETH slot 0, bloque padre 26437504 | **HTTP 403, JSON-RPC -32602: Archive requests require a personal token** |

El error de trace es indisponibilidad de esa API en el endpoint; no se deduce que Ethereum o todos los proveedores carezcan de ella. La segunda observación confirma que este endpoint también bloquea el método de estado padre probado. No equivale a probar todas las APIs de archivo. Anvil **no se inició**: lanzar --fork-transaction-hash habría requerido obtener/reconstruir estado y transacciones previas sin un acceso demostrado, fuera del propósito de esta primera prueba acotada. No se descargó un bloque histórico completo con cuerpos de transacciones ni se intentaron otros siete candidatos.

[Plan, requests, errores y flags exactos](evidence/first-swap-replay-20261117/replay-capability.json). URLs en errores se redactaron; no se guardaron claves ni endpoint privado. El runner exige HTTPS, restringe métodos y crea artefactos con flag wx: un segundo intento no debe sobrescribir la evidencia existente.

## Evaluación económica y pasos no ejecutados

| Etapa | Estado |
| --- | --- |
| Selección/receipt/orden/base fee | VERIFICADOS |
| Replay después de toda la tx 48, antes de tx 49 | **BLOCKED / INSUFFICIENT_REPLAY_ACCESS** |
| Cotizaciones y sizing V3 tier/V2-V3/triangulares post-trigger | **NOT RUN — requiere replay** |
| Settlement real Morpho en ese estado | NOT RUN |
| Gas cold, tip y builder payment para oportunidad post-tx | NOT RUN |
| Beneficio neto simulado positivo | **NO VERIFICADO** |
| EV/probabilidad de inclusión propia | NO MEDIDA |

No se ejecuta la fase económica con estado de fin de bloque. Los tests unitarios de topología no son una simulación de estas rutas después del trigger. No se heredan cotizaciones/gas de otros bloques ni se concluye que esta transacción causó una oportunidad o que no la causó. El resultado es el bloqueo de acceso, no un juicio económico sobre el estado que falta.

## Siguiente paso mínimo

Obtener, sin contratar automáticamente nada, un endpoint que admita debug_traceCall con semántica transaccional comprobable, o acceso a estado padre/cuerpos canónicos para reconstrucción local con Anvil y un presupuesto explícito de datos/CPU. Mantener exactamente el mismo candidato, bloque/hash y orden. Solo después de verificar esa frontera pueden cotizarse rutas distintas y medirse settlement/gas con financiación real del fork. Un eth_call histórico a fin de bloque no resuelve este requisito.

No hay firmas, bundles, contratos desplegados, producción modificada, fondos reales ni trading habilitado. No se creó otro PR. Se actualiza exclusivamente el PR #16 DRAFT con investigación y el bloqueo explícito, conservando la evidencia anterior.

## Revalidación posterior de la misma solicitud

Sobre HEAD `ff5005f`, se repitió `npm test`: **39 PASS, 0 FAIL, 0 SKIP**. Se revisaron 14 archivos relevantes sin coincidencias de credenciales/URLs privadas; el sample conserva conteos 166/8/0/20 y SHA256 original. No hay HTTPS_URL ni bindings runtime/secret nuevos. No se reintentaron las mismas consultas de acceso sin cambio de requisito: **0 nuevas RPC**. El bloqueo previo sigue siendo la única evidencia de replay; no se declara que el proveedor haya sido consultado de nuevo. No hubo estado post-transacción ni fase económica nueva. [Registro de revalidación](evidence/first-swap-replay-20261117/recheck-ff5005f.json).
