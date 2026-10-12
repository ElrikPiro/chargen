# Alcance de E0 y E1

Este documento concreta el plan de implementación para las dos primeras entregas verticales. La guía de referencia completa es [`../.agents/docs/10-plan-implementacion.md`](../.agents/docs/10-plan-implementacion.md); los archivos heredados que describen el código Python se conservaron en [`../legacy/`](../legacy/). Ninguna cifra de estos escenarios de prueba se interpreta como una regla demográfica universal.

## E0: contratos, servicio local y dominio (F00–F02)

E0 deja una aplicación Rust local que abre una interfaz web servida por el propio ejecutable, acepta escenarios estructurados y rechaza los inválidos antes de iniciar una corrida. El servicio escucha únicamente en loopback y no descarga recursos para mostrar la interfaz.

- F00 concreta el cierre anual, los años consultables, los cortes de rama y la visibilidad temporal. Mantiene escenarios pequeños para parentesco, mercado, migración, cultura, organizaciones y eventos superpuestos como contratos futuros; que una fixture exista no significa que ese proceso ya esté implementado.
- F01 fija el contenedor de servicios, contratos inyectables, configuración local, aislamiento de datos y una página de prueba. GitHub Actions comprueba Linux y Windows; CI extrae y prueba los paquetes independientes. El workflow de release publica artefactos por plataforma con manifiesto y SHA-256.
- F02 define identidades, vínculos familiares, uniones, hogares, fechas, valores y contratos de escenario versionados con validación localizada.

## E1: evolución demográfica persistida (F03–F05)

E1 añade el motor anual, el historial durable y un censo que cambia con nacimientos y muertes. Un mismo mundo se puede pausar entre cierres anuales, reanudar después de reiniciar la aplicación y consultar por año y rama. Una rama conserva su prefijo común y no ve el futuro de su madre.

Incluye:

- población inicial configurable por edad, hogar y perfiles; todos los vivos cuentan, aunque sean menores, ancianos, no tengan pareja o no sean candidatos a unión;
- vínculos de parentesco biológico y de cuidado, uniones con historial, adopción, viudedad y separación según las políticas habilitadas;
- fallecimientos y nacimientos fijados al cierre del año en que ocurren;
- identidad y rasgos de decisión fijados antes de usarse, aleatoriedad derivada por claves estables y resultados repetibles dentro de la versión del motor;
- escritura transaccional por año, checkpoints, recuperación, reanudación, ramas y consultas genealógicas de solo lectura;
- una UI prototipo para validar/crear un mundo, ejecutar hasta un cierre anual, pausar/reanudar/cancelar y consultar censos históricos.

Las reglas de uniones, reproducción y mortalidad se configuran por escenario. Los perfiles incluidos son ficticios y sirven para recorridos técnicos. Cada valor de demostración lleva unidad y rango; cambiarlo puede cambiar el mundo y no convierte ese valor en una recomendación histórica o biológica.

## Convención temporal y persistencia

El año mostrado representa su cierre. Una corrida hasta `through_year = Y` deja confirmado el cierre de Y, incluido. Si `current_year` aún no existe, el primer paso procesa `start_year`; si existe, procesa el siguiente año. La cancelación se reconoce entre años: el año actual se confirma completo o se descarta completo. El último commit durable es la fuente para reanudar; la aplicación no anuncia progreso antes de confirmar el commit.

Las consultas exigen mundo, rama y año. Una persona aún no nacida no aparece en ese cierre; una persona muerta durante ese año no figura viva al cierre. Una edición efectiva desde el inicio de Y corta la historia compartida en Y−1. Los eventos futuros que ya se realizaron en la rama madre no se copian a la hija.

## Aceptación de E1

- Una población con varias edades evoluciona durante años enteros y su censo cuadra con nacimientos y muertes confirmados.
- La misma semilla, escenario, revisión de reglas y comandos produce los mismos eventos al continuar sin pausa, pausado/reanudado y tras reiniciar.
- Abrir, ordenar o exportar una consulta no modifica eventos, IDs, censo ni resultados futuros.
- La genealogía consultada coincide con los vínculos persistidos, no inventa ancestros y no crea personas.
- Una rama sin cambios reproduce los resultados desde el mismo corte; una rama editada mantiene intacta la madre y no filtra su futuro.
- Un fallo de escritura o una cancelación deja visible el último cierre anual confirmado, nunca una fracción del siguiente.
- Linux y Windows compilan y ejecutan los recorridos de aceptación del mismo commit en CI. El paquete incluye la web local y arranca sin instalar Node, Python o Rust.
- CI genera y extrae los paquetes Linux/Windows antes de probar el ejecutable; el workflow de release adjunta cada paquete, sus manifiestos y el fichero consolidado `SHA256SUMS.txt`.
- B0 y B1 sirven para corrección/coste; B2 explora escala. La meta completa de 1.000 años, 200 asentamientos, 50.000 habitantes vivos de media, dos horas y 16 GB se mide y acepta en E5, no en E1.

## Escenarios de medición y decisiones pendientes

| Perfil | Escala prevista | Uso | Estatus |
| --- | --- | --- | --- |
| B0 | Decenas de personas, pocos asentamientos, 10 años | Trazas pequeñas, reglas y errores | Fixture técnica |
| B1 | 1.000 personas, 10 asentamientos, 100 años | Tiempo por proceso, historial, reanudación y consulta | Fixture técnica |
| B2 | 10.000 personas, 50 asentamientos, 200 años | Curvas de escalado y carga de consultas | Fixture técnica; puede crecer durante medición |
| B3 | 50.000 vivos de media, 200 asentamientos, 1.000 años | Rendimiento integral acordado | F12/E5; no es una condición de E1 |

La propuesta editable B3, su tolerancia y el registro del equipo observado están detallados en [acceptance-contract.md](acceptance-contract.md). La tolerancia todavía requiere aprobación de autoría y el equipo observado no sustituye el PC objetivo de 16 GB. Las tasas numéricas de mortalidad, fecundidad y formación de uniones también requieren calibración; por ahora los escenarios de demo identifican sus valores como ficticios.

La protección de rama debe exigir los checks `Verify (ubuntu-24.04)`, `Verify (windows-2022)` y `Compare Linux and Windows B0 output`. Esa configuración de GitHub queda pendiente de una acción manual del administrador del repositorio; CI no puede activarla desde estos archivos.

El estado de la validación formal de Windows, el destino MSVC y el contrato de runtime del paquete se detalla en [platform-validation.md](platform-validation.md).

## Fuera de E0/E1

El movimiento entre asentamientos y las rutas se implementan en F06; cultura, instituciones y organizaciones en F07; grupos profesionales, dinero y mercados en F08; enfermedades y conflictos programados en F09; las fichas narrativas completas y la UI de edición en ramas en F10–F11. El B3 integral y la distribución final pertenecen a F12–F13.

Quedan excluidos de la primera versión: mercados individuales por hogar, empresas detalladas, guerra táctica, transmisión por contacto, brotes espontáneos, guerras automáticas, cambios de valores culturales de adultos, servicios en nube, narración dependiente de una API externa y migración automática desde el formato Python. Las organizaciones no ejecutan acciones económicas, políticas ni bélicas.
