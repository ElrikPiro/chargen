# elrikpiro's chargen version 0.2.0

chargen es una librería en python para generación procedural de personajes para rol o escritura creativa.

## Instalación

Por el momento, elrikpiro's chargen carece de módulo importable a través del sistema pip. En su lugar se sugiere clonar o descargar este repositorio.

```bash
git clone https://github.com/ElrikPiro/chargen.git
```

Para poder utilizar los scripts que contiene el módulo es necesario instalar los requisitos.

```bash
make init
```

## Uso

Creación de un personaje aleatorio

```python
from chargen import Character

Character({}).save()
```

El resultado generará un fichero json, todavía vacío de datos al respecto del personaje generado. A partir de las distintas funciones de la clase `Character` se podrá ir generando los datos requeridos del personaje.

Se puede hacer uso del render por defecto para generar toda la información de un personaje y añadirla a un fichero Markdown:

```python
import os
from chargen import render
from chargen import Character

(...)
    david = Character({}, "personaje_1.json") #Carga un personaje ya existente
    print(render.markdownGenerator(david.file)) #Pregunta al usuario por los datos que faltan e imprime la ficha del personaje en formato markdown

```

## Contribuir
Para sugerir cambios, se recomienda abrir un issue. También estoy abierto a considerar los pull request que abrais.

## Licencias
Parte del software hace uso de algunos strings de terceros (Dwarf Fortress) y algunos de los ejemplos podrían usar nombres de personaje o localizaciones de leyenda de los 5 anillos (L5R). El resto del software es mío y se distribuye bajo la licencia CoffeeWare [LICENSE](LICENSE)
