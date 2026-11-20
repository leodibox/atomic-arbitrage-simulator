# Issue #15 — Long-tail opportunity discovery

**Decisión: NO-GO** para construir otro ejecutor/sender con los resultados de esta campaña. **No es una afirmación de ausencia global de arbitraje.** Todo permanece DRAFT y `executable=false`.

Se leyeron íntegramente [Issue #15](https://github.com/leodibox/atomic-arbitrage-simulator/issues/15) y `docs/MORPHO_V3_MAINNET_CERTIFICATION.md` en commit `46979e3` antes de empezar. Rama nueva desde main `c48afd880301ff47ea5a1717440cd7c6cc228fda`: `research/issue-15-long-tail-discovery-20261117`. No se cambian PR #10–#14, Render, Rust ni ejecutores.

## Resultado y cobertura, sin extrapolaciones

| Métrica | Resultado |
| --- | ---: |
| Ventana solicitada | últimos 7 días por timestamp |
| Inicio por búsqueda binaria de headers | 26386892 |
| Fin finalized, chain ID 1 | 26437145 |
| Bloques de ventana solicitada | 50254 |
| Bloques con eventos accesibles por factory | 8254 (16.42%) |
| Ventanas factory×rango completas / fallidas | 15 / 63 |
| Pools distintos descubiertos en rangos accesibles | 111 |
| Pares de tokens únicos de esos eventos | 111 |
| Mercados nuevos con WETH | 74 |
| Eventos de pares sin WETH excluidos por alcance | 37 |
| Mercados WETH inspeccionados, más control | 74 + 1 |
| Long-tail con ≥2 pools utilizables verificados | 0 |
| Long-tail con un único pool utilizable | 69 |
| Long-tail sin liquidez relevante | 4 |
| Fallos de metadata de token | 1 |
| Mercado control USDC/WETH con ≥2 pools | 1, con 6 pools |
| Rutas ordenadas filtradas por spread marginal | 30, todas del control |
| Rutas con curva de tamaños | 2, ambas control |
| Observaciones de ruta / bloques | 6 / 3 |
| Cotizaciones exactas de dos saltos por tamaño/bloque | 48 |
| Puntos con margen bruto positivo | 3, todos en un bloque/control |
| Puntos positivos tras costes a 10/30/60 gwei | 0 / 0 / 0 |
| Candidatos positivos con seguridad y financiación certificados | 0 |

**Límite principal:** el RPC público devuelve HTTP 403 para las primeras 63 ventanas factory×rango; solo 15 ventanas recientes responden. El inventario no cubre siete días completos. Sushi registra cero eventos en sus rangos accesibles, no se afirma cero creaciones en la semana. Se requieren RPC y permisos históricos para cerrar esa cobertura.

Las tres factories son Uniswap V2 `0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f`, SushiSwap V2 `0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac` y Uniswap V3 `0x1F98431c8aD98523631AE4a59f267346ea31F984`. Son un registro explícito, no todos los exchanges de Mainnet. Fuentes de identidad: documentación oficial de [Uniswap](https://docs.uniswap.org/contracts/v3/reference/deployments/ethereum-deployments), [V2 factory](https://docs.uniswap.org/contracts/v2/reference/smart-contracts/factory) y [Sushi](https://docs.sushi.com/contracts/cpamm). Código presente consultado al bloque; registro getPair/getPool, factory, tokens, tarifas y orden de cada pool comprobados. Esto no es una auditoría o atestación de compilación completa.

Solo se dimensionan ciclos WETH→token→WETH para mantener gas y financiación en unidades canónicas ETH/WETH; no hay triangulares ni rutas ancladas en otros tokens. Los 37 eventos excluidos se conservan. Se comenzó con una muestra estratificada de 60 mercados, después se amplió a todos los 74 accesibles porque quedaba presupuesto RPC, reutilizando **el mismo checkpoint, bloque y ventana**, sin cambiar reservas ni criterios para forzar beneficio.

## Ejecución, límites RPC y hashes

No había HTTPS_URL configurada; se usó temporalmente PublicNode HTTPS, sin claves ni URLs secretas en código/evidencia. Node 24 necesitó NODE_USE_ENV_PROXY=1; TLS verificado. Solo se permiten seis métodos RPC de lectura y cualquier método de envío/bundle falla antes de la red. Calls arbitrarias de token/Quoter limitadas a 500.000 gas, requests a 25 segundos y presupuesto 6.000 calls por ejecución. La pasada completa de inventario/sizing usó 1359 llamadas, además de 1.220 de la pasada piloto con descubrimiento.

La petición posterior de hashes de runtime al mismo bloque ya recibió HTTP 403: no se sustituyó por latest ni se inventaron hashes. Las consultas iniciales de bytecode/factory sí se completaron, pero el hash de runtime no se conservó. Futuros replays dependen de acceso archive; no se garantiza un proveedor público. No hubo reintentos indefinidos ni sustitución de datos denegados por ceros.

| Observación | Bloque | Hash | Timestamp |
| --- | ---: | --- | ---: |
| sizing | 26437143 | `0x9b25fcef01bbd82ee066aec06d0fd52b30fe692214739ed8a6b6d500a86e7da7` | 1794947135 |
| sizing | 26437144 | `0x9cc77fcc856cd1998c41093e12edf5dc26ffb045553f70ba37727a2af119f42e` | 1794947147 |
| sizing | 26437145 | `0x46eff74c6c5fdc7555c6f966f464352fcebf421e49dacb91575d48da13682dc9` | 1794947159 |

El fin de descubrimiento y el fork usan 26437145, hash `0x46eff74c6c5fdc7555c6f966f464352fcebf421e49dacb91575d48da13682dc9`. Inicio de descubrimiento: hash `0x440199d3967ebbf03edfc06f1f1477f9eab20b09e764111bb8a118a16782ab6f`, timestamp 1794342359. Los estados de cada comparación usan su número explícito; no se mezclan reservas de bloques distintos dentro de una ruta. Los tres bloques son adyacentes (24 segundos), **no** una prueba de persistencia por días o eventos independientes. El Issue prefiere mayor diversidad temporal: queda pendiente con RPC histórico.

## Contrapartes históricas, profundidad y riesgos

Para cada token observado se consulta getPair en las dos factories V2 y getPool en los cuatro tiers V3 100/500/3000/10000, sin exigir fecha reciente al segundo pool. Direcciones cero son MISSING; identidad falsa o errores no entran al inventario utilizable. Dos pools deben tener direcciones distintas. Ningún long-tail verificable superó ese requisito; no se inventó un top-20 long-tail.

De los cuatro pools sin liquidez, dos tienen EMPTY_RESERVES y dos ZERO_ACTIVE_LIQUIDITY. Una liquidez L V3 no se llama TVL ni saldo prestable. Profundidad V2 y r0/r1, tick/sqrt/L V3 y decimales se conservan por pool. Se limita el sizing a 3 WETH y, si hay V2, al 10% de la reserva WETH más pequeña; también al cash Morpho observado. Estos son límites de investigación, no capacidad ejecutable garantizada.

Los tokens solo pasan code/decimals y getters pasivos owner/paused/slot EIP-1967. FOT, rebase, honeypot y roles se etiquetan **UNKNOWN/NOT TESTED**. Un slot EIP-1967 cero no prueba ausencia de proxy; un owner cero o pool líquido no hace seguro al token. En ningún caso `safe=true`. Las transferencias long-tail no se probaron porque no hubo ruta de dos pools; no se declara un rechazo económico global de esos tokens.

### Rechazos de mercados nuevos

| Token | Pools utilizables | Estado |
| --- | ---: | --- |
| `0x1fb42128960054A935bB54B6Bc0172bc6ddFD993` | 0 | ZERO_ACTIVE_LIQUIDITY |
| `0x0Dce7CfE1f695a693962e1b05BB9DcDd1D42d008` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xC80dAB5f215857e979411449579C46147709e039` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xc1b0fc13BdB04B76bDb1Bcf5015577004710e303` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x3f98476Ca7EafeA9CA72A0E866F7A6EfF00a5cBA` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x64D28f384FE538FdFA256385dB8d051e709cE756` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x65A88f93aC2dc7581FD020Dbd3Cdc415C5984478` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xc49E96b52be500f0085feD21B5d5496cB6ab2734` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x34A1e56e2eAF14E801B1d54cC96ff11A9509822a` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x17d4aa4f8638325d645e807e5a5b4630D1810F09` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x7F9C394b88AA7Cba400604603d42FFDA6EBE75BF` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x30806632FAD20B04A04215def68B7DE7B426292f` | ? | TOKEN_METADATA: RPC_ERROR_3 |
| `0x6aE520fE35042e6E90b8488cDdBbC5DC725Ed14b` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x66e17c1d7dB989e866970dF1cDE5581E5f2dFA20` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x6Bd22A1659834F38b2F5DBDe77758b8029E3b7d2` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xd8DBc233cEDD4A855C8F941271beADe34a2DDcb4` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x7F8DE1b3bc8D5414Fb2A0CAF4FDa8E87892DF62E` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x8F00300E2918071018D91985F07ddb10d926fCaA` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x5903B0cF2446BC4A3cfC90Fc28779c6769485087` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xA57799CbEa701B5bAA7e3aE4952F70aaa19A59BC` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x28a34977a01a5d34878e2E84C768d13B0b7A11F4` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x5f2133480e42360064adcA80178B69cbE2dfBAAA` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x8c81fE8A2D870Fa7521E6ee38e49CA3FdDF648A5` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xcC3cC5DF84D7eC55C21d0f823Dc613B162Ad3766` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x8D60773D2060A89C96147982188d1c75Dc584734` | 0 | ZERO_ACTIVE_LIQUIDITY |
| `0xD3639f2f057eB3B5100433D3A3282A9032b9F3e8` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xd8864e371b6b0Fd21c0612B34f8DFFB40f521Bed` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x5C4f9C565e1B37942f42BBc9DFfd2bB83E0C4197` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x25eC84FfFAD138B755737ffdF1F217c5fd3F644b` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x68a9A8ad88622dE2c6F6aB901CC3e0559cc44Ab1` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x879A9042060Eecce8963E538de575a109Dd88D2c` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x256C55046AB0E0C5286Fb86c5F0b865080aD0446` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xcbC215b3760d8dBdCcc786447ddc617a1E43B4a0` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x0452B688613B3027cCB53bCb48AA0b6d6B8D2E77` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x2578DCd4DBABB163C4Ae8EC1c0f89e77F2364D68` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x200C2720666E57956F67EbEA6f6D0abB2370075E` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xe8363430dF17d003847Eec2b248765E7e8488A57` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x1C4A9d399F045F04c0a259e3A7E73f17b446F6e8` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x22dE37E20be3012257D1Cc1AF9AAEf5774aD1d9a` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x4F7216eC910421F373fCEE74D85BBa46571d00A3` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xcb380181Dadd55e1e22300fbE8e602C93e0FE91D` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xD4070eCf275E60E398D81EA2DA92836804DDc83F` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xdA8a8B37589A98263Bd881C1C854aD958948C373` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xCd77941525c38C82B4fD3F0eC73c7F3c35db48BB` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x123E819714184167762890A8AE13C389C1e4e71E` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xd98e521533fBDB9229974d6a5E4eFdA432032A33` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x931db15B93C98323b7cbE46eb7F91aDD0ad53945` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x04efe8695358E8BfD719dB62E292Bd71c827d017` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x95c7082fb200C77Cf0fF7849CC8cC8177e6f91BD` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xF71C70ef343D3eF33C828Cd7487752ce4d2C5E11` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x43ba3bc640164C7F7ffD1749365B9C095A1126de` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x1FE82cea124D1506cbF89b47B4B9D841E8FFF99E` | 0 | EMPTY_RESERVES |
| `0x322bea960cFefa81008EaEe5D29F8e1Be1e8BA92` | 0 | EMPTY_RESERVES |
| `0xbECe4927bB2885a88F9904F4f8C9C408BB93849d` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x54A74A191c288944a3D679c7FEba6420E6789bC1` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xC9Cf4d365349BF8f2952eb59032fd7A09BC783B2` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xeC644c7D965472636f0C36d2Ca5bA6a2e99F061F` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xCBEfA5e43a04a8265cF4224Dbd29E051228b9F46` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x2F0F54C33041dd295f2B5C1390efB29393265e0C` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xc2Ad82D596CCA4cd16FBbD1C16dCFa41CE4E6C57` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xa50ad3a559A10f384a5bB2e27516f63E0B937b1A` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x0E037574c39bC104bC4556FCe78958222CB7d154` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xD0170C16743C528576508936D46568FA249Cc9bB` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x40db5a11746b278B8e0b592D94401ED8ceF36208` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x4368bB38CF07Ba4b19aD4b37DB8caBE7e60096E7` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x08D5625BC4E7B624785F823F7EAfBda4b8dEe732` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xF02D879358dc4a9C9Ca39E52a8AA9417668FFc7C` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x0dB18eAE9b476E6DeA5FF1B97a76Ed7f4429797c` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xFe928AF504ec8A5cD26a9364405FdE2bD829db1f` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xDCB12339F0B54d0e2CB23147ABe26aBD74C2aAe2` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x6CAe898E20D041d34c2670a14054E8caF73De327` | 1 | SINGLE_POOL_NO_ROUTE |
| `0xB38a713774082532Bf70ddaecAE93e9667f83172` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x15b6789a86006c7dF23055f34FBcC9a0f51bD577` | 1 | SINGLE_POOL_NO_ROUTE |
| `0x84913BeE65BdcF81bEE0C6Ef2aB0659fC6E67505` | 1 | SINGLE_POOL_NO_ROUTE |

### Pools del control y estados al bloque final

| Pool | Venue/tipo/tarifa | Reservas o tick/L |
| --- | --- | --- |
| `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | UniswapV2 / v2 / 3000 ppm | r0=8620298922974; r1=4799146068676991083645; WETH=4799.146068676991083645 |
| `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | SushiV2 / v2 / 3000 ppm | r0=115104441382; r1=64118638724191474318; WETH=64.118638724191474318 |
| `0xE0554a476A092703abdB3Ef35c80e0D76d32939F` | UniswapV3 / v3 / 100 ppm | sqrt=1866665143337220640391441874174030; tick=201356; L=312159731259885036 |
| `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` | UniswapV3 / v3 / 500 ppm | sqrt=1867040274528632416004487896851777; tick=201360; L=4249028564064179596 |
| `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8` | UniswapV3 / v3 / 3000 ppm | sqrt=1869389656408341466279997804162165; tick=201385; L=954485176759493943 |
| `0x7BeA39867e4169DBe237d55C8242a8f2fcDcc387` | UniswapV3 / v3 / 10000 ppm | sqrt=1875240877856483832994831326635585; tick=201448; L=13381640065932002 |

## Financiación: modelo frente a prueba

WETH canónico: `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2`. ETH gas y WETH input comparten 18-decimales y unidad económica 1:1 por wrapping, no se requiere un supuesto de peg USD ni convertir USDC raw como wei. Los escenarios 10/30/60 gwei son **precio total por gas**, no tips añadidos. La reserva/premium se toma al bloque de cada fila; no se trasladan USD/ETH de otra fecha.

Morpho `0xBBBBBbbBBb9cC5e90e3b3Af64bdAF62C37EEFFCb`: cash WETH al fin **14135.998245223616169343**, prima modelo 0 por implementación canónica documentada y certificación previa; no se inventa un getter ni se usa supply−borrow de un mercado individual. El fork actual confirma entrega del préstamo antes de sus swaps, pero **siempre revierte deliberadamente**, de modo que no es una nueva prueba de repago exitoso.

Aave `0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2`: prima final 5 bps; aToken `0x4d5f47fa6a74757f35c14fd3a6ef8e3c9bc514e8`, cash WETH **304366.191146524876580794**; active=true, paused=false, flashLoanEnabled=true. Son cash/flags observados, no ejecución ni garantía de préstamo. Ambos modelos se conservan por punto; el premium de Aave se redondea arriba. No se modificó ningún contrato Aave.

## Sizing, break-even y límites matemáticos

Para dos V2 estáticos estándar: out(x)=A·x/(B+C·x), con tarifas reales 997/1000. Si el margen marginal tras financiación es no positivo, la curva continua cóncava no puede superar principal+coste fijo positivo; el código descarta esa hipótesis sin fingir un óptimo. En un segmento válido positivo calcula el estacionario, refina vecinos enteros y las raíces cuadráticas continuas solo si hay máximo neto positivo. No afirma prueba de óptimo discreto global por redondeo; FOT/rebase invalidarían el modelo. Se contrasta con brute force en tests. No se halló un long-tail V2/V2 elegible al que aplicar una raíz positiva on-chain.

V3 usa QuoterV2 `0x61fFE014bA17989E743c5F6cB21bF9697530B21e`, con final sqrt, ticks cruzados y gas de quote. Se dimensiona una malla acotada 0,001 / 0,003 / 0,01 / 0,03 / 0,1 / 0,3 / 1 / 3 WETH; no se interpola como V2 ni se presume concavidad global sobre ticks. **Óptimo global y puntos de equilibrio de V3: UNKNOWN**, no inventados. Hubo ocho cotizaciones que cruzaron ticks inicializados.

**Capital recomendado con evidencia actual: 0 (no operar)**. El mejor tamaño bruto *muestreado*, no óptimo certificado, fue 0.010000000000000000 WETH. Incluso ese caso no cubre el gas. No se conoce un capital óptimo rentable ni una raíz neta positiva; el resto del espacio no muestreado sigue abierto, no certificado.

## Resultados multi-bloque y gas

Dos controles dimensionados: V3 1bp→Sushi V2 y V3 1bp→Uni V2. Ocho tamaños×dos rutas×tres bloques = 48 puntos, dos modelos de financiación cada uno y tres escenarios de gas. No hubo quote errors en esos 48; Quoter V3 sí es referencia exacta pero no prueba transferencia/repago. Las 28 rutas restantes del control solo tienen filtro marginal/identidad, no curva completa o fork.

Gas asumido 500.000 por intento; no un estimateGas verificado ni una cota superior demostrada. La tabla muestra el mejor margen bruto muestreado por ruta/bloque y sus márgenes Morpho tras gas supuesto; Aave descuenta además su prima en el JSON.

| Bloque | Segundo pool | Tamaño WETH | Bruto WETH | Neto 10 gwei | Neto 30 gwei | Neto 60 gwei |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 26437143 | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | 0.001000000000000000 | -0.000000219335782644 | -0.005000219335782644 | -0.015000219335782644 | -0.030000219335782644 |
| 26437143 | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | 0.001000000000000000 | -0.000000242580476654 | -0.005000242580476654 | -0.015000242580476654 | -0.030000242580476654 |
| 26437144 | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | 0.001000000000000000 | -0.000000061058463579 | -0.005000061058463579 | -0.015000061058463579 | -0.030000061058463579 |
| 26437144 | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | 0.001000000000000000 | -0.000000084304402135 | -0.005000084304402135 | -0.015000084304402135 | -0.030000084304402135 |
| 26437145 | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | 0.010000000000000000 | 0.000002346700951391 | -0.004997653299048609 | -0.014997653299048609 | -0.029997653299048609 |
| 26437145 | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | 0.001000000000000000 | -0.000000185608057284 | -0.005000185608057284 | -0.015000185608057284 | -0.030000185608057284 |

### Falso positivo: spread positivo no cubre gas

Al bloque 26437145, V3 1bp→Sushi V2 presenta un spread marginal después de fees cercano a 3,92 bp. Tres tamaños tienen bruto positivo; el máximo muestreado es **0.000002346700951391 WETH** con 0.010000000000000000 WETH. Ese tamaño quedó reproducido exactamente en fork, pero el neto a 10 gwei y 500k gas es **-0.004997653299048609 WETH**. No es una oportunidad rentable.

El fork midió 249.450 gas del probe call warm (217.369 dentro del callback) para ese caso. Aun usando únicamente esa medición incompleta, 10 gwei cuesta 0,0024945 WETH, muy superior al bruto. No se usa el supuesto 500k para fabricar la conclusión: también es negativa con el valor medido parcial. El precio de gas formal máximo bruto/gas del probe es ~0,0094075 gwei; solo es una cota diagnóstica de ese punto positivo, no una tarifa enviable.

### Reversiones, EV y probabilidades no calibradas

Cada punto conserva sensibilidad a éxito supuesto 0/25/50/75/100%. EV=p·(bruto−prima−gas éxito)−(1−p)·gas fallo; no se adopta tasa de inclusión de terceros. Con 500k en éxito/fallo, revert cost es 0,005/0,015/0,03 ETH a 10/30/60 gwei. Todas las sensibilidades siguen negativas. Cuando el umbral exigiría p>100%, se registra UNATTAINABLE y null; no se presenta como probabilidad real o equilibrio alcanzable. MEV/selección adversa no se cuantifican por falta de evidencia.

## Fork: qué se probó y qué no

Foundry 1.7.1/Solc 0.8.24, forge-std 15b2df92a05d4d2f76a06653872fefb287e0e01c. Fixture aislado de investigación, no otro executor ni sender. Cada caso usa dos pools distintos reales y Morpho real al bloque final. Verifica exact output contra referencia, pago nominal vs delta de balances (detectaría FOT), factories, callbacks y conservación de balances. **Siempre fuerza rollback después de ambos swaps**, incluso si el bruto es positivo. No prueba repayment final, estimación enviable o ganancia realizada. No hubo fondos/reservas sintéticos ni firmas.

| Test | Resultado | Margen observado WETH | Gas call warm |
| --- | --- | ---: | ---: |
| test_candidate0 | PASS | 0.000002346700951391 | 249450 |
| test_candidate1 | PASS | -0.000000185608057284 | 238337 |
| test_candidate2 | SKIP: solo existen dos candidatos seleccionados | — | — |

El gas excluye gas intrínseco/calldata, cold-state, refunds y repago, e incluye la instrumentación/catch. Los dos casos son controles blue-chip; **cero long-tail** llegaron a fork por falta de contraparte utilizable. Las pruebas no avalan riesgos de los tokens long-tail ni al executor de producción. Una petición posterior de estado fue denegada por archive: no se cambió de bloque para obtener un pase.

## Top 20 verificables y razones de rechazo

No existen 20 candidatos long-tail de dos venues en esta cobertura; no se rellenan con direcciones inventadas. La tabla siguiente es el top-20 de *filtros de rutas del control* entre pools distintos al bloque final, no 20 oportunidades. Spread después de fees es marginal; quote_verified=false en este filtro. Solo las dos primeras tuvieron sizing/fork. Todas gas_verified=false y executable=false.

| # | Primer pool | Segundo pool | Spread marginal bp | Estado |
| --- | --- | --- | ---: | --- |
| 1 | `0xE0554a476A092703abdB3Ef35c80e0D76d32939F` | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | 3.92 | SIZED_CONTROL_NOT_PROFITABLE |
| 2 | `0xE0554a476A092703abdB3Ef35c80e0D76d32939F` | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | -1.84 | SIZED_CONTROL_NOT_PROFITABLE |
| 3 | `0xE0554a476A092703abdB3Ef35c80e0D76d32939F` | `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8` | -1.87 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 4 | `0xE0554a476A092703abdB3Ef35c80e0D76d32939F` | `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` | -1.98 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 5 | `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | -4.09 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 6 | `0xE0554a476A092703abdB3Ef35c80e0D76d32939F` | `0x7BeA39867e4169DBe237d55C8242a8f2fcDcc387` | -9.82 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 7 | `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | -9.86 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 8 | `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` | `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8` | -9.89 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 9 | `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` | `0xE0554a476A092703abdB3Ef35c80e0D76d32939F` | -10.01 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 10 | `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` | `0x7BeA39867e4169DBe237d55C8242a8f2fcDcc387` | -17.83 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 11 | `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8` | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | -54.14 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 12 | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | -54.17 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 13 | `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8` | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | -59.88 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 14 | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8` | -59.93 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 15 | `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8` | `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` | -60.01 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 16 | `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8` | `0xE0554a476A092703abdB3Ef35c80e0D76d32939F` | -60.03 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 17 | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | `0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640` | -60.04 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 18 | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | `0xE0554a476A092703abdB3Ef35c80e0D76d32939F` | -60.06 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 19 | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | -65.64 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |
| 20 | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` | `0x8ad599c3A0ff1De082011EFDDc58f1908eb6e6D8` | -65.66 | NONPOSITIVE_MARGINAL_FILTER; NOT_SIZED |

Rechazos principales: 63 ventanas RPC denegadas; 69 long-tail de pool único; 4 pools sin reservas/L activa; 1 metadata revertida; 37 eventos fuera de ancla WETH; 28 rutas de control sin sizing por política de alcance; 48 puntos sin neto positivo a 10 gwei. Son categorías de distintas unidades, no se suman como tokens únicos. El scanner/executor actual no adquiere soporte V3 por este trabajo; ninguna ruta descubierta queda autoejecutable.

## Tests y errores corregidos

- Unit tests: **14 PASS, 0 FAIL, 0 SKIP**. RPC simulado, eventos/identidad, pinning de quote V3, read-only denylist, sizing/brute force, gas/EV y límites de cobertura.
- Fork final: **2 PASS, 0 FAIL, 1 SKIP**; mismo resultado JSON/verbose, no se duplican como casos distintos.
- Primer unit run: 9 PASS/1 FAIL; el roundtrip con input mínimo producía cero intermedio y el segundo quote arrojaba excepción. Se corrigió el modelo para retornar cero, conservando la pérdida por redondeo.
- Primer fork: 0 PASS/2 FAIL/1 SKIP; asumía saldo inicial cero de la dirección del test, que tenía WETH histórico. Se corrigieron las assertions para preservar/comparar balances iniciales reales, **sin editarlos** ni usarlos como subsidio.
- Se corrigió el umbral EV para no presentar una probabilidad >100% como alcanzable. Quote inputs/reservas y tamaños no se cambiaron para forzar beneficio.

## Decisión y siguiente paso

**NO-GO** bajo Issue #15: no hay una sola ruta auténtica reproducible positiva después de los costes con profundidad y controles suficientes para justificar construir otro ejecutor. La conclusión es una puerta de proyecto, no una prueba de que Mainnet carezca de oportunidades. Faltan la mayor parte de siete días, venues no incluidos, pares no WETH, persistencia temporal amplia y pruebas de seguridad long-tail.

Prioridad siguiente: proporcionar acceso RPC histórico seguro, repetir toda la semana y observaciones de días/eventos distintos, ampliar factories solo después de verificar sus contratos/tarifas/quoters, y revisar tokens antes de ejecutar cualquier fork arbitrario acotado. Si no aparece un neto positivo robusto, investigar L2 con gas menor o estrategias no basadas en competir por el mismo arbitraje público; son hipótesis a medir, no promesas. No desarrollar otro sender ni habilitar fondos reales con estos resultados.

## Reproducción y artefactos

[Instrucciones](../research/long-tail/README.md). Dependencias fijadas por package-lock; no requiere claves privadas. HTTPS_URL se inyecta de forma segura; no colocar su valor en comandos/historial o reportes.

`npm ci --prefix research/long-tail` → `npm --prefix research/long-tail test` → `NODE_USE_ENV_PROXY=1 node research/long-tail/src/probe.mjs`. Un run nuevo elige su finalized; no mezclar sus resultados con este. Para replay exacto utilizar DISCOVERY_CHECKPOINT con [discovery.json](../research/long-tail/evidence/run-26437145-full/discovery.json), RUN_DIR nuevo y acceso al mismo estado. El checkpoint debe ser de confianza/inspeccionado y la sonda revalida chain/header/hash.

- [Descubrimiento y ventanas fallidas](../research/long-tail/evidence/run-26437145-full/discovery.json).
- [Inventario, tokens, riesgos y pools históricos](../research/long-tail/evidence/run-26437145-full/inventory.json).
- [Curvas, fuentes, flags, financiación, gas, EV y bloques](../research/long-tail/evidence/run-26437145-full/results.json).
- [Casos de fork seleccionados](../research/long-tail/evidence/run-26437145-full/fork-candidates.json).
- [Salida fork](../research/long-tail/evidence/run-26437145-full/fork-output.txt) / [JSON fork](../research/long-tail/evidence/run-26437145-full/fork-results.json).
- [Salida de los 14 unit tests](../research/long-tail/evidence/run-26437145-full/unit-output.txt).

Todos los archivos son evidencia de investigación: no contienen secretos ni instrucciones para enviar transacciones.
