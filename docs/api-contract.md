# API local de E0–E3

La interfaz se sirve y consulta desde el mismo ejecutable en `127.0.0.1`. La app no habilita CORS para orígenes externos ni descarga recursos remotos. JSON usa nombres `snake_case`; IDs de mundo y rama son cadenas estables, los IDs numéricos del dominio se serializan como enteros JSON.

Los límites predeterminados son 64 MiB por solicitud JSON, 100 filas por página y 10.000 filas como máximo de desplazamiento. El servidor valida el límite de cuerpo antes de deserializar, los enteros de paginación y la pertenencia mundo/rama/año. `limit > 100`, offsets negativos, IDs excesivamente largos, años no cerrados y escenarios con errores reciben un error estructurado sin comenzar trabajo.

Las formas serializadas están descritas en [`schemas/scenario-v1.schema.json`](../schemas/scenario-v1.schema.json), [`schemas/scenario-v2.schema.json`](../schemas/scenario-v2.schema.json) y [`schemas/scenario-v3.schema.json`](../schemas/scenario-v3.schema.json); los tipos de cliente están en [`web/contracts.ts`](../web/contracts.ts). La API valida además las relaciones y límites semánticos con el dominio Rust; el esquema JSON no sustituye esa validación.

Los escenarios E1 conservan `schema_version: 1` y `rules_version: 1`. E2 usa `schema_version: 2` y `rules_version: 2`, y declara `territory` y `culture`. E3 usa `schema_version: 3` y `rules_version: 3`, y declara `economy` y `scheduled_events`, incluso si la agenda está vacía. Los campos desconocidos se rechazan. La configuración omitida en una historia E1/E2 permanece omitida y no activa procesos nuevos al cargarla. El alcance E2 está en [`e2-scope.md`](e2-scope.md); el alcance E3 está en [`e3-scope.md`](e3-scope.md).

## Rutas

| Método y ruta | Entrada | Respuesta |
| --- | --- | --- |
| `GET /api/health` | — | `{version, offline}` |
| `GET /api/demo-scenario` | — | Escenario demostrativo ficticio en JSON |
| `GET /api/demo-scenario-e2` | — | Demo ficticia E2 con rutas temporales, movimiento y reglas culturales |
| `GET /api/demo-scenario-e3` | — | Demo ficticia E3 con profesiones productoras, dinero explícito y un evento acotado |
| `POST /api/scenarios/validate` | `{scenario}` | `{valid, issues}`; cada error tiene `path`, `code`, `message` |
| `GET /api/worlds` | — | Lista de mundos persistidos |
| `POST /api/worlds` | `{scenario}` | Mundo creado, rama raíz y año inicial; 409 si el ID ya existe |
| `GET /api/worlds/{world_id}/branches` | — | Ramas con padre, corte, head y revisión |
| `POST /api/worlds/{world_id}/branches` | `{parent_branch_id, branch_id, cut_year, scenario}` | Rama hija creada desde el cierre confirmado; `cut_year: null` crea la rama antes del primer cierre |
| `GET /api/worlds/{world_id}/branches/{branch_id}/census?year=&settlement_id=&limit=&offset=` | — | Censo agregado y paginado para el cierre solicitado |
| `GET /api/worlds/{world_id}/branches/{branch_id}/people?year=&settlement_id=&limit=&offset=` | — | Personas visibles en ese cierre y rama |
| `GET /api/worlds/{world_id}/branches/{branch_id}/people/{person_id}?year=` | — | Proyección temporal de la persona, o 404 si aún no nació/no existe |
| `GET /api/worlds/{world_id}/branches/{branch_id}/people/{person_id}/genealogy?year=&depth=` | — | Relaciones registradas hasta el cierre, con profundidad acotada |
| `GET /api/worlds/{world_id}/branches/{branch_id}/settlements?year=&limit=&offset=` | — | Asentamientos, residentes y balances de movimiento visibles al cierre |
| `GET /api/worlds/{world_id}/branches/{branch_id}/routes?year=&limit=&offset=` | — | Rutas dirigidas y abiertas en el año consultado, según la revisión de esa rama |
| `GET /api/worlds/{world_id}/branches/{branch_id}/migration?year=&settlement_id=&limit=&offset=` | — | Balance anual confirmado de entradas y salidas por asentamiento |
| `GET /api/worlds/{world_id}/branches/{branch_id}/markets?year=&settlement_id=&limit=&offset=` | — | Grupos económicos y sus saldos actuales e históricos al cierre |
| `GET /api/worlds/{world_id}/branches/{branch_id}/markets/settlements?year=&settlement_id=&limit=&offset=` | — | Mercado por bien/asentamiento: precios, necesidades, demanda solvente, intercambios y existencias |
| `GET /api/worlds/{world_id}/branches/{branch_id}/markets/trades?year=&settlement_id=&limit=&offset=` | — | Intercambios realizados, pagos y costes de transporte, paginados |
| `GET /api/worlds/{world_id}/branches/{branch_id}/events?year=&settlement_id=&limit=&offset=` | — | Eventos y efectos realizados hasta el cierre consultado, paginados |
| `GET /api/worlds/{world_id}/branches/{branch_id}/culture?year=&settlement_id=&limit=&offset=` | — | Suma ponderada, peso, miembros elegibles y media de rasgos culturales |
| `GET /api/worlds/{world_id}/branches/{branch_id}/institutions?year=&settlement_id=&person_id=&limit=&offset=` | — | Definiciones y afiliaciones vigentes a ese cierre |
| `GET /api/worlds/{world_id}/branches/{branch_id}/institutions/{institution_id}/members?year=&settlement_id=&person_id=&limit=&offset=` | — | Afiliaciones activas paginadas para una institución |
| `GET /api/worlds/{world_id}/branches/{branch_id}/organizations?year=&settlement_id=&limit=&offset=` | — | Organizaciones fundadas hasta el año, fundadores, integrantes y disolución visible |
| `GET /api/worlds/{world_id}/branches/{branch_id}/organizations/{organization_id}/members?year=&limit=&offset=` | — | IDs de integrantes activos de la organización en ese cierre; lista paginada |
| `GET /api/worlds/{world_id}/branches/{branch_id}/people/{person_id}/residence?year=&limit=&offset=` | — | Residencia y profesiones con intervalos visibles de la persona |
| `GET /api/worlds/{world_id}/branches/{branch_id}/people/{person_id}/affiliations?year=&limit=&offset=` | — | Afiliaciones institucionales y membresías de organización por intervalo |
| `POST /api/worlds/{world_id}/branches/{branch_id}/runs` | `{through_year}` | Trabajo asíncrono con ID y estado inicial |
| `GET /api/runs/{job_id}` | — | Estado, año confirmado, pasos y error recuperable |
| `POST /api/runs/{job_id}/pause` | — | Solicita pausa al cierre anual siguiente |
| `POST /api/runs/{job_id}/resume` | — | Continúa un trabajo pausado |
| `POST /api/runs/{job_id}/cancel` | — | Cancela en el próximo límite seguro y conserva el último commit |
| `POST /api/worlds/{world_id}/branches/{branch_id}/checkpoint` | — | Checkpoint del último cierre confirmado |
| `POST /api/worlds/{world_id}/export` | — | Exportación SQLite coherente y su manifiesto |
| `POST /api/restore` | `{export_id}` | Valida y restaura una copia de exportación a un slot local independiente |

