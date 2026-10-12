# Decisiones de arquitectura para E0/E1

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
