# Validación de E4 (F10–F11)

Fecha: 12 de octubre de 2026. Base: E3, commit `5b11a50`. Aplicación 0.5.0; esquemas y reglas de simulación 1, 2 y 3. Implementación delegada a agentes gpt-6-luna con razonamiento max; planificación, revisión y aceptación centralizadas. Los mundos usados son sintéticos. No se han hecho commit, push ni publicación de E4.

## Alcance entregado

Fichas de persona, hogar y asentamiento por fuente, mundo, rama y cierre anual; nombres y descriptores ornamentales deterministas; hechos, genoma heredado, rasgos, residencia, profesión, afiliaciones, uniones múltiples, integrantes y cronología conocidos hasta ese corte. Un ID ausente produce `not_known_at_year` sin confirmar su existencia futura. La caché de presentación comparte hasta 256 entradas por catálogo y no escribe hechos ni consume RNG de simulación.

Genealogía por tipo y profundidad con páginas de hasta 100 relaciones; conservación de vínculos distintos entre la misma pareja. Familias amplias y uniones con más de 100 integrantes anuncian totales y tienen continuación. Exportaciones Markdown locales en español con mundo/rama/año, cantidades de página y escape de puntuación, HTML y etiquetas de diagramas. El árbol SVG tiene nodos operables por teclado y una tabla paginada alternativa.

Biblioteca de mundos y copias, editor mediante los JSON Schema locales 1–3, agenda editable, validación semántica antes de guardar/ramificar, controles de trabajos, progreso SSE con consulta periódica de respaldo, selección temporal visible, vistas históricas y económicas paginadas, edición futura desde un corte y comparación de censos, balances, dinero, bienes/precios y cultura. El borrador, contexto y trabajo activo se recuperan desde almacenamiento del navegador. Una API incompatible bloquea operaciones remotas y conserva el borrador.

Exportar SQLite, abrir una copia aislada e importar un archivo local conservan separados el original y la copia aunque sus IDs coincidan. El catálogo de copias pagina con `total/offset/limit/has_more` y persiste tras reiniciar. Hasta 16 contenedores de copia permanecen abiertos; solo se expulsan los inactivos. El importador limita el archivo a 64 MiB y valida esquema, integridad y referencias, sin reemplazar el catálogo original.

## Pruebas automatizadas

Pasan 100 pruebas Rust: 10 de aplicación, 18 de dominio, 47 de motor, 11 de consultas, 11 de almacén y tres integraciones de bases reales E1/E2/E3. Pasan formato Rust y Clippy con `-D warnings`, contratos TypeScript 5.9.3, comprobaciones de recursos locales y diez pruebas JavaScript: ocho del editor y dos de importes decimales exactos. Rust 1.90.0 y dependencias fijadas; compilación `--locked --offline`.

La [fixture pre-E4 de E3](validation/e3-pre-e4-compatibility.md) conserva los diez tríos originales de hashes de estado/eventos/commit. La nueva prueba abre y reproduce esos estados y eventos, avanza desde el cierre nueve al diez en memoria y comprueba que no reescribe los bytes ni hashes originales. Las integraciones existentes mantienen las garantías de bases E1 y E2. [Legacy conserva sus 60 entradas SHA-256](validation/e4-legacy.json).

`scripts/validate-e4.mjs` usa expectativas calculadas independientemente: una persona inicial fallece y nacen hijos con alelos heredados reales; los censos de los años 0, 1 y 2 son 4, 5 y 6. La expresión genética se contrasta con los alelos fijados de progenitores/hijo. Un vínculo biológico y otro de cuidado hacia la misma persona siguen siendo dos relaciones. Personas fallecidas, aún no conocidas y desconocidas mantienen estados honestos.

La aceptación consulta y exporta el año cero antes y después de avanzar, y durante otro trabajo activo. Contrasta estados y eventos con una rama sin consultas; comprueba ausencia de futuros, paginación, límites, Markdown con un nombre adversarial, esquema desconocido, progreso SSE terminal, rama con natalidad desactivada (4 habitantes frente a 6), importación corrupta rechazada, descarga binaria, copias con IDs repetidos, escritura aislada y recuperación después de reiniciar. Comprueba mediante SHA-256 que los cinco recursos web embebidos coinciden con el código revisado, evitando entregar un binario con módulos desactualizados.

Pasan el humo API y las aceptaciones independientes E1, E2, E3 y E4 con el ejecutable release. Se repiten sobre el binario extraído del paquete; cada servidor usa una base y directorio temporales, loopback y PATH sin toolchains ni intérpretes para la aplicación. Node se usa desde fuera únicamente como conductor de pruebas. No se necesita red externa para los recorridos ni se incluyen CDN, fuentes o módulos remotos.

