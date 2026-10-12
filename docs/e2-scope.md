# Alcance de E2: territorio, cultura y organizaciones

E2 incorpora F06–F07 como una extensión versionada de E1. La demo y los catálogos incluidos son inventados para probar reglas, persistencia y consultas; no se presentan como datos históricos o demográficos.

La demo navegable se carga desde `/api/demo-scenario-e2` o con el botón **Cargar demo E2**. Incluye dos asentamientos, una ruta que abre para los cierres 1–2, un desplazamiento programado en el año 1 y reglas ficticias de afiliación y organización. La interfaz conserva también la demo E1 para comprobar compatibilidad.

## F06: asentamientos y migración

- Los escenarios E2 declaran un grafo de rutas dirigidas con revisión, costo, capacidad anual e intervalos de apertura. No se infiere un trayecto de una ruta inversa.
- Cada persona conserva intervalos de residencia por cierre anual. Los traslados voluntarios se proponen desde una fotografía común y se arbitran de forma determinista frente a los límites de rutas y destinos.
- El desplazamiento forzado solo ocurre cuando una orden explícita del escenario lo solicita. Los cierres de ruta y los desplazamientos de las fixtures son comandos programados visibles, no una propagación automática de guerras, epidemias o desastres de F09.
- El movimiento conserva una identidad por persona, actualiza la residencia y las estructuras asociadas y registra el balance de salidas y entradas por asentamiento.

## F07: cultura e instituciones

- La cultura de un asentamiento se consulta como agregado de los valores de sus habitantes: suma ponderada, peso total, cantidad de miembros y valor entero resultante. Un asentamiento vacío no recibe un valor inventado.
- La consulta del agregado redondea la media al entero más cercano; los empates se alejan de cero. La mezcla usada para asignar cultura a una persona al nacer trunca hacia cero según sus pesos configurados. Son operaciones distintas.
- El valor cultural adulto queda fijo. En los nacimientos, el motor puede combinar rasgos parentales, la cultura local del asentamiento y la variación configurada; después, el recién nacido contribuye al agregado correspondiente.
- Instituciones y religiones son definiciones preconfiguradas con afiliaciones por intervalos temporales. Las consultas muestran la afiliación válida al cierre escogido y el historial visible hasta ese año.
- Una afiliación de ámbito local conserva su asentamiento de anclaje; puede continuar tras la migración si su política lo permite. Las consultas por persona distinguen el anclaje institucional de la residencia actual.
- Las organizaciones tienen fundadores, membresías y posible disolución con fechas. El agregado de miembros solo incluye afiliaciones activas en el cierre consultado; fundadores y cambios históricos permanecen consultables después de la disolución.
- La creación y disolución de organizaciones utiliza límites y cadencias explícitos para evitar crecimiento descontrolado. Las consultas no crean entidades ni alteran la cultura adulta.

## Compatibilidad temporal

Los escenarios E1 usan `schema_version: 1`, `rules_version: 1` y carecen de configuración territorial y cultural. Los campos E2 ausentes se deserializan con valores por omisión compatibles; la configuración ausente mantiene la ejecución E1. E2 usa `schema_version: 2`, `rules_version: 2` y declara territorio y cultura. Una versión de reglas se conserva en la historia y no se reinterpreta al cargar una base existente. En ramas con prefijo cerrado, las nuevas políticas territoriales y culturales se aplican desde el cierre siguiente. No se pueden cambiar entidades iniciales, identidades, perfiles, catálogo de rasgos ni genética; las órdenes programadas ya fechadas dentro del prefijo y las definiciones institucionales/religiosas usadas por afiliaciones históricas permanecen intactas.

Cada consulta E2 requiere el mundo, la rama y un año cerrado. La rama determina el prefijo visible y los intervalos se evalúan en el cierre inclusivo. La madre conserva su futuro; una hija consulta únicamente su propio prefijo y sus commits.

## Interfaz y alcance excluido

La interfaz permite elegir el asentamiento y una persona, consultar el censo y mostrar cultura, rutas abiertas, movimientos, instituciones, organizaciones, residencias y afiliaciones para el mismo cierre y rama. Los listados de API tienen un máximo de 100 filas por página; las instituciones y organizaciones muestran el conteo de integrantes activos y ofrecen endpoints paginados para consultar sus IDs. El historial de afiliación también se pagina por persona.

E2 no implementa producción, consumo, profesiones económicas, dinero, mercados ni comercio (F08). Tampoco crea enfermedades, conflictos o consecuencias espontáneas (F09). Las profesiones o asignaciones que necesite una regla territorial son datos de estado versionados, no una economía.

Los perfiles E2-B0, E2-B1 y E2-B2 reutilizan las escalas de población, asentamientos y años de E1, pero miden carga territorial y cultural con población estable. Desactivan nacimientos, mortalidad, uniones y adopciones, y generan sus propias rutas y agendas vacías de desplazamientos forzados; sus residentes iniciales no cambian por demografía. E2-B2 significa 10.000 residentes iniciales, 50 asentamientos y 200 cierres de E2, no el workload demográfico B2 de E1. Los catálogos culturales e institucionales son sintéticos, con etiquetas, unidades y descripciones inventadas. E2-CATALOG-CURRENT usa 83 dimensiones, como escala sintética de 50 facetas más 33 opiniones; E2-CATALOG-LARGE duplica ese catálogo a 166 dimensiones. Ambos comparan 1.000 habitantes, 10 asentamientos y 20 años, y registran el tamaño configurado junto con consultas históricas de censo, cultura, instituciones, organizaciones y rutas. No incluyen textos ni catálogos de terceros. Son mediciones de ingeniería tempranas; el objetivo B3 y la aceptación de rendimiento final pertenecen a E5.
