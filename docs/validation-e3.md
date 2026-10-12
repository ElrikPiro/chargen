# Validación de E3 (F08–F09)

Fecha: 12 de octubre de 2026. Versión: 0.4.0. Implementación delegada a agentes gpt-6-luna con razonamiento max; planificación, revisión y aceptación centralizadas. Los escenarios son sintéticos.

E3 incorpora economía por profesión y asentamiento, producción con insumos y trabajo vivo, necesidades físicas, pedidos solventes, comercio dirigido con capacidad de carga y coste, consumo y pérdidas. Conserva libros anuales de existencias y dinero, transferencias por cambios de grupo y reacciones para el siguiente año. Las carencias modifican mortalidad, nacimientos y migración únicamente mediante parámetros explícitos.

La agenda configura prioridad, filtros, selección exacta o probabilística, duración y efectos: muertes, incapacidad laboral, reducción productiva, destrucción de existencias, cierres de rutas y desplazamientos. Las consecuencias quedan registradas; las restricciones vencen y los daños realizados permanecen. No hay propagación ni shocks autónomos. Las reglas E3 usan esquema/reglas 3 y las consultas exigen mundo, rama y año confirmado.

## Aceptación independiente

Pasan 89 pruebas Rust: 5 de aplicación, 18 de dominio, 47 de motor, 6 de consultas, 11 de almacén y las dos integraciones reales de E1/E2. También pasan Clippy con `-D warnings`, formato Rust, contratos TypeScript 5.9.3 y las comprobaciones JavaScript de recursos locales. La compilación usa Rust 1.90.0 y dependencias fijadas con `--locked --offline`.

`scripts/validate-e3.mjs` contrasta hechos calculados a mano, sin obtener sus expectativas del motor. Un agricultor produce cuatro raciones, consume una y vende una a precio dos en el año cero: conserva dos raciones y dos unidades monetarias. El exceso local baja el precio siguiente a uno; la demanda solvente sin oferta en otro lugar sube su precio a tres. En el año uno se vende una ración local y otra por una ruta de carga limitada a una unidad: el agricultor conserva tres raciones y cuatro unidades monetarias, los compradores quedan con siete y ocho, y la tesorería recibe una por transporte.

En el año dos se destruyen las tres raciones almacenadas, se incapacita temporalmente al trabajador, se cierra la ruta y se registra una única muerte aunque un segundo efecto vuelva a seleccionar la población restante. El año tres vuelve a producir cuatro raciones y la ruta reabre. Una rama desde el corte uno que elimina esos eventos conserva tres habitantes y cuatro raciones en el año dos; consultar el prefijo y avanzar la hija no cambia la madre.

El caso sin presupuesto distingue necesidad física sin cubrir de demanda solvente cero y no sube el precio por insolvencia. La selección exacta imposible conserva el último cierre confirmado y permite continuar en una rama corregida; el recorte explícito registra cantidad realizada y advertencia. Una rama con receta futura de seis raciones produce seis y conserva seis en el año dos. Cambiar las dotaciones iniciales, unidades de stock o un evento ya iniciado se rechaza. A0 acota precios y capacidades heredadas a los límites revisados antes de abrir los libros. Se verifican páginas de eventos, historia personal sin futuro, exportación, restauración y reinicio. El ejecutable se inicia con directorio y PATH temporales, sin herramientas de desarrollo en el PATH.

Las pruebas Rust cubren reposición de insumos en A5 para su consumo en el siguiente A4, reservas de mercancía y carga, excedentes perecederos, cambios productivos y profesionales, transferencias proporcionales y liquidación a tesorería, desbordamientos, prioridad y solapamiento de eventos, desplazamientos familiares, capacidades compartidas, cierre de rutas y vencimiento de restricciones. Una regresión reproduce la lectura paginada de resúmenes económicos con carencias y movimientos realizados.

## Compatibilidad

Las fixtures SQL reales de E1 y E2 se abren, consultan y reanudan, comprobando hashes originales de estados, eventos y commits. La fixture E2 también se reproduce desde el escenario original, cierre por cierre. Los nuevos campos vacíos se omiten y las descripciones de fases anteriores conservan sus bytes. Legacy conserva las 60 entradas SHA-256 de su inventario original.

Las huellas de los perfiles anteriores permanecen exactamente en `8abe550c435eed83` para [B0 E1](validation/e3-e1-b0.json) y `53ce4b5fcd174c7c` para [E2-B0](validation/e3-e2-b0.json). E3-B0 produce `9b8bb7d23681a2da`; se repite desde el ejecutable empaquetado para contrastar determinismo.

## Mediciones

Una ejecución por perfil, con población estable, sin compilación ni otra simulación simultánea. Linux x86_64, Intel Core i7-9700K, ocho CPUs lógicas y 31,3 GiB de RAM. Los tiempos suman motor y confirmación anual durable; excluyen inicialización, huella diagnóstica y consultas posteriores. RSS es el máximo muestreado, no una garantía de pico continuo. Las recetas, asignaciones profesionales, presupuestos y agenda están declarados antes de generar el estado inicial.

| Perfil | Habitantes / lugares / años | Bienes / profesiones / eventos configurados | Simulación y persistencia | RSS muestreado | SQLite | Grupos | Intercambios acumulados |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [E3-B0](validation/e3-b0.json) | 40 / 3 / 10 | 3 / 4 / 2 | 0,091 s | 13,9 MiB | 0,8 MiB | 15 | 131 |
| [E3-B1](validation/e3-b1.json) | 1.000 / 10 / 100 | 4 / 8 / 5 | 10,620 s | 82,2 MiB | 58,2 MiB | 90 | 5.068 |
| [E3-B2](validation/e3-b2.json) | 3.000 / 50 / 40 | 6 / 12 / 8 | 46,555 s | 368,7 MiB | 172,5 MiB | 650 | 9.015 |

Los informes registran 2, 5 y 8 efectos programados realizados, respectivamente. E3-B2 consulta 650 grupos y 6.671 eventos históricos, recorriendo todas las páginas; esas consultas tardan 2,4 ms y 361,2 ms después de abrir la vista. Las métricas separan la apertura/reconstrucción histórica de las proyecciones posteriores. E3-B2 tiene una carga distinta de B2 E1/E2 y no representa una prueba de 10.000 habitantes durante 200 años.

## Plataformas y límites

El código incluye aceptación y comparación determinista E1/E2/E3 en la matriz Linux/Windows de GitHub Actions. Windows queda pendiente de ejecución formal en ese runner; no se acredita desde esta máquina Linux. No se han hecho commit, push ni publicación. Rust y TypeScript de desarrollo siguen aislados en `.tooling/`; el ejecutable funciona sin esas instalaciones.

El paquete Linux `dist/chargen-0.4.0-linux-x86_64.tar.gz` incluye ejecutable, documentación y manifiesto con huella SHA-256 del árbol fuente y `source_dirty: true`, porque los cambios no están confirmados en Git. Se extrae, verifica manifiesto/checksum y ejecuta humo y aceptación E1/E2/E3 sobre el binario extraído, con PATH temporal sin Python, Rust ni Node. Las huellas B0/E2-B0/E3-B0 se contrastan también desde ese binario. La interfaz fue consultada en navegador sobre el año nueve de E3-B0: muestra grupos, balances, necesidades/demanda, transacciones y efectos realizados; el servidor de comprobación se cierra después.

El editor y los flujos completos F10/F11 pertenecen a E4. El objetivo de 50.000 habitantes y 1.000 años pertenece a E5; los perfiles E3 de esta entrega miden una carga sintética explícita y no acreditan ese objetivo.
