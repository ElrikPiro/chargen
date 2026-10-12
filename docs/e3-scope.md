# Alcance de E3: economía agregada y eventos programados

E3 incorpora F08 y F09 como reglas versionadas sobre la historia E1/E2. Usa `schema_version: 3` y `rules_version: 3`, declara `economy` y `scheduled_events` (aunque la agenda esté vacía) y conserva las configuraciones de reglas anteriores sin activarlas por omisión. La economía se calcula por grupos de profesión y asentamiento; los eventos de salud y conflicto se programan explícitamente. Las consultas de mercado y de consecuencias son de solo lectura y siempre se fijan a un mundo, una rama y un año cerrado.

Los escenarios y catálogos demostrativos son sintéticos y sirven para comprobar el motor, los balances y las consultas. No son datos históricos, económicos ni demográficos observados. Las decisiones numéricas de una demo se identifican como políticas ficticias en el escenario y en los informes.

## F08: economía por grupos profesionales

El grupo profesional de un asentamiento es la unidad económica. El modelo no crea una cartera monetaria por habitante ni una simulación de empresas. Los habitantes aportan fuerza de trabajo, composición y necesidades agregadas; los grupos conservan dinero, existencias y magnitudes de producción.

El escenario versionado configura bienes y unidades, almacenamiento y pérdidas, profesiones, productos e insumos, necesidades, capacidades productivas, saldos iniciales, reglas monetarias, rutas y parámetros de mercado. Las cantidades, redondeos y reglas de unidades son explícitos y validados antes de crear o ramificar un mundo.

El ciclo anual respeta el orden de reglas: A4 produce según trabajo, insumos y capacidad; A5 prepara ofertas y demanda solvente, realiza intercambios y transportes con reservas explícitas; A6 consume, registra carencias y exceso de oferta, y prepara ajustes de precio y producción para el año siguiente. Las necesidades físicas y las compras que pueden pagarse son magnitudes diferentes. Un bien sin oferta no genera una venta; una necesidad sin presupuesto no se transforma en una oferta solvente.

Cada cierre conserva balances verificables. Para cada bien, las existencias finales deben coincidir con existencias iniciales más producción e importaciones, menos consumo, exportaciones y pérdidas/destrucción. Para el dinero, el saldo final se explica por saldos iniciales, entradas, salidas y cualquier emisión o retirada configurada. Las reservas de oferta, presupuesto, capacidad de ruta y demanda evitan vender o gastar dos veces los mismos recursos. Los grupos sin miembros siguen sujetos a la política declarada de continuidad o liquidación y no producen.

Los cambios de miembros por residencia o profesión aplican una política explícita de continuidad de inventario y dinero. No se crean saldos por cambiar una etiqueta personal. Los precios reaccionan mediante ajustes simples, acotados y configurables ante demanda solvente pendiente o excedente; no se calcula un equilibrio global ni se promete validar una teoría macroeconómica. Carencias, oportunidades y precios solo modifican demografía o migración cuando una política del escenario lo indica.

Las consultas históricas de mercado muestran los grupos, saldos, existencias, cantidades relevantes, precios, transacciones y carencias que correspondan al cierre elegido. Los listados grandes se paginan con límites del servidor. La lectura y exportación no ejecutan procesos ni modifican el resultado de la simulación.

## F09: agenda explícita de eventos

Un evento programado define su alcance temporal y territorial, prioridad, población elegible, duración y efectos. La selección de afectados usa cantidades exactas o probabilidades reproducibles; los resultados realizados quedan registrados con sus entidades, magnitudes y causas. Una consulta histórica no vuelve a muestrear la receta probabilística.

Los efectos admitidos incluyen muertes, daños o pérdidas de bienes, restricciones temporales, cierres de rutas y desplazamientos. Los efectos temporales dejan de aplicarse al terminar su vigencia; los daños permanentes y las bajas ya realizadas permanecen en la historia. Los cierres anuales guardan balances que permiten explicar cada cambio económico relevante.

