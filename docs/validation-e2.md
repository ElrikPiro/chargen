# Validación de E2 (F06–F07)

Fecha: 12 de octubre de 2026. Versión del ejecutable: 0.3.0. Los datos de las demostraciones y mediciones son sintéticos.

E2 implementa rutas dirigidas y versionadas, migración voluntaria y órdenes explícitas de desplazamiento, residencia y profesión temporal, balances territoriales, cultura incremental, nacimientos con fotografía cultural común, afiliaciones institucionales/religiosas y organizaciones con fundadores, membresías y disolución. Las políticas de una rama revisada entran en vigor después de su corte; las consultas del prefijo usan la revisión original.

## Corrección y compatibilidad

La ronda final pasa 62 pruebas Rust (4 aplicación, 11 dominio, 32 motor, 4 consultas, 10 almacén y 1 integración real de E1). También pasan formato Rust, Clippy con `-D warnings`, contratos TypeScript, sintaxis JavaScript/JSON y comprobación de recursos web locales. Se compila el ejecutable de distribución con Rust 1.90.0, Cargo.lock fijado y dependencias disponibles sin red.

La validación independiente `scripts/validate-e2.mjs` usa resultados calculados manualmente: dos nacimientos del mismo año deben recibir valores 40 y 66 desde una fotografía local de 60. El agregado posterior es 346/6, redondeado a 58. Un hogar de cuatro personas se traslada después por una ruta dirigida; los nacimientos siguientes reciben 35 en el destino y 74 en el origen. La unión activa conserva al progenitor genético aunque resida en otro lugar; esa regla de E1 permanece vigente. Se comprueban también cultura ausente del lugar vacío, conservación de rasgos personales, historia de profesión/residencia, afiliaciones, disolución sin pérdida de fundadores, balances, cierre de ruta, divergencia de ramas, restauración y reinicio.

Los estados terminales de las corridas se publican después de liberar el escritor del mundo. La regresión del worker verifica ese orden; iniciar otra rama inmediatamente después de completar una corrida no conserva un lease de la anterior.

La fixture SQL de compatibilidad procede de la base B0 de E1 creada antes de estos cambios. Su prueba abre los diez cierres originales, verifica sus hashes, reanuda y exporta sin reescribir el pasado. Los escenarios con reglas 1 mantienen su ejecución; E2 requiere esquema y reglas 2.

Las pruebas de motor cubren capacidades de ruta y destino compartidas, asimetría, rutas sin capacidad, desplazamiento del hogar completo, movimiento individual, rechazo y cancelación atómicos, revisión futura de profesiones y repetición determinista desde el mismo corte. Las pruebas culturales comparan agregados incrementales con un recálculo completo, elegibilidad por edad, nacimientos en lote, límites de organizaciones e intervalos temporales.

## Mediciones

Medido en Linux x86_64, Intel Core i7-9700K, ocho CPUs lógicas y 31,3 GiB de RAM informados por el sistema. Se usa una ejecución por perfil, sin compilación simultánea. Los tiempos suman motor y confirmación durable de cada año; excluyen inicialización, huella diagnóstica de estado completo y lectura final. La memoria es el máximo RSS muestreado, no un pico continuo garantizado.

| Perfil | Habitantes / lugares / años | Dimensiones / instituciones | Simulación y persistencia | RSS muestreado | Base SQLite |
| --- | --- | --- | --- | --- | --- |
| [E2-B0](validation/e2-b0.json) | 40 / 3 / 10 | 6 / 4 | 0.052 s | 10.2 MiB | 0.2 MiB |
| [E2-B1](validation/e2-b1.json) | 1,000 / 10 / 100 | 20 / 16 | 8.395 s | 67.9 MiB | 27.9 MiB |
| [E2-B2](validation/e2-b2.json) | 10,000 / 50 / 200 | 50 / 50 | 380.267 s | 914.9 MiB | 795.0 MiB |
| [E2-CATALOG-CURRENT](validation/e2-catalog-current.json) | 1,000 / 10 / 20 | 83 / 16 | 4.585 s | 113.5 MiB | 13.5 MiB |
| [E2-CATALOG-LARGE](validation/e2-catalog-large.json) | 1,000 / 10 / 20 | 166 / 32 | 10.666 s | 209.7 MiB | 25.4 MiB |

Los perfiles E2 mantienen población estable, sin nacimientos, mortalidad, formación de uniones ni adopción, y activan migración, agregados, afiliaciones y organizaciones. E2-B2 conserva 100 organizaciones consultables al cierre. Los informes registran fases A0–A11 y tiempos de consultas históricas; el tiempo de apertura y reconstrucción de la vista se incluye en `historical_census_micros`, mientras las consultas culturales e institucionales posteriores reutilizan esa vista.

Los perfiles de catálogos comparan 83 dimensiones —la escala de 50 facetas más 33 opiniones— y 166 dimensiones con etiquetas sintéticas. No usan datos externos ni afirman calibración demográfica. El rendimiento de referencia de 50.000 habitantes durante 1.000 años pertenece a E5 y no se acredita con estas mediciones.

La regresión [B0 con reglas E1](validation/e2-e1-b0.json) conserva exactamente la huella original `8abe550c435eed83`. E2-B0 produce `53ce4b5fcd174c7c`; los workflows comparan ambas huellas entre plataformas.

## Plataformas

El paquete Linux `dist/chargen-0.3.0-linux-x86_64.tar.gz` se extrae y se verifica con `scripts/check-package.mjs` y su SHA-256. La aceptación ejecuta `smoke-api.mjs`, `validate-e1.mjs` y `validate-e2.mjs` contra ese ejecutable, con directorio de trabajo y PATH temporales ajenos al repositorio. También se contrastan las huellas B0/E2-B0 desde el paquete. La interfaz se revisa en navegador: las tablas muestran rutas, balances, cultura, organizaciones y los intervalos de residencia/afiliación, con año y rama explícitos. Los workflows ejecutan pruebas, empaquetado y aceptación de E1/E2 sobre el paquete extraído en Linux y Windows. La ejecución formal de Windows y la comparación entre plataformas quedan pendientes de GitHub Actions para el commit que incorpore estos cambios. No se ha publicado ni enviado el código al remoto.

Rust 1.90.0 y TypeScript 5.9.3 usados para desarrollar permanecen en `.tooling/`, aislados en el repositorio. El ejecutable distribuido no requiere instalarlos.