Para iniciar una corrida el servidor lee la revisión de escenario persistida. `through_year` es inclusivo y representa el cierre anual visible. El motor corre fuera del event loop. En cada año aplica cambios sobre estado de trabajo, valida y hace commit; solo después publica el nuevo progreso. El escritor exclusivo vive durante ese trabajo.

Las consultas E2/E3 abren el mismo contexto inmutable `(world_id, branch_id, year)` que las consultas de censo y genealogía. Un intervalo de residencia, afiliación o membresía es semiabierto: `start` se incluye y `end_exclusive` no. Las organizaciones se muestran solo después de su fundación y dejan de estar activas en el cierre de `dissolved_year`; el endpoint de integrantes pagina los IDs activos y el endpoint de afiliaciones de persona conserva el historial individual. Las rutas listadas son las abiertas al cierre consultado; la respuesta omite el calendario restante de apertura. El promedio agregado de cultura usa redondeo al entero más cercano, con empates alejándose de cero; en ausencia de peso se devuelve `null`, sin inventar valor para un asentamiento vacío. El valor cultural heredado al nacer usa la política de mezcla del motor y trunca su resultado hacia cero.

Las rutas E3 de mercado leen el libro anual ya confirmado; no ejecutan compras ni recalculan precios. Dinero y cantidades se devuelven como cadenas decimales para evitar pérdida de precisión en clientes JavaScript. `/markets` incluye cuentas de grupo y tesorería (dinero y existencias), y paginación por grupo; `/markets/settlements` expone el resumen por bien; `/markets/trades` contiene compras y ventas efectivamente anotadas. Necesidad física, demanda solvente y cantidades sin atender se exponen por separado. La ruta de eventos devuelve efectos realizados hasta el año seleccionado, nunca la agenda futura declarada en el escenario; los efectos temporales solo se muestran activos dentro de su intervalo, mientras el daño permanente y su causa permanecen en la historia.

En instituciones de ámbito global, el filtro de asentamiento selecciona residentes actuales; en instituciones de ámbito local, selecciona el asentamiento de anclaje de la afiliación. Ese anclaje se conserva si la política permite que una afiliación continúe después de una migración. El endpoint de afiliaciones de persona expone dicho lugar junto a sus intervalos.

Las solicitudes fallidas usan `{code, message, details?}` y un status HTTP apropiado (`400` entrada inválida, `404` recurso inexistente, `409` conflicto con corrida/escritor, `413` cuerpo mayor al límite, `500` error interno). La respuesta no revela rutas privadas del sistema ni trazas de SQLite.

## Identificadores y ramas

La rama raíz se llama `main`. El `world_id` del contrato de escenario se transforma en su representación decimal estable para el índice de la historia local. La revisión de escenario de una rama se valida completa antes de pedir al almacén que materialice el corte. Para un cambio efectivo desde el principio de Y, el corte compartido es Y−1. En un corte confirmado, pueden revisarse políticas demográficas, territoriales, culturales y económicas para los cierres posteriores; identidades y entidades iniciales, perfiles, catálogos de rasgos y genética permanecen fijos. En E3, los IDs y unidades de bienes, dotaciones iniciales y eventos con inicio en el corte o antes permanecen intactos. Las órdenes de desplazamiento fechadas en el corte o antes y las instituciones/religiones ya referenciadas por el prefijo no se pueden reescribir ni eliminar. Si aún no hay un cierre anual, se usa `cut_year: null`; si se solicita un año que no está confirmado, no se crea la rama. El almacén comprueba que el prefijo compartido siga siendo compatible con la nueva revisión.

## Estado del prototipo

La UI permite creación/validación de escenario, selección de mundo/rama/cierre, simulación con progreso, pausa/reanudación/cancelación y consultas temporales de censo, residencia, rutas, cultura, organizaciones, mercados y eventos. Rama, checkpoint, genealogía, transferencia y las consultas paginadas de grupos/mercados/eventos también se publican como API. El servicio local no ofrece autenticación: se limita a loopback y no debe configurarse para escuchar en una interfaz de red.
