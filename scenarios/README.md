# Escenarios de E0/E1

`demos/b0-family.json` es un mundo ficticio pequeño con tres personas, una pareja y dos vínculos biológicos conocidos entre progenitores y su hija.

`benchmarks/profiles.json` fija las entradas de B0, B1 y B2. El comando `chargen-app bench --profile B0|B1|B2` expande esos perfiles de forma determinista en escenarios válidos; así se pueden producir 1.000 o 10.000 habitantes sin mantener miles de líneas JSON duplicadas. Cada informe registra perfil, semilla, versión del motor, CPU, memoria declarada por el host, duración por año, hechos/eventos y bytes escritos.

Los parámetros de los escenarios son **ficticios para pruebas**, llevan valores enteros y las probabilidades se expresan en partes por millón (`ppm`, 0–1.000.000). No representan una calibración demográfica. [El contrato F00](../docs/acceptance-contract.md) detalla unidades, fixtures temporales, la propuesta editable de B3 y el equipo observado; aún se debe aprobar la tolerancia propuesta y acordar el PC de referencia de 16 GB.
