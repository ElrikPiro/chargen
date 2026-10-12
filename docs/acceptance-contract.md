# Contrato de aceptación F00

Este documento fija ejemplos numéricos para validar la semántica temporal y deja especificaciones ejecutables para procesos posteriores. Las cifras demográficas son datos de prueba editables, ficticios y no calibrados. Una especificación marcada como futura no significa que E1 implemente ese proceso.

## Cierre anual y consulta histórica

El estado consultable de un año es el cierre inclusivo de ese año. El primer paso de un escenario con `start_year = 0` confirma el año 0; avanzar `through_year = 3` confirma los cierres 0, 1, 2 y 3. Un nacimiento ocurrido en Y aparece en el censo del cierre Y. Una muerte ocurrida en Y no cuenta como viva al cierre Y. Un evento iniciado en Y no altera una consulta de cierre anterior. La consulta queda fijada a `world_id`, `branch_id` y `year`.

La siguiente fixture pequeña se usa como contrato para recorridos de entre 3 y 10 cierres. Las órdenes `nacimiento`, `muerte` y `traslado` son cambios deterministas de la fixture de aceptación; no requieren reglas demográficas adicionales.

| Cierre | Cambios aplicados en el año | Vivos en A | Vivos en B | Vivos totales | Visibilidad esperada |
| --- | --- | ---: | ---: | ---: | --- |
| 0 | Base: p1 adulta y p2 menor residen en A | 2 | 0 | 2 | p1 y p2 existen; p3 no existe |
| 1 | Nace p3 en A como hija de p1; p2 muere | 2 | 0 | 2 | p3 existe; p2 está muerta al cierre |
| 2 | p1 y p3 se trasladan de A a B | 0 | 2 | 2 | la residencia histórica de p1 en A sigue visible en 1 |
| 3 | Sin cambios | 0 | 2 | 2 | el cierre vuelve a reportar p1 y p3 en B |
| 4 | Nace p4 en B como hija de p3 | 0 | 3 | 3 | p4 existe con nacimiento en 4 |
| 5 | p1 muere en B | 0 | 2 | 2 | p1 está muerta al cierre; p3 y p4 viven |
| 6 | Sin cambios | 0 | 2 | 2 | ninguna entidad futura aparece |
| 7 | p5 nace en B como hija de p3 | 0 | 3 | 3 | p5 existe; la genealogía contiene solo vínculos declarados |
| 8 | p4 muere en B | 0 | 2 | 2 | p4 consta en historia pero no en vivos |
| 9 | p6 nace en B como hija de p5 | 0 | 3 | 3 | p6 aparece por primera vez en 9 |

Cada prefijo de la tabla constituye una ejecución válida: tres cierres consulta 0–2; cuatro, 0–3; cinco, 0–4; y diez, 0–9. En cada longitud deben cumplirse el balance `total = A + B`, la ausencia de personas aún no nacidas y la consistencia de un cierre al volver a consultar. Las fixture orders son una sonda de contrato temporal F00 y no describen el proceso de migración de F06 ni la fecundidad de E1.

## Ramas y cortes temporales

- Una rama creada con corte Y conserva los cierres hasta Y, ambos inclusive. Si la revisión editada comienza a regir desde Y, el punto compartido es Y−1; la API expresa ese corte como el último cierre compartido.
- Antes de confirmar el primer cierre no existe un año `start_year − 1` representable para todo `i32`; `cut_year: null` significa crear una hija desde el estado inicial materializado.
- Consultar o editar una hija no modifica la madre. La hija no copia eventos ni estado futuro de la madre después del corte.
- Reanudar después de reiniciar usa el último cierre durable de esa rama. Un año pendiente nunca es visible a medio confirmar.
- Una rama sin cambios, con igual semilla y versiones de reglas, reproduce el mismo resultado desde el corte.

## Especificaciones futuras de procesos

Los ejemplos siguientes fijan expectativas de F00 para el diseño de fases posteriores. Deben convertirse en pruebas cuando sus procesos entren en alcance; E1 solo garantiza la historia demográfica, el cierre, el almacenamiento y las consultas que ya están descritos en [e1-scope.md](e1-scope.md).

### Familia y genealogía

Una persona recién nacida con dos progenitores biológicos declarados debe devolver exactamente esos dos ancestros a distancia 1. No se crean abuelos si no existen vínculos para ellos. Una persona sin enlaces parentales conocidos devuelve cero ancestros; consultar genealogía no asigna IDs ni cambia el estado. Los enlaces de cuidado o adopción se conservan como tipos distintos de los biológicos.

### Mercado local (F08, futuro)

Un vendedor comienza con 8 unidades de grano y un comprador solicita 6 unidades con 10 monedas disponibles. El precio local es 2 monedas por unidad. La transacción compra exactamente 5: el comprador recibe 5 unidades, conserva 0 monedas y mantiene una unidad de demanda insatisfecha; el vendedor conserva 3 unidades y recibe 10 monedas. La suma del bien sigue siendo 8 y la suma de monedas entre comprador y vendedor no cambia. Ninguna lectura de mercado produce recursos.

### Migración (F06, futuro)

Dos personas viven en A y ninguna en B. Una ruta dirigida A→B permite que una unidad familiar de dos personas migre en el año fijado. Al cierre, A=0, B=2 y el total global=2. Si no existe ruta B→A no hay migración inversa. El traslado conserva IDs y rasgos personales. Esta fixture futura no convierte la tabla temporal anterior en una implementación de rutas.

### Cultura (F07, futuro)