## Recorrido gráfico comprobado

En el navegador integrado, usando solo controles visibles y un catálogo temporal: editar el nombre de un asentamiento, validar, guardar un mundo, simular hasta el cierre dos, consultar el año cero, abrir ficha y árbol, seguir a un familiar con Enter, y descargar una ficha Markdown cuyo contenido se inspecciona. Se edita la probabilidad de nacimientos mediante formulario, se crea una hija desde el corte cero, se simula y compara con la madre.

Se inicia otro trabajo, se pausa en un cierre confirmado, se cierra/reabre la pestaña y se recupera el trabajo pausado; se reanuda y cancela conservando el cierre. Se interrumpe y reinicia el servidor de prueba: aparece el estado desconectado y se recuperan selección y borrador. Un servidor local de prueba que declara API 99 muestra incompatibilidad y rechaza solicitudes. Se descarga una copia SQLite y se importa desde el selector de archivos: queda disponible como fuente independiente, con las dos ramas originales.

La navegación entre pestañas mediante flechas y los nodos del árbol mediante Enter funcionan. Las tablas son la alternativa textual de los gráficos; las regiones y controles tienen etiquetas y los resultados anuncian su contexto. La comprobación es con una pestaña cerrada/reabierta en el mismo perfil del navegador, no con un reinicio completo del sistema operativo ni con un lector de pantalla específico.

También se carga la demo de esquema 3, se cambia la prioridad de un evento mediante formulario, se valida, guarda y simula hasta el año tres; la vista de mercados muestra saldos, existencias, necesidades, intercambios y las consecuencias realizadas en el año uno. Se inspeccionan ficha y árbol en una ventana de 614 píxeles de ancho: los controles caben y las tablas/diagrama largos tienen desplazamiento contenido.

## Compatibilidad determinista

| Perfil | Huella E3 de referencia | Huella E4 |
| --- | --- | --- |
| [B0 E1](validation/e4-b0.json) | `8abe550c435eed83` | `8abe550c435eed83` |
| [E2-B0](validation/e4-e2-b0.json) | `53ce4b5fcd174c7c` | `53ce4b5fcd174c7c` |
| [E3-B0](validation/e4-e3-b0.json) | `9b8bb7d23681a2da` | `9b8bb7d23681a2da` |

Estas ejecuciones pequeñas comprueban compatibilidad, no acreditan rendimiento bajo carga. No se modifica el motor ni el dominio demográfico/económico en E4.

## Paquete y límites reales

El paquete Linux `dist/chargen-0.5.0-linux-x86_64.tar.gz` contiene ejecutable, interfaz y recursos embebidos, documentación, manifiesto y checksum. El manifiesto identifica la base Git y una huella SHA-256 del árbol, con `source_dirty: true` porque E4 permanece sin commit. Se verifica el paquete extraído y se repiten aceptaciones y las tres huellas anteriores desde su ejecutable.

La [CI del commit E3](validation/e4-e3-ci.json) pasó Linux, Windows y la comparación de las tres huellas en [GitHub Actions](https://github.com/ElrikPiro/chargen/actions/runs/38249080204). Ese resultado acredita E3; Windows para los cambios E4 queda pendiente de ejecutar la matriz tras un commit/push autorizado. Esta máquina valida E4 en Linux x86_64.

Las fichas exportadas incluyen una página, con recuento explícito y continuación disponible en la aplicación/API; no prometen volcar árboles o censos completos en un único archivo. El dibujo se limita a 40 nodos por página y conserva la tabla completa. Profundidad máxima 64, presupuesto de recorrido genealógico 10.000 y offset del servicio predeterminado 10.000, con límite indicado; la escala integral de 50.000 habitantes durante 1.000 años pertenece a E5. Los trabajos permanecen en memoria mientras vive el servicio; después de reiniciarlo, los cierres históricos conservados permiten iniciar una nueva corrida.

## Reproducción

Con las variables de Rust aisladas en `.tooling/`: `cargo test --workspace --all-targets --locked --offline`, `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` y `cargo fmt --all -- --check`. Ejecutar cada suite JS directamente: `node --experimental-default-type=module web/scenario-editor.test.js` y `node --experimental-default-type=module web/economic-amounts.test.js`; después `node scripts/check-web.mjs` y TypeScript sobre `web/tsconfig.json`.

Para un binario dado, ejecutar `node scripts/smoke-api.mjs <binario>` y `node scripts/validate-eN.mjs <binario>` para N=1,2,3,4. El recorrido gráfico usa un catálogo de prueba propio y `serve --bind 127.0.0.1:18604 --no-open`. Los controles finales de paquete quedan registrados en `validation/e4-package-checks.json`.
