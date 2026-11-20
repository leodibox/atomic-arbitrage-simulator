# Recent Swap pilot — ejecución 2026-11-17

Rama: `research/issue-15-long-tail-discovery-20261117`, HEAD de partida `8bff028`.

- `npm ci`: completado, nueve paquetes instalados.
- `node --test test/intra_uniswap_routes.test.mjs`: **9 PASS, 0 FAIL, 0 SKIP**.
- `node --test test/swap_trigger_candidates.test.mjs`: inicialmente **11 PASS**; tras añadir regresión de firmas ABI, **12 PASS, 0 FAIL, 0 SKIP**.
- Primer intento del muestreador: **BLOCKED, SWAP_EVENT_TOPIC_MISMATCH**, antes de llamadas RPC. Se demostró que ambos topics Swap configurados eran erróneos; se corrigieron a Keccak-256 de las firmas canónicas V2/V3 y se añadió un test independiente.

Segundo intento con los mismos límites `SWAP_WINDOW_BLOCKS=48 SWAP_MAX_CANDIDATES=8`: **COMPLETADO**, 20/20 solicitudes RPC read-only. Se utilizó el fallback PublicNode HTTPS y NODE_USE_ENV_PROXY=1, sin claves ni desactivar TLS. No hubo backfill ni ampliación de la ventana.

- Chain ID: 1.
- Bloques: 26437480–26437527 (48 bloques).
- Finalizado: 26437527, hash `0xd9b84b43023df29b37d6890af5d7d0d40456b5cd92738dc6664a2d0fc5fd7d99`.
- Pools canónicos indexados: 5.
- Swap logs observados: 166.
- Candidatos confirmados por receipt y header: 8; fallos de receipts: 0.
- Estado post-transacción: `NOT_REPLAYED`.
- Operaciones con neto positivo verificado: 0; tradeExecutionAllowed=false.

[Evidencia completa](evidence/recent-swap-sample.json). Los candidatos son el mayor Swap individual por bloque seleccionado, no rutas arbitrajables ni una muestra long-tail completa. No se verificaron notional/depth USD independientes, cotizaciones post-transacción, gas de una operación enviable, builder payments ni EV. Los headers/receipts confirman inclusión y evento, no el estado inmediatamente posterior a toda la transacción.

Cambios de código mínimos preservados para el PR #16 existente: dos topics y una prueba de regresión; sin cambios a Rust, contratos, Render, claves o fondos. No se firmaron/enviaron transacciones ni se publicó un PR nuevo. No se hizo replay ni se habilitó trading.

Revalidación de preservación: `npm test` completó **35 PASS, 0 FAIL, 0 SKIP**. Los cuatro archivos relevantes se revisaron en busca de secretos; sin matches de credenciales o RPC privado. Checksum original y registro: [preservation.json](evidence/first-swap-replay-20261117/preservation.json). No se regeneró ni sobrescribió el sample.
