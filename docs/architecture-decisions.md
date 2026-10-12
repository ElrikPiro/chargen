# Decisiones de arquitectura para E0/E2

## ADR-001: servidor local con recursos estáticos

La aplicación sirve `web/` desde el binario Rust. Para E0/E1 se usa HTML, CSS y JavaScript nativos; React/Vite añadiría proceso de construcción y dependencias sin aportar una necesidad a las pantallas del prototipo. Los tipos TypeScript se mantienen como contrato verificable de la API, pero el paquete final no requiere Node.

Consecuencia: no hay recursos de terceros, CDN, telemetría externa ni petición de red fuera de loopback en la UI. El servidor valida que su dirección sea loopback y rechaza una dirección pública.

## ADR-002: límites del contenedor

`ServiceConfig` contiene puerto, ruta de datos y límites de solicitudes. `AppContainer` se construye explícitamente a partir de configuración, almacén, consultas, reloj de medición y canales de progreso. Los handlers no buscan servicios globales. El dominio no recibe HTTP ni rutas de archivos.

Una simulación ejecuta el motor en un hilo bloqueante de Tokio porque cada año puede consumir CPU. El token de cancelación se comparte con el trabajo y se revisa en los límites anuales. El `WorldWriter` se mantiene durante la corrida; cada año se confirma en SQLite antes de publicar progreso. Si falla el guardado, no se comunica el cierre como terminado.

## ADR-003: estado local e histórico

E0/E1 usa una base SQLite local única. La historia y el estado recuperable se escriben por transacciones anuales; consultas fijan mundo, rama y cierre anual. El motor reconstruido desde escenario versionado y estado confirmado es temporal; la base durable decide desde qué cierre se reanuda.

Una rama referencia el prefijo inmutable de su madre hasta el corte. Las consultas resuelven los segmentos válidos para mundo/rama/año y no utilizan el último año de forma implícita.

## ADR-004: UI pequeña y pruebas de contrato

La interfaz de E1 prioriza abrir contexto, crear/validar escenario, simular, observar progreso y consultar censos. El backend expone además consultas individuales y genealógicas, ramas, exportación y restauración. Las respuestas y solicitudes se describen en `web/contracts.ts` y se comprueban con TypeScript y casos de API.

## ADR-005: configuración tipada, aleatoriedad y reloj

Las reglas de mundo viven en los tipos `Scenario` y `DemographicPolicies` de `chargen-domain`; `Scenario::validate` produce el `ValidatedScenario` que recibe `chargen-engine`. La composición del servicio (`ServiceConfig`) no se mezcla con las políticas del mundo. Cada política conserva tipos, unidades y límites propios; añadir un proceso futuro requiere una versión de reglas y validación explícita.

El motor aplica la semilla y `rules_version` del escenario a decisiones aleatorias identificadas por `DecisionDomain`; el algoritmo se etiqueta `chargen-splitmix-keyed-v1` y su constante pública `RNG_VERSION`. Los IDs de almacenamiento no forman parte de las claves RNG. Cambiar mezcla, dominios u orden de decisiones requiere versionar el contrato. No se comparte un generador mutable entre corridas ni entre una consulta y una simulación.

El reloj inyectable del motor (`MetricsClock`) y el del servicio (`MeasurementClock`) miden duración monotónica solamente. Nunca alimentan años, decisiones, IDs o estado persistido. El sink `MetricsSink` recibe métricas después de confirmar el commit anual. La prueba del worker usa reloj, escritor y sink falsos para comprobar el contrato sin depender del tiempo real o de SQLite.

## ADR-006: territorio, cultura y compatibilidad de reglas

E2 incorpora territorio y cultura como opciones tipadas del escenario. `rules_version: 1` deja ambas configuraciones ausentes y conserva las fases E1; `rules_version: 2` las valida antes de crear o bifurcar un mundo. Las rutas son dirigidas y versionadas, los cambios de residencia se expresan como intervalos, y los eventos anuales se confirman junto al estado. Las consultas reciben explícitamente mundo, rama y año y nunca actualizan agregados ni historia.

La persistencia genérica de estado/delta permite almacenar las nuevas secciones sin reinterpretar el prefijo E1. Los campos E2 ausentes o vacíos deben omitirse al serializar el estado E1 para mantener las huellas históricas; los lectores aceptan sus valores por defecto. La carga comprueba los hashes existentes antes de usar la forma tipada para continuar una corrida. El caso de compatibilidad usa una exportación SQL de una base E1 real con sus hashes originales.

La interfaz sigue siendo HTML/CSS/JavaScript estático servido por el ejecutable. Sus consultas nuevas usan el mismo selector temporal y la misma API local; TypeScript y JSON Schema documentan el cableado, mientras la validación semántica se mantiene en Rust. Las reglas de movimiento no generan guerras ni epidemias (F09), y las profesiones no representan una economía (F08).