Cuatro habitantes adultos tienen un valor cultural de 20, 40, 80 y 100; el agregado local con pesos iguales es 60. Para un nacimiento, la media parental de 20 y 40 es 30; el componente parental tiene peso 2, el valor local peso 1 y la variación es 0. El valor infantil esperado es `(2×30 + 60) / 3 = 40`. Los adultos siguen conservando sus valores originales después de cambiar el agregado del asentamiento.

### Organización (F07, futuro)

Configurar el umbral de cooperación en 60, `minimum_members = 2` y `maximum_members = 2`. Las personas p01 y p02 tienen cooperación 70 y 80; p03 tiene 59. Aunque los candidatos llegan como `[p02, p01]`, la organización se crea con los IDs estables ordenados `[p01, p02]` y excluye p03. Si p02 sale y queda un miembro, se disuelve al cierre y conserva su evento histórico. La primera versión de este proceso no ejecuta acciones económicas, políticas ni bélicas.

### Eventos superpuestos (F09, futuro)

La persona p01 está viva antes del cierre 2. Los eventos `evt-a` y `evt-b` se programan para 2 con la misma prioridad; `evt-a` causa muerte inmediata y `evt-b` causa un segundo efecto letal. El desempate por `stable_event_id` coloca `evt-a` primero. p01 muere una sola vez con causa `evt-a`; el efecto de `evt-b` queda registrado como omitido por precondición. No se crea un evento espontáneo. Repetir la corrida conserva orden, resultado y causa.

## B3 propuesto para calibración futura

B3 es una propuesta editable de ensayo de escala, no una condición de E1 ni una calibración aprobada. Dimensiones iniciales: 50.000 habitantes vivos de media, 200 asentamientos y 1.000 cierres anuales; tolerancia propuesta de media: ±5 % (47.500–52.500). La población no se trunca artificialmente para entrar en el intervalo; la media se calcula sobre todos los cierres y semillas acordados.

Catálogos de referencia iniciales, cada entrada sujeta a edición por autoría:

- Bienes: grano, legumbres, carne, pescado, madera, piedra, mineral, metal, tela, medicina, herramientas y combustible.
- Grupos de profesión: agricultura, ganadería, pesca, caza, silvicultura, minería, metalurgia, textil, construcción, transporte, comercio y curación.
- Dimensiones culturales: cooperación, confianza, apertura, tradición, autonomía, riesgo, reciprocidad y religiosidad.
- Intensidad de eventos programados: 2.000 eventos durante 1.000 años (2 por año en promedio); 70 % local, 25 % regional con alcance máximo de cinco asentamientos y 5 % global; 50 % de efecto exacto y 50 % probabilístico; duración de 1–3 años; 5 % de inicios mientras otro evento todavía está activo. Todas estas proporciones e intensidades son hipótesis para revisión y edición, no reglas fijadas del producto.

Protocolo de medición propuesto: compilación Release; guardar todos los cierres y eventos; ejecutar tres semillas como mínimo, con tres repeticiones por semilla en el equipo de referencia; registrar CPU/modelo/núcleos, RAM, sistema, arquitectura, medio y sistema de archivos, versión del motor y de reglas, semilla, tiempo total y por cierre, tiempos por fase, eventos, media y pico de población, RSS pico, tamaño durable y latencia de consultas. Informar mediana y dispersión, además de cada corrida. La ventana objetivo original es una a dos horas en un PC promedio con 16 GB de RAM; su aceptación pertenece a F12/E5, no a E1.

El equipo observado durante esta implementación es Linux con CPU Intel Core i7-9700K de 8 núcleos, 31 GiB de RAM y una partición `/dev/sdb6` de 373 GB. Se registra como contexto de desarrollo, no como máquina de referencia aceptada: su memoria no coincide con el objetivo de 16 GB y aún falta acordar el almacenamiento de referencia con autoría.

## Parámetros y decisiones todavía editables

En `b0-family.json`, los parámetros de demostración tienen estos valores y unidades:

| Parámetro | Valor de demo | Unidad y alcance |
| --- | ---: | --- |
| Mortalidad basal | 500 | ppm por persona viva y decisión anual; sustituible por bandas de edad |
| Formación de unión | 10.000 | ppm por intento anual de persona elegible, edades 18–60; un intento por persona |
| Separación de unión | 0 | ppm por decisión anual |
| Probabilidad de nacimiento | 100.000 | ppm por oportunidad anual ordinal de una persona gestante elegible |
| Edad gestacional | 18–45 | años al cierre consultado |
| Edad mínima de aportante | 18 | años al cierre consultado |
| Intervalo mínimo entre nacimientos gestacionales | 1 | año simulado |
| Nacimientos por persona gestante y año | 1 | máximo entero por año |
| Mortalidad infantil | 0 | ppm por decisión de mortalidad infantil |
| Adopción | desactivada | no se generan candidatos ni adopciones; la probabilidad configurada es 0 ppm |
| Cooperación | 0–100 | escala entera del rasgo; variación máxima del rasgo heredado: 5 puntos |

Las probabilidades llevan unidades explícitas en ppm (0–1.000.000); edades son años enteros; límites y pesos son enteros de escenario. Los valores de demo solo prueban el cableado y su intervalo, no son tasas históricas ni recomendaciones. Antes de presentar perfiles como históricos o recomendados, la autoría debe fijar población de referencia, fuentes, distribución de edades, causas, tasas, resolución temporal y márgenes de incertidumbre. Los resultados de la fixture determinista de arriba prueban semántica, no plausibilidad demográfica.
