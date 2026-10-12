# Alcance y aceptación de E4 (F10–F11)

E4 entrega consultas históricas de presentación y los recorridos locales para crear, simular, consultar, exportar y ramificar mundos. El backend fija cada lectura a `(world_id, branch_id, year)` y devuelve la revisión de escenario visible en ese cierre. Una revisión heredada de la rama madre se mantiene visible en el corte inclusivo; sin año, el editor recibe la revisión actual de la rama.

## F10: fichas y narración

- Persona, hogar y asentamiento se proyectan desde el estado confirmado; fichas/eventos/relaciones no escriben estado ni usan RNG del simulador.
- La ficha de persona identifica el asentamiento y muestra residencia, profesión, afiliaciones institucionales/organizativas y uniones vigentes o históricas al corte, con intervalos recortados; el genoma presentado procede solo de loci ya fijados.
- Las uniones mantienen todos sus participantes y asociaciones múltiples. La ficha carga hasta 100 uniones y 100 participantes por unión; `/people/{id}/unions` y `/unions/{id}/members` permiten continuar ambas dimensiones con paginación acotada.
- La persona incluye hechos y relaciones ya conocidos al cierre, `template_version`, `fact_fingerprint`, estado de vida visible y recuentos de cronología/vínculos truncados. Una persona ausente produce `not_known_at_year` sin consultar el futuro.
- Presentación se cachea por mundo, rama, cierre, identidad lógica, versión de plantilla y huella de hechos; cada servicio comparte una caché LRU limitada solo a valores de presentación. El contenedor de una copia restaurada tiene otra caché.
- El árbol pagina enlaces vigentes y conserva vínculos de tipos distintos entre la misma pareja. `limit` no supera 100; `depth` no supera 64 y el recorrido tiene un presupuesto de nodos.
- Plantillas y exportaciones son locales y en español. Markdown y etiquetas de diagramas escapan texto; la ficha de asentamiento exporta una sola página de residentes.
- Los hechos y funciones de consulta no acceden a `rules_version: 4`, recetas futuras ni datos posteriores al corte.

## F11: aplicación y API

- El editor descarga JSON Schema Draft 2020-12 embebido para cada versión soportada (1, 2 o 3), presenta las rutas/ramas persistidas y valida escenario antes de crear o ramificar.
- Selector y consultas mantienen la fuente (original o copia), mundo, rama y año visibles. Hogares, personas, eventos, mercados y árboles anuncian el total y permiten cargar páginas.
- Comparación exige rama y año en ambos lados y devuelve en filas acotadas censos, balances demográficos, economía por bien/precio, dinero agregado y cultura disponible. Un error contextual aparece como error explícito.
- SSE de progreso envía snapshots del trabajo en `/runs/{job_id}/progress`; el endpoint de snapshot sigue habilitado como fallback.
- La descarga de SQLite y el Markdown son archivos locales. Un restore abre una copia aislada bajo `api_base`, incluso cuando comparte `world_id` con la base original. El catálogo puede reabrirse después de reiniciar y los trabajos continúan usando el mismo actor/contenedor durante requests.
- Importar un archivo local envía bytes SQLite con límite de 64 MiB; el servidor valida esquema e integridad y nunca evalúa archivos. Errores de corrupción, tamaño o versión incompatible son visibles y no reemplazan el catálogo original.
- Recursos web, esquema y código se distribuyen dentro del ejecutable y no usan conexiones de red externas. La UI ofrece navegación por teclado y tabla para los resultados.

## Límites y evidencia

Las respuestas de fichas tienen páginas de hasta 100 elementos y offsets configurables hasta el límite del servicio. El catálogo de restauraciones y el número de contenedores abiertos están acotados; solo se expulsa un contenedor sin requests activos ni trabajos activos. Los ficheros restaurados persisten aunque se cierre su contenedor en memoria.

La aceptación E4 ejecuta una base sintética con persona fallecida, futuro aún no conocido, cuidador y nombre adversarial; verifica lectura antes/después de evolucionar sin cambio de historia, páginas de árbol, escapes Markdown, rama de comparación, exportación, restauración/importación binaria, aislamiento de ID repetido y reapertura tras reinicio. Los checks del workflow se ejecutan en Linux y Windows; el resultado de una máquina Linux local no acredita Windows. El presupuesto de rendimiento de 50.000 habitantes y 1.000 años se valida en E5.