## ADR-007: economía agregada y agenda causal de E3

E3 añade `EconomicConfig` y la agenda explícita de eventos bajo `schema_version: 3` y `rules_version: 3`. El agente económico es `(asentamiento, profesión)`; `profession_id: null` representa el grupo de consumidores. Inventarios y dinero son enteros con operaciones comprobadas. `Money` y `Quantity` se exponen como cadenas decimales en la API para conservar precisión en clientes JavaScript. No existe dinero, crédito, subsidio ni recurso implícito.

El cierre anual guarda en `State.economy.last_year_report` el libro de bienes, dinero, necesidades y comercio. `HistoryQuery::open(world, branch, year)` lee ese reporte desde el estado histórico confirmado. Compras y ventas no se emiten como un evento por cada transacción; el reporte anual es su fuente consultable completa. Los shocks de agenda sí generan eventos semánticos con resultados realizados y referencias de entidad indexadas.

La agenda configurada es una entrada de reglas, no una proyección histórica. Las consultas muestran solo inicio y efectos ya realizados hasta su corte; no devuelven eventos programados futuros. La duración de una restricción se representa con un intervalo semiabierto y la selección se materializa al comienzo del evento para no volver a tirar al consultar o reanudar. Los efectos económicos por evento quedan identificados en las pérdidas y el balance anual.

Las consultas de E3 se exponen como páginas limitadas (`markets`, `markets/settlements`, `markets/trades`, `events`) sobre el `HistoricalView` ya fijado. Así, sus proyecciones no abren un contexto global ni acceden al futuro de una rama madre. Los eventos globales se indexan por `EntityRef::World` para paginar por rama/año sin cargar toda la historia en memoria.

Los escenarios E1/E2 mantienen la configuración nueva ausente y los campos de estado vacíos se omiten al serializar. E3 no cambia sus hashes, reglas, fixtures SQL o resultados reproducibles. Las revisiones de rama solo aplican economía y eventos futuros: catálogo e IDs de bienes, unidades, dotaciones iniciales y agenda que ya comenzó antes o en el corte no se reescriben.

## ADR-008: fichas E4 y proyección solo lectura

Fichas y narración se derivan de `HistoryQuery` anclado a mundo/rama/cierre. La consulta solo lee el estado confirmado, eventos materializados hasta ese año y la revisión de escenario visible por la línea de ramas. El endpoint legado de persona y el árbol E3 mantienen sus respuestas; E4 añade rutas de dossier/eventos y una página genealógica nueva.

Los nombres/descriptores viven en una LRU de presentación acotada, inyectada por `ContainerBuilder`; cada fuente restaurada construye su propia caché. La clave contiene contexto temporal, identidad lógica, versión de plantilla y huella de hechos. La caché nunca comparte el estado del motor, libros económicos ni decisiones aleatorias. Los exportadores reciben proyecciones listas y escapan Markdown; no guardan la narración en SQLite.

Las páginas de presentación tienen un tope de 100 elementos, los árboles limitan su profundidad y presupuesto, y el timeline de ficha declara si truncó. Uniones y participantes se paginan por separado para conservar grupos y múltiples parejas sin una expansión no acotada. Los nombres de lugar y las afiliaciones proceden de catálogos visibles al cierre; los intervalos de residencia, afiliación, profesión y unión se recortan a ese año. El dossier puede representar ausencia neutral (`not_known_at_year`) y no hace una segunda lectura del head para inferir nacimientos futuros. Hechos futuros de profesión/fallecimiento y fin de uniones se recortan antes de serializar.

## ADR-009: comparación contextual y biblioteca de restauraciones

La comparación no elige defaults ocultos: rama y año de ambos lados deben abrir como cierres confirmados. Las filas acotadas contienen la unión de asentamientos y proyecciones a ambos lados, con balances demográficos, economía por bien/precio y cultura disponible; los libros monetarios/tesorería se incluyen por contexto. Los errores no se sustituyen por un censo aproximado.

Una restauración ocupa un archivo SQLite local propio y se sirve bajo un prefijo API aislado `/api/restores/{restored_id}`. El contenedor de la copia conserva su fábrica de consultas, caché de presentación, directorios de exportación/restauración, trabajos y registro de jobs entre requests. El catálogo en disco permite encontrar la copia después de reiniciar; el mapa en memoria es acotado y solo expulsa contenedores sin requests ni trabajos activos. Nunca se cambia el path global de la base activa ni se remapean IDs o hashes. El import local valida SQLite, esquema e integridad antes de registrarse; el cuerpo binario está limitado a 64 MiB y se procesa como datos, no como código.

El progreso de trabajos se publica como SSE y se conserva un snapshot HTTP para polling/reconexión. Cerrar o abrir el navegador no duplica ni interrumpe un actor de trabajo persistente de la aplicación.
