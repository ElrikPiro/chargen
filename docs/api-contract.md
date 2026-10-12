# API local de E0/E1

La interfaz se sirve y consulta desde el mismo ejecutable en `127.0.0.1`. La app no habilita CORS para orígenes externos ni descarga recursos remotos. JSON usa nombres `snake_case`; IDs de mundo y rama son cadenas estables, los IDs numéricos del dominio se serializan como enteros JSON.

Los límites predeterminados son 64 MiB por solicitud JSON, 100 filas por página y 10.000 filas como máximo de desplazamiento. El servidor valida el límite de cuerpo antes de deserializar, los enteros de paginación y la pertenencia mundo/rama/año. `limit > 100`, offsets negativos, IDs excesivamente largos, años no cerrados y escenarios con errores reciben un error estructurado sin comenzar trabajo.

La forma serializada del escenario está descrita en [`schemas/scenario-v1.schema.json`](../schemas/scenario-v1.schema.json) y sus contratos de cliente están en [`web/contracts.ts`](../web/contracts.ts). La API valida además las relaciones y límites semánticos con el dominio Rust; el esquema JSON no sustituye esa validación.

## Rutas

| Método y ruta | Entrada | Respuesta |
| --- | --- | --- |
| `GET /api/health` | — | `{version, offline}` |
| `GET /api/demo-scenario` | — | Escenario demostrativo ficticio en JSON |
| `POST /api/scenarios/validate` | `{scenario}` | `{valid, issues}`; cada error tiene `path`, `code`, `message` |
| `GET /api/worlds` | — | Lista de mundos persistidos |
| `POST /api/worlds` | `{scenario}` | Mundo creado, rama raíz y año inicial; 409 si el ID ya existe |
| `GET /api/worlds/{world_id}/branches` | — | Ramas con padre, corte, head y revisión |
| `POST /api/worlds/{world_id}/branches` | `{parent_branch_id, branch_id, cut_year, scenario}` | Rama hija creada desde el cierre confirmado; `cut_year: null` crea la rama antes del primer cierre |
| `GET /api/worlds/{world_id}/branches/{branch_id}/census?year=&settlement_id=&limit=&offset=` | — | Censo agregado y paginado para el cierre solicitado |
| `GET /api/worlds/{world_id}/branches/{branch_id}/people?year=&settlement_id=&limit=&offset=` | — | Personas visibles en ese cierre y rama |
| `GET /api/worlds/{world_id}/branches/{branch_id}/people/{person_id}?year=` | — | Proyección temporal de la persona, o 404 si aún no nació/no existe |
| `GET /api/worlds/{world_id}/branches/{branch_id}/people/{person_id}/genealogy?year=&depth=` | — | Relaciones registradas hasta el cierre, con profundidad acotada |
| `POST /api/worlds/{world_id}/branches/{branch_id}/runs` | `{through_year}` | Trabajo asíncrono con ID y estado inicial |
| `GET /api/runs/{job_id}` | — | Estado, año confirmado, pasos y error recuperable |
| `POST /api/runs/{job_id}/pause` | — | Solicita pausa al cierre anual siguiente |
| `POST /api/runs/{job_id}/resume` | — | Continúa un trabajo pausado |
| `POST /api/runs/{job_id}/cancel` | — | Cancela en el próximo límite seguro y conserva el último commit |
| `POST /api/worlds/{world_id}/branches/{branch_id}/checkpoint` | — | Checkpoint del último cierre confirmado |
| `POST /api/worlds/{world_id}/export` | — | Exportación SQLite coherente y su manifiesto |
| `POST /api/restore` | `{export_id}` | Valida y restaura una copia de exportación a un slot local independiente |

Para iniciar una corrida el servidor lee la revisión de escenario persistida. `through_year` es inclusivo y representa el cierre anual visible. El motor corre fuera del event loop. En cada año aplica cambios sobre estado de trabajo, valida y hace commit; solo después publica el nuevo progreso. El escritor exclusivo vive durante ese trabajo.

Las solicitudes fallidas usan `{code, message, details?}` y un status HTTP apropiado (`400` entrada inválida, `404` recurso inexistente, `409` conflicto con corrida/escritor, `413` cuerpo mayor al límite, `500` error interno). La respuesta no revela rutas privadas del sistema ni trazas de SQLite.

## Identificadores y ramas

La rama raíz se llama `main`. El `world_id` del contrato de escenario se transforma en su representación decimal estable para el índice de la historia local. La revisión de escenario de una rama se valida completa antes de pedir al almacén que materialice el corte. Para un cambio efectivo desde el principio de Y, el corte compartido es Y−1. Si aún no hay un cierre anual, se usa `cut_year: null`; si se solicita un año que no está confirmado, no se crea la rama. El almacén comprueba que el prefijo compartido siga siendo compatible con la nueva revisión.

## Estado del prototipo

La UI E1 muestra creación/validación de escenario, selección de mundo/rama, simulación con progreso, pausa/reanudación/cancelación y censo. Las rutas de consulta individual/genealógica, rama, checkpoint y transferencia se publican como API para completar recorridos posteriores. El servicio local no ofrece autenticación: se limita a loopback y no debe configurarse para escuchar en una interfaz de red.