Los eventos superpuestos respetan prioridad y desempates estables, no el orden de llegada de peticiones HTTP. Las selecciones se hacen sin reemplazo cuando corresponda, una persona no puede morir dos veces y la cantidad exacta solicitada no inventa población elegible. El límite superior de afectados aplica la política configurada de error o recorte documentado. El desplazamiento explícito no se cuenta de nuevo como migración voluntaria del mismo año.

Una enfermedad no contagia a otros asentamientos y un conflicto no crea campañas. Los efectos pueden continuar durante la duración programada, pero no crean automáticamente otro evento. La consulta de eventos presenta efectos realizados hasta el año consultado; no expone recetas futuras como hechos históricos.

## Tiempo, ramas y compatibilidad

Una proyección E3 mantiene el contexto `(world_id, branch_id, year)` y solo incluye cierres confirmados visibles en esa rama. Los prefijos heredados conservan sus reglas. Una revisión de eventos o políticas económicas rige solo desde el año efectivo de la rama; las ediciones no reescriben hechos ya confirmados ni copian el futuro realizado de la rama madre.

E3 usa el par 3/3. Los escenarios E1 y E2 conservan sus versiones y comportamiento, y al cargarse no activan economía ni agenda de eventos por omisión. Los campos nuevos opcionales o vacíos se omiten al serializar reglas anteriores para conservar hashes, replay y exportaciones existentes. La migración del formato de almacenamiento es independiente de la versión de reglas y no autoriza reinterpretar estados antiguos.

Las consultas de mercado y eventos se añaden al contenedor histórico existente, que ya es de solo lectura y está fijado al año. El navegador recibe contratos tipados y recursos locales; la carga de la interfaz no requiere servicios externos. La vista E3 puede mostrar mercados y consecuencias históricas, pero no pretende completar el editor ni el rediseño de la aplicación F10/F11.

## Fuera de alcance

- Carteras, presupuestos o intercambios individuales por hogar o persona; empresas detalladas y optimización macroeconómica global.
- Crédito, rescates, subsidios, tributos, emisión o retirada de dinero implícitos; saldos negativos sin una política explícita.
- Guerras tácticas, ejércitos, autoridad política, consecuencias bélicas de organizaciones, contagio por contacto, epidemias espontáneas o disparadores autónomos de eventos.
- Inventar víctimas o recursos para satisfacer una cantidad configurada; ocultar carencias físicas tras una cifra de demanda solvente.
- Mostrar el futuro realizado de una rama madre en una consulta de su hija o modificar la historia al consultar/exportar.
- Completar las pantallas, editor y flujos de extremo a extremo de F10/F11.

## Verificación prevista

Los casos pequeños deben observar un mercado con excedente, escasez solvente, necesidad sin presupuesto, comercio limitado por una ruta, cambio de grupo y cierre económico. Cada caso comprueba saldos y bienes, pagos/transferencias, reservas y la diferencia entre necesidad física y demanda solvente.

Los casos de eventos comprueban dos eventos superpuestos, selección exacta superior a elegibles, probabilidad cero, límites de edad/lugar, cierres y daños temporales, desplazamiento y expiración. Se compara una rama con evento y otra sin evento desde el mismo corte y se comprueba que la madre conserve su futuro. Ninguna consulta, orden de lectura ni exportación modifica el estado o los resultados.

Los perfiles E3 de rendimiento declaran número de habitantes, asentamientos, cierres, bienes, profesiones, dimensiones culturales, intensidad de eventos y semilla. Sus informes incluyen resultados deterministas de estado e historia, métricas reales de grupos/mercados/eventos, consultas históricas y tamaños de base de datos. B0/B1/B2 de E3 identifican una carga de economía/eventos versionada y no se confunden con los perfiles demográficos E1 ni con la carga territorial/cultural E2. Un runner alojado no acredita por sí solo el objetivo de rendimiento del equipo de referencia.
