# Documentación del proyecto chargen

**chargen** es una librería Python para la **generación procedural de personajes** orientada a juegos de rol y escritura creativa. Permite crear personajes con atributos físicos, personalidad, genoma, historia familiar y ubicación geográfica generados algorítmicamente. También incluye un sistema de emparejamiento ("casamentera") que simula el proceso de búsqueda de pareja en una población y genera árboles genealógicos a lo largo del tiempo.

---

## Tabla de contenidos

1. [Estructura del proyecto](#1-estructura-del-proyecto)
2. [Módulo `core.py`](#2-módulo-corepy)
3. [Módulo `character.py`](#3-módulo-characterpy)
4. [Módulo `casamentera.py`](#4-módulo-casamenterapy)
5. [Módulo `grafolocalizaciones.py`](#5-módulo-grafolocalizacionespy)
6. [Módulo `render.py`](#6-módulo-renderpy)
7. [Ficheros de configuración JSON](#7-ficheros-de-configuración-json)
8. [Suite de tests](#8-suite-de-tests)
9. [Herramientas auxiliares](#9-herramientas-auxiliares)
10. [Flujo de uso típico](#10-flujo-de-uso-típico)

---

## 1. Estructura del proyecto

```
chargen/
├── chargen/                  # Paquete principal de la librería
│   ├── __init__.py           # Punto de entrada del paquete; reexporta las clases y funciones públicas
│   ├── core.py               # Utilidades de E/S JSON y tipos base
│   ├── character.py          # Clase Character y lógica de generación de personaje
│   ├── casamentera.py        # Sistema de emparejamiento poblacional
│   ├── grafolocalizaciones.py# Grafo de localizaciones con Dijkstra
│   └── render.py             # Generación de fichas en formato Markdown
├── json/                     # Datos persistidos de los personajes creados
│   ├── jason.json            # Ejemplo mínimo de estructura de un personaje
│   └── config/               # Bases de datos de configuración del mundo
│       ├── familias.json         # Nombres de familias/apellidos
│       ├── genoma.json           # Definición de alelos y dominancias genéticas
│       ├── localizaciones.json   # Nodos geográficos y distancias entre ellos
│       ├── necesidades.json      # Necesidades psicológicas y sus factores
│       ├── nombresPropios.json   # Diccionario id→nombre propio
│       ├── obras.json            # Año ficticio de cada "obra" de referencia (para calcular edades)
│       ├── personalidad.json     # Lista de facetas y opiniones de personalidad
│       └── rokugani.json         # Variante de genoma (temática L5R)
├── tests/                    # Suite de tests unitarios
│   ├── context.py            # Ajuste del sys.path para importar chargen desde los tests
│   ├── testUtils.py          # Utilidades compartidas por los tests
│   ├── test_example.py       # Test de humo básico
│   ├── test_character.py     # Tests de la clase Character y el genoma
│   ├── test_casamentera.py   # Tests del sistema de emparejamiento
│   └── test_render.py        # Tests del generador Markdown
├── necesidades.csv           # Fuente original (CSV) de la tabla de necesidades
├── necesidadesCsvToJson.py   # Script de conversión CSV → JSON para necesidades
├── notebook.ipynb            # Cuaderno Jupyter de ejemplo/demostración
├── Makefile                  # Automatización: `make init` y `make test`
├── requirements.txt          # Dependencias Python del proyecto
├── setup.py                  # Configuración de empaquetado setuptools
├── README.md                 # Descripción e instrucciones básicas de uso
└── MANIFEST.in               # Ficheros extra incluidos en la distribución
```

---

## 2. Módulo `core.py`

**Ruta:** `chargen/core.py`

Este módulo proporciona las **utilidades de entrada/salida** sobre ficheros JSON y define la enumeración de tipos de relación familiar. Es el núcleo de persistencia sobre el que se apoya toda la librería.

### 2.1 `getJsonFromUrl(url: str) -> dict`

Realiza una petición HTTP GET a la URL indicada y devuelve el cuerpo de la respuesta deserializado como diccionario Python. Usa la librería `requests`.

**Uso esperado:** no se usa directamente en el flujo principal; está disponible para cargar bases de datos de configuración alojadas remotamente.

---

### 2.2 `loadJson(jsonRef: str) -> dict`

Abre el fichero `json/<jsonRef>` (ruta relativa al directorio de trabajo), lo deserializa con `json.load` y devuelve el resultado. Todos los accesos a configuración y datos de personajes pasan por esta función.

**Parámetros:**
- `jsonRef` — ruta relativa dentro de la carpeta `json/`, por ejemplo `"config/genoma.json"` o `"personaje_1.json"`.

---

### 2.3 `writeJson(jsonRef: str, jsonData)`

Serializa `jsonData` a JSON con sangría de 4 espacios y lo escribe en `json/<jsonRef>`. Es el único punto de escritura a disco de la librería.

---

### 2.4 `generateNewNameId() -> int`

Lee el diccionario `config/nombresPropios.json`, calcula `len + 1` como nuevo identificador numérico, inserta una entrada `PLACEHOLDER` con ese id y persiste el fichero. Devuelve el id recién creado.

**Propósito:** reservar un id único para un nombre propio antes de que el usuario lo haya asignado; los PLACEHOLDERs se rellenan más tarde con `resetPlaceHolder`.

---

### 2.5 `generateNewFamilyId() -> str`

Análogo a `generateNewNameId` pero para `config/familias.json`. El id generado tiene el formato `"PLACEHOLDER_N"` (string), donde N es `len + 1`.

---

### 2.6 `generateNewLugar() -> str`

Análogo a los anteriores pero para `config/localizaciones.json`. El id generado tiene el formato `"PLACEHOLDER_N"`. La entrada insertada es `{"nombre": "PLACEHOLDER"}`.

---

### 2.7 `resetPlaceHolder(config, key, value, isLugar=False, isFamilia=False)`

Sustituye la entrada `PLACEHOLDER` con la clave `key` por el valor real `value` en el fichero de configuración indicado por `config`.

Hay tres modos de operación:
- **Lugar (`isLugar=True`):** además de actualizar la clave `key`, si el nombre del lugar aún no existe en el diccionario, lo crea con campos por defecto (`tipo`, `descripcion`, `enlaces`). Esto garantiza que cualquier lugar nuevo sea navegable en el grafo.
- **Familia (`isFamilia=True`):** además de asignar `key → value`, crea una segunda entrada `value → value` (el nombre de la familia apunta a sí mismo), lo que permite resolver referencias cruzadas sin requerir el id numérico.
- **Nombre propio (modo por defecto):** simplemente asigna `key → value`.

---

### 2.8 `class RelationType`

Enumeración de enteros que clasifica el tipo de relación entre el personaje que se está construyendo y el personaje que lo invoca ("caller"):

| Constante | Valor | Significado |
|-----------|-------|-------------|
| `NONE` | 0 | Sin relación; creación independiente |
| `PARENT` | 1 | El caller es progenitor del personaje |
| `SPOUSE` | 2 | El caller es cónyuge del personaje |
| `DESCENDANT` | 3 | El caller es descendiente del personaje |

---

## 3. Módulo `character.py`

**Ruta:** `chargen/character.py`

Contiene la clase `Character`, que es el **objeto central** de la librería. Un personaje se representa como un diccionario JSON con campos opcionales que se generan de forma diferida (lazy) la primera vez que se solicitan.

### 3.1 `hasDeformation(alelo: dict) -> bool`

Función auxiliar de nivel de módulo. Comprueba si dos copias de un alelo (materno y paterno) comparten el mismo `hash`. Si lo hacen, se considera que el individuo tiene una deformación genética en ese locus. Devuelve `False` si alguno de los hashes es `-1` (alelo no inicializado).

---

### 3.2 `class Caller`

Objeto de contexto que se pasa a `Character.__init__` para indicar qué personaje está creando al nuevo y con qué relación.

| Atributo | Tipo | Descripción |
|----------|------|-------------|
| `file` | `str` | Nombre del fichero JSON del personaje que invoca la creación |
| `relation` | `RelationType` | Tipo de relación con el nuevo personaje |

Cuando `relation == RelationType.NONE`, el personaje se crea de forma independiente (sin datos hereditarios).

---

### 3.3 `class Character`

#### Constructor: `__init__(deductions, jsonRef='', caller=Caller())`

Hay dos modos de uso:
1. **Cargar desde fichero (`jsonRef != ''`):** lee `json/<jsonRef>` y, si el `caller` tiene una relación, llama a `updateJsonStruct` para mezclar `deductions` con los datos cargados.
2. **Crear nuevo (`jsonRef == ''`):** llama a `generateJsonStruct` para inicializar la estructura base con `NaN` en los campos no definidos, mezcla `deductions`, genera el nombre del fichero y guarda en disco.

#### `reload() -> dict`
Recarga los datos del personaje desde disco. Útil cuando otro proceso puede haber modificado el fichero.

#### `updateJsonStruct(deductions, caller)`
Si el caller tiene una relación distinta de `NONE`, mezcla `deductions` sobre `self.data` con el operador `|` (unión de diccionarios de Python 3.9+).

#### `generateJsonStruct(deductions, caller)`
Cuando se crea un personaje desde cero, inicializa `self.data` con la estructura mínima:
```json
{
  "nombre": NaN,
  "eventos": { "nacimiento": NaN, "muerte": NaN, "matrimonio": NaN },
  "sexo": NaN,
  "edad": { "": NaN },
  "familia": NaN,
  "lugar_nacimiento": NaN,
  "lugar_residencia": NaN,
  "clase_social": NaN,
  "parientes": {
    "padre": NaN, "madre": NaN,
    "hermanos": NaN, "conyugue": NaN,
    "hijos": { "len": NaN, "lista": [] }
  }
}
```
Luego mezcla `deductions` encima.

#### `generateJsonFileName()`
Calcula el nombre del fichero JSON como `"personaje_<nombreId>.json"` y lo asigna a `self.file`.

#### `save()`
Delega en `writeJson` para persistir `self.data` en `self.file`.

---

#### Métodos de evento vital

##### `rollEdadMuerte(base=40, sign=1) -> int`
Genera una edad de muerte mediante una tirada de dado d20 recursiva:
- Si sale 1: restar otra tirada (pena) → muerte más joven.
- Si sale 20: sumar otra tirada (recompensa) → vida más larga.
- En cualquier otro caso: `base + roll * sign`.

##### `generateMuerte()`
Calcula `nacimiento + rollEdadMuerte()` y lo persiste en `eventos.muerte`.

##### `getMuerte() -> int`
Devuelve la fecha de muerte. Si no está generada, la genera primero.

##### `rollEdadMatrimonio(base=16, sign=1) -> int`
Similar a `rollEdadMuerte` pero con dado d10:
- Si sale 9: sumar otra tirada.
- En cualquier otro caso: `base + roll * sign`.

##### `generateMatrimonio()`
Calcula `nacimiento + rollEdadMatrimonio()`. Si la fecha supera la de muerte, asigna `"Soltero"`.

##### `getMatrimonio()`
Devuelve la fecha de matrimonio (o `"Soltero"`), generándola si es necesario.

##### `generateNacimiento()`
Deduce la fecha de nacimiento usando la primera fuente disponible:
1. Si tiene madre: igual a la fecha de matrimonio de la madre.
2. Si tiene cónyuge: `matrimonio - rollEdadMatrimonio()`.
3. Si tiene hijos: `nacimiento del primogénito - rollEdadMatrimonio()`.
4. Si no hay datos: solicita al usuario la fecha por consola.

##### `getNacimiento() -> int`
Devuelve la fecha de nacimiento, generándola si es necesario.

---

#### Métodos de identidad

##### `getSexo() -> str`
Devuelve `"Hombre"` o `"Mujer"`. Si no está asignado, lo elige aleatoriamente con `random.choice`.

##### `getEdad(obra: str) -> int | str | NaN`
Calcula la edad del personaje durante la "obra" ficticia de referencia cargada desde `config/obras.json`.
- Si aún no había nacido o si la obra no existe: devuelve `NaN`.
- Si ya había muerto: devuelve `"Fallecido"`.
- En caso contrario: devuelve `anyoObra - nacimiento`.

##### `getNombreId() -> int`
Devuelve el id numérico del nombre del personaje. Si no está asignado, llama a `generateNewNameId()` y lo persiste.

##### `getNombre() -> str`
Traduce el id numérico a un nombre propio buscando en `config/nombresPropios.json`.

---

#### Métodos de familia y lugar

##### `hasMother() / hasFather() -> int`
Comprueban si los campos `parientes.madre` / `parientes.padre` existen y no son `NaN`. Devuelven 1 o 0.

##### `hasSpouse() -> int`
Comprueba si `parientes.conyugue` está definido.

##### `hasDescendants() -> int`
Comprueba si `parientes.hijos.lista` tiene al menos un elemento.

##### `getFamiliaId() -> str`
Devuelve el id de la familia. Si no está asignado, llama a `deducirFamilia()`:
- Si tiene padre, hereda el id de familia del padre (cadena hereditaria).
- Si no tiene padre, genera un nuevo id PLACEHOLDER.

##### `getFamilia() -> str`
Resuelve el id a un nombre legible consultando `config/familias.json`. Si el id sigue siendo un PLACEHOLDER sin resolver, devuelve `"Indefinido"`.

##### `getLugarNacimientoId() / getLugarResidenciaId() -> str`
Devuelven el id del lugar. Si no está asignado:
- Nacimiento: se hereda del padre o se genera un PLACEHOLDER nuevo.
- Residencia: para mujeres casadas, hereda el de la residencia del cónyuge; para el resto, coincide con el de nacimiento.

##### `getLugarNacimiento() / getLugarResidencia() -> dict`
Resuelven el id a un objeto `{nombre, tipo, descripcion, enlaces}` leyendo `config/localizaciones.json`. Manejan la indirección de PLACEHOLDER igual que `getFamilia`.

##### `getClaseSocial() -> str`
Devuelve la clase social. Si no está asignada:
- Mujer casada: hereda la del cónyuge.
- Tiene padre: hereda la del padre.
- En caso contrario: PLACEHOLDER.

---

#### Métodos de parentesco

##### `getPadre() -> Character`
Carga el personaje padre. Si no existe referencia, **genera un padre** con datos coherentes (mismo lugar que el hijo, misma familia, fecha de matrimonio = nacimiento del hijo) y lo vincula.

##### `getMadre() -> Character`
Carga la madre como la cónyuge del padre.

##### `setConyugue(c: Character)`
Establece el enlace matrimonial bidireccional entre `self` y `c`: actualiza `eventos.matrimonio`, `lugar_residencia`, `clase_social` e hijos en el objeto del cónyuge, y persiste ambos ficheros.

##### `getConyugue() -> Character`
Carga el cónyuge. Si no existe, **genera un cónyuge** del sexo opuesto con los datos del matrimonio del personaje actual, y lo vincula.

##### `getHijos() -> list[str]`
Devuelve la lista de ficheros de los hijos. Si el número de hijos no está determinado, lo genera con `generateNumeroHijos()` (distribución normal centrada en 2.5, σ=2). Va creando hijos uno a uno hasta alcanzar el número objetivo, respetando la fecha límite de fertilidad de la madre (`min(muerte padre, muerte madre, nacimiento madre + 40)`). Los intervalos entre nacimientos se calculan con `rollOffsetNacimiento` (acumulación de d6 hasta que no salga 5).

##### `getHermanos() -> list[str]`
Devuelve `getMadre().getHijos()` — todos los hijos de la madre son hermanos.

---

#### Métodos de personalidad

##### `getPersonalidad() -> dict`
Genera y/o devuelve el diccionario de personalidad con dos sub-diccionarios:
- `facetas`: 50 rasgos de temperamento con valores en `[-50, 50]`.
- `opiniones`: 33 creencias/valores con valores en `[-50, 50]`.

**Algoritmo de generación:**
- Si los padres no tienen personalidad: distribución Beta(6,6) escalada a `[-50, 50]` (curva de campana centrada en 0).
- Si los padres tienen personalidad: para cada faceta, la media entre padre y madre, con desviación estándar `|padre - madre| / 2` (distribución normal). Truncado a `[-50, 50]`. Las opiniones siempre se generan con distribución Beta independiente.

---

#### Métodos de genoma

##### `hasGenoma() -> bool`
Comprueba si `self.data["genoma"]` es un diccionario no vacío.

##### `getGenProgenitor(database, progenitor, especie, bodypart, alelo, gen, getHashParameter=False) -> str`
Determina qué variante alélica hereda el personaje de un progenitor concreto:
1. Si el alelo ya estaba determinado en `self.data["genoma"]`, lo devuelve directamente.
2. Si el progenitor tiene ese alelo: elige aleatoriamente uno de sus valores.
3. Si el progenitor no tiene ese alelo: elige aleatoriamente de la base de datos.

Con `getHashParameter=True`, devuelve `"variante;hash"` donde `hash` es un entero aleatorio que permite rastrear la herencia y detectar consanguinidad.

##### `getGenoma(fenotipo='') -> dict`
Genera y/o devuelve el genoma completo del personaje:
1. Determina las especies (por defecto `"humano"`; soporta multi-especie si los padres tienen especies distintas).
2. Para cada especie → parte del cuerpo → alelo, determina la variante heredada del padre (gen paterno) y de la madre (gen materno).
3. Guarda los hashes para detectar deformaciones futuras.

##### `getAleloList() -> list[dict]`
Devuelve una lista plana de todos los alelos del genoma, cada uno como `{nombre_alelo: {paterno: ..., materno: ...}}`.

---

## 4. Módulo `casamentera.py`

**Ruta:** `chargen/casamentera.py`

Implementa la simulación de **búsqueda de pareja en una población** a lo largo del tiempo. El nombre hace referencia al rol tradicional de "casamentero/a".

### 4.1 Funciones de módulo

#### `isReachable(localizacionA, localizacionB) -> bool`
Crea un `GrafoLocalizaciones` y comprueba si existe un camino entre las dos localizaciones. Dos personajes del mismo lugar siempre son alcanzables entre sí.

#### `getDistanciaSocial(a: Character, b: Character) -> float`
Calcula el coste del camino más corto (en días de viaje) entre los lugares de nacimiento de dos personajes usando Dijkstra. Si están en el mismo lugar, devuelve 0.

---

### 4.2 `class Casamentera`

#### Atributos

| Atributo | Tipo | Descripción |
|----------|------|-------------|
| `poblacion_` | `list[int]` | Lista de ids de personajes aún en el mercado matrimonial |
| `year_` | `int` | Año actual de simulación |
| `end_` | `int` | Año final de simulación |
| `debug_` | `bool` | Si es `True`, imprime logs detallados |
| `usarLocalizaciones_` | `bool` | Si es `True`, considera la distancia geográfica |

#### `__init__(poblacion, begin, end, debug=False, usarLocalizaciones=False)`
Inicializa la simulación con la lista de personajes disponibles y el rango temporal.

#### `log(msg)`
Imprime `msg` solo si `debug_` es `True`.

---

#### `iterar()`

Método principal del ciclo de simulación. En cada llamada:

1. **Limpia la población** (`cleanPopulation`): elimina personajes muertos, no fértiles o ya casados.
2. **Obtiene la población válida** del año actual.
3. **Calcula la deseabilidad absoluta** de cada candidato.
4. Para cada candidato, en orden descendente de deseabilidad absoluta:
   - Obtiene la población del sexo opuesto alcanzable geográficamente.
   - Filtra a aquellos con deseabilidad absoluta ≥ 50% de la del candidato.
   - Aplica el **algoritmo del secretario** (optimal stopping): evalúa los primeros `n/e` candidatos para establecer un umbral, luego selecciona el primer candidato que supere ese umbral.
   - Si hay candidato: los casa, registra la fecha de matrimonio, genera los hijos y los añade a la población.
   - Si no hay candidato: el personaje se marca como "diva" (estándares demasiado altos) y recibe fecha `NaN` en `matrimonio`.
5. **Selecciona el próximo año** buscando en el rango `[year+1, year+10]` el año con más candidatos válidos.

#### `getDeseabilidadAbsoluta(lista, fenotipo='', min=0) -> dict`

Calcula un **valor numérico de atractivo** para cada personaje de la lista. La fórmula es:

```
deseabilidad = GEN × CLASE × (FACETA + OPINION)
```

Donde:
- **GEN:** media de dominancias de los genes expresados (0 a 1). Cuanto más "dominante" el fenotipo, mayor puntuación.
- **CLASE:** ponderación de la clase social:
  - Noble=3, Alta=2, Media=1, Ciudadano=1, Pobre=0.5, Paria=0.25
- **FACETA:** productorio de 6 facetas relevantes (`IMMODERATION`, `POLITENESS`, `LOVE_PROPENSITY`, `STRESS_VULNERABILITY`, `CONFIDENCE`, `ANGER_PROPENSITY`) normalizadas entre 0.5 y 2.
- **OPINION:** análogo con 4 opiniones relevantes (`ROMANCE`, `MERRIMENT`, `INDEPENDENCE`, `FAMILY`).

Solo se incluyen en el resultado los personajes cuya deseabilidad supere el umbral `min`.

#### `getDeseabilidadRelativa(entrevistador, entrevistado, fenotipo='') -> float`

Calcula cuánto le gusta el `entrevistado` al `entrevistador`. La fórmula es:

```
resultado = consanguinidad × (atraccionTemperamento + atraccionIdeologica + atraccionGenetica)
          - distanciamiento - diferenciaEdad
```

- **atraccionGenetica:** productorio sobre todos los alelos. Si el entrevistado tiene un fenotipo más dominante, multiplica ×2; si es menos dominante, ×0.5; si es igual, ×1. Favorece la complementariedad genética.
- **atraccionIdeologica:** productorio sobre las opiniones. Si tienen el mismo signo (coinciden en valorar positiva o negativamente algo), ×1; si difieren, se penaliza proporcionalmente a la diferencia.
- **atraccionTemperamento:** productorio sobre las facetas, penaliza las diferencias independientemente del signo.
- **consanguinidad:** penaliza relaciones entre parientes (hermanos=0.25, padres/hijos=0.5, tíos/primos=0.75, sin relación=1).
- **distanciamiento:** `distanciaSocial / 10` (solo si `usarLocalizaciones_` está activo).
- **diferenciaEdad:** `(nacimiento_C - nacimiento_D) / 20` (penaliza diferencias de edad).

#### `getPoblacionValida(year, sexoCandidato='', localizacion='') -> tuple[list[Character], list[int]]`

Filtra la población actual y devuelve una tupla `(lista de Characters, lista de ids)` con los personajes que cumplen **todos** los criterios:
- No están muertos en `year`.
- Están en edad fértil (`year` entre `nacimiento + edad_mínima` y `nacimiento + edad_máxima`). Edad mínima: 16 para hombres, 14 para mujeres. Edad máxima: muerte para hombres, menopausia a los 40 para mujeres.
- Si se especifica `sexoCandidato`, son del sexo opuesto.
- No tienen ya cónyuge.
- Si se especifica `localizacion`, son alcanzables geográficamente desde ella.

#### `cleanPopulation()`

Elimina de `self.poblacion_` los personajes que ya no son elegibles:
- Muertos en `self.year_`.
- Fuera del rango de fertilidad.
- Ya casados.

---

## 5. Módulo `grafolocalizaciones.py`

**Ruta:** `chargen/grafolocalizaciones.py`

Implementa el **grafo geográfico** que permite calcular distancias entre localizaciones del mundo ficticio.

### 5.1 `mockup_localizacion() -> dict`

Devuelve un diccionario con la estructura mínima de un nodo de localización (plantilla de referencia):
```json
{
  "nombre": "Centro del mundo",
  "tipo": "Ciudad",
  "descripcion": "La ciudad en el centro del mundo",
  "enlaces": {}
}
```

---

### 5.2 `class LocalizacionesNodo`

Representa un **nodo del grafo** (una localización del mundo).

| Atributo | Tipo | Descripción |
|----------|------|-------------|
| `nombre` | `str` | Nombre de la localización |
| `tipo` | `str` | Tipo (Ciudad, Pueblo, etc.) |
| `descripcion` | `str` | Descripción textual |
| `enlaces` | `dict` | `{nombre_destino: dias_de_viaje}` |

#### `__init__(localizacion: dict)`
Inicializa el nodo a partir de un diccionario, con valores por defecto `""` para `tipo` y `descripcion`, y `{}` para `enlaces`.

#### `toDict() -> dict`
Serializa el nodo a un diccionario con las 4 claves estándar.

#### `addEnlace(nodo: LocalizacionesNodo, dias: float)`
Añade una conexión directa (unidireccional) al nodo destino con el coste en días de viaje.

#### `getEnlaces() -> dict`
Devuelve el diccionario de conexiones del nodo.

---

### 5.3 `class GrafoLocalizaciones`

Gestiona el **grafo completo** de localizaciones.

| Atributo | Tipo | Descripción |
|----------|------|-------------|
| `nodos` | `dict` | `{nombre: dict_nodo}` cargado desde JSON |

#### `__init__(json="config/localizaciones.json")`
Carga el grafo desde el fichero JSON indicado usando `loadJson`.

#### `setNodo(nodo: LocalizacionesNodo)`
Registra o sobreescribe un nodo en el grafo.

#### `getNodo(nombre: str) -> LocalizacionesNodo`
Recupera un nodo por nombre.

#### `getShortestPath(origen: str, destino: str) -> list`

Calcula el **camino más corto** entre dos localizaciones usando el algoritmo de **Dijkstra** (librería `dijkstra`):
1. Construye un objeto `dijkstra.Graph` añadiendo todas las aristas (incluyendo aristas de cada nodo a sí mismo con coste 0).
2. Ejecuta `DijkstraSPF` desde `origen`.
3. Devuelve `[lista_de_nodos_del_camino, distancia_total]`.
4. Si `destino` no es alcanzable desde `origen`, devuelve `[[], -1.0]`.

---

## 6. Módulo `render.py`

**Ruta:** `chargen/render.py`

Genera una **ficha de personaje en formato Markdown** con toda la información del personaje, lista para usar en herramientas de worldbuilding como Obsidian.

### 6.1 Constantes

```python
DEPTH_UP = 1    # Niveles de ancestros a incluir en el árbol genealógico
DEPTH_DOWN = 1  # Niveles de descendientes a incluir
```

---

### 6.2 `generateDeformityText(character: Character) -> str`

Recorre todos los alelos del personaje y, para cada uno con el mismo hash en el par materno/paterno (indicador de consanguinidad o mutación), añade una línea descriptiva. Si no hay deformidades, devuelve `"No tiene signos de deformidad."`.

---

### 6.3 `fixPlaceholders(personaje, methodology='default', prompt='')`

Solicita por consola (`input`) los valores que aún son PLACEHOLDER en los campos:
- **Familia:** si es `"Indefinido"`, pide el nombre e invoca `resetPlaceHolder(..., isFamilia=True)`.
- **Nombre:** si es `"PLACEHOLDER"`, pide el nombre e invoca `resetPlaceHolder` para `nombresPropios.json`.
- **Lugar de nacimiento / residencia:** si el nombre es `"PLACEHOLDER"`, pide el nombre real e invoca `resetPlaceHolder(..., isLugar=True)`.
- **Clase social:** si es `"PLACEHOLDER"`, pide la clase social.

---

### 6.4 `getFullName(personaje, prompt='') -> str`

Llama a `fixPlaceholders` y devuelve `"<Familia> <Nombre>"`.

---

### 6.5 `getEdadList(personaje) -> str`

Devuelve un bloque de texto con la edad del personaje en cada "obra" de referencia listada en `config/obras.json`, formateado como lista Markdown.

---

### 6.6 `fromTupleToMarkdownTable(tuple) -> str`

Convierte una tupla en una fila de tabla Markdown: `| item1 | item2 | ... |`.

---

### 6.7 `generateListaNecesidades(personalidad) -> str`

Calcula y formatea las **10 necesidades psicológicas más importantes** del personaje:
1. Lee `config/necesidades.json`.
2. Para cada necesidad, suma los valores de las facetas/opiniones "positivas" y resta los de las "negativas".
3. Si el resultado es ≥ 0, usa el nombre positivo de la necesidad; si es negativo, el nombre negativo.
4. Ordena por intensidad y devuelve las 10 primeras como tabla Markdown.

---

### 6.8 `getFirstHeader(personaje) -> str`

Genera la **cabecera H1** de la ficha: `# Apellido Nombre`.

---

### 6.9 `getSecondHeader(personaje) -> str`

Genera la sección **"## Datos Básicos"** con:
- Nombre, sexo, año de nacimiento, año de muerte.
- Edad en cada obra de referencia.
- Familia (con enlace wiki `[[Familia X]]`).
- Especies del genoma.
- Clase social.
- Lugar de nacimiento y residencia (con enlaces wiki).

---

### 6.10 `getMermaidFamilyTree(personaje, depthUp, depthDown) -> str`

Genera un **diagrama de flujo Mermaid** del árbol genealógico:
1. Asciende `depthUp` generaciones recopilando ancestros.
2. Desde los ancestros más altos, desciende `depthUp + depthDown` generaciones.
3. Para cada matrimonio, crea un `subgraph` con los cónyuges y aristas hacia los hijos.

El resultado es un bloque de código Mermaid embebido en el Markdown.

---

### 6.11 `getThirdHeader(personaje) -> str`

Genera la sección **"## Trasfondo"** con:
- **Situación familiar:** padre, madre, hermanos, cónyuge, hijos (todos con enlaces wiki `[[Nombre]]`).
- **Línea temporal:** lista cronológica de los eventos relevantes (nacimiento, bodas, muertes de familiares), limitada a los que ocurren antes de la muerte del personaje.
- Árbol genealógico Mermaid.

---

### 6.12 `getFourthHeader(personaje) -> str`

Genera la sección **"## Personalidad"** con:
- **Facetas:** tabla con las 5 más positivas y las 5 más negativas.
- **Opiniones:** tabla con las 5 más positivas y las 5 más negativas.
- **Necesidades:** tabla de las 10 necesidades más importantes.

---

### 6.13 `getExpresion(genoma, sexo, query) -> str`

Determina qué alelo se **expresa fenotípicamente** dado el genoma completo, el sexo y una ruta de consulta `"especie;parte;alelo"`:
- Si los alelos materno y paterno son el mismo, se expresa ese.
- Si son distintos, se expresa el de mayor dominancia.
- Si dominancias iguales: el paterno en hombres, el materno en mujeres (herencia ligada al sexo).
- Si el alelo paterno tiene el flag `"incompleto"`, devuelve `"incompleto"` (dominancia parcial).

---

### 6.14 Funciones de descripción física

#### `getDescCabello_(rizado, matizPelo, satPelo, claridadPelo) -> str`
Combina los tres atributos del cabello (matiz, saturación, claridad) en un código numérico de 3 dígitos y lo traduce a una descripción de color en español (p.ej. `"castaño claro"`, `"rubio platino"`, `"azabache"`, etc.).

#### `getDescOjos_(tamanoOjos, matizOjos, satOjos, claridadOjos, pestanas, cejas, achinaos) -> str`
Análogo al del cabello pero para ojos, añadiendo forma (redondos vs. rasgados), pestañas y cejas.

#### `getMofletesYHoyuelos_(mofletes, hoyuelos) -> str`
Construye la descripción de mofletes y hoyuelos. Devuelve cadena vacía si no son expresivos.

#### `getNariz_(puenteNas, anchoNas, tamanoNas) -> str`
Construye la descripción de la nariz con puente, anchura y tamaño.

#### `generateDescripcion(personaje) -> str`
Función de alto nivel que compone **toda la descripción física** del personaje leyendo su genoma:
- Somatotipo, color de piel, estatura, pecas, vello corporal, abdomen, pecho.
- Cabello (rizado, color), ojos (tamaño, color, forma, pestañas, cejas), forma de la cara, mofletes, hoyuelos, nariz, labios.
- Deformidades genéticas.

---

### 6.15 `getFifthHeader(personaje) -> str`

Genera la sección **"## Aspecto físico"** invocando `generateDescripcion`.

---

### 6.16 `markdownGenerator(jsonFile: str) -> str`

**Función principal del módulo.** Carga el personaje desde `jsonFile` y concatena las cinco secciones:
1. `getFirstHeader` — Nombre completo
2. `getSecondHeader` — Datos básicos
3. `getThirdHeader` — Trasfondo y árbol genealógico
4. `getFourthHeader` — Personalidad
5. `getFifthHeader` — Aspecto físico

---

## 7. Ficheros de configuración JSON

Todos residen en `json/config/`.

### 7.1 `nombresPropios.json`

Diccionario `{id_numérico: nombre_propio}`. Los ids son enteros en formato string. Las entradas de tipo `"PLACEHOLDER"` son slots reservados pendientes de asignación.

Ejemplo:
```json
{ "1": "Yasuki", "2": "PLACEHOLDER" }
```

---

### 7.2 `familias.json`

Diccionario de apellidos. Tiene dos tipos de entradas:
- `"PLACEHOLDER_N": "PLACEHOLDER"` — slot sin asignar.
- `"PLACEHOLDER_N": "Apellido"` — id que apunta al nombre.
- `"Apellido": "Apellido"` — el propio nombre apuntándose a sí mismo (para resolución directa).

---

### 7.3 `localizaciones.json`

Diccionario de nodos geográficos. Cada entrada tiene la forma:
```json
"NombreLugar": {
  "nombre": "NombreLugar",
  "tipo": "Ciudad | Pueblo | Indeterminado | ...",
  "descripcion": "Texto descriptivo",
  "enlaces": {
    "OtroLugar": 0.5
  }
}
```
Los valores de `enlaces` son coste en "días de viaje" (float). Las conexiones son unidireccionales; para bidireccional hay que declarar ambas.

El fichero de ejemplo incluye las localizaciones `Alcries`, `Onda`, `Ayodar`, `Valencia` (red real de localidades) y `Ciudad`, `Pueblo` (localizaciones abstractas para tests).

---

### 7.4 `personalidad.json`

Define las **listas de nombres** de facetas y opiniones. No incluye valores; los valores se generan aleatoriamente al crear cada personaje.

- `facetas`: 50 rasgos de temperamento (p.ej. `PERSEVERANCE`, `BRAVERY`, `GREED`).
- `opiniones`: 33 creencias y valores (p.ej. `ROMANCE`, `FAMILY`, `LOYALTY`).

Los nombres son los mismos que usa Dwarf Fortress internamente.

---

### 7.5 `genoma.json` y `rokugani.json`

Bases de datos de genética. Estructura jerárquica:
```
especie → parte_del_cuerpo → nombre_alelo → variante → { dominancia, incompleto? }
```

Ejemplo simplificado:
```json
{
  "humano": {
    "cabeza": {
      "calvicie": {
        "calvo": { "dominancia": 0 },
        "no calvo": { "dominancia": 1 }
      }
    }
  }
}
```

- `dominancia`: float de 0 a 1; mayor valor → más probabilidad de expresión.
- `incompleto` (opcional): si está presente, la expresión del alelo es parcial ("dominancia incompleta").

`rokugani.json` es una variante del genoma para personajes de la ambientación L5R (Leyenda de los 5 Anillos).

---

### 7.6 `necesidades.json`

Lista de necesidades psicológicas con la estructura:
```json
{
  "necesidades": [
    ["nombre_positivo", "nombre_negativo", ["faceta_positiva1", ...], ["faceta_negativa1", ...]]
  ]
}
```

Cada necesidad tiene un nombre cuando la tendencia es positiva y otro cuando es negativa, más listas de facetas/opiniones que la potencian o inhiben.

---

### 7.7 `obras.json`

Mapa `{nombre_obra: año_ficticio}` que permite calcular la edad del personaje en distintos momentos de la ficción:
```json
{ "Inicio de la campaña": 1150, "La gran guerra": 1200 }
```

---

### 7.8 `jason.json`

Ejemplo mínimo de la estructura JSON de un personaje. Sirve como referencia y plantilla. Contiene todos los campos posibles con sus valores por defecto (`NaN`).

---

## 8. Suite de tests

**Ruta:** `tests/`

Los tests usan el framework estándar `unittest` y se ejecutan con `nosetests tests` (o `python -m unittest discover`).

### 8.1 `context.py`

Ajusta `sys.path` para que los tests puedan importar el paquete `chargen` desde la raíz del repositorio, independientemente del directorio de trabajo.

### 8.2 `testUtils.py`

Utilidades comunes para los tests (importación del módulo con el contexto correcto).

### 8.3 `test_example.py`

Test de humo básico que verifica que el paquete se importa sin errores.

### 8.4 `test_character.py`

| Test | Qué verifica |
|------|-------------|
| `test_getGenProgenitor_getHashParameterTrue_returnsASemiColonSeparatedStr` | Con `getHashParameter=True`, la función devuelve una cadena `"variante;hash"` |
| `test_getGenProgenitor_hasGenoma_getsHash` | El hash devuelto es un entero no nulo |
| `test_getGenProgenitor_hasParents_HashInherithed` | El hash paterno del hijo es uno de los hashes del padre (herencia correcta) |
| `test_hasDeformation_twoSameGenes_True` | Dos alelos con el mismo hash → `hasDeformation` devuelve `True` |
| `test_getAleloList_hasGenoma_returnsListDict` | `getAleloList` devuelve una lista no nula |

### 8.5 `test_casamentera.py`

| Test | Qué verifica |
|------|-------------|
| `test_suite` | Se puede instanciar `Casamentera` |
| `test_iterar_unaDivaYunTirillas_laDivaNoTieneFechaMatrimonio` | Una persona noble frente a una paria nunca debería casarse (la noble queda como "diva" con fecha `NaN`) |
| `test_iterar_edadesMuyDiferentesPeroPosible_terminanCasados` | Dos personas de la misma clase con edad distinta pero en rango deben casarse en al menos 5 de 10 repeticiones |
| `test_getPoblacionValida_localizacionesIrrastreables_soloAparecenCompatibles` | Solo aparecen los candidatos alcanzables geográficamente |
| `test_iterar_localizacionesIrrastreables_personajesNoSeCasan` | Dos personas de localizaciones sin conexión nunca se casan |
| `test_getDeseabilidadRelativa_mismoCandidatoDistintaDistancia_distintaDeseabilidad` | Un candidato cercano tiene mayor deseabilidad que el mismo candidato lejos |
| `test_getDeseabilidadRelativa_mismoCandidatoDistintaEdad_distintaDeseabilidad` | Un candidato de edad similar tiene mayor deseabilidad que el mismo candidato con más diferencia de edad |

### 8.6 `test_render.py`

Tests del generador Markdown (verifica que las funciones de render producen salida válida para un personaje de prueba).

---

## 9. Herramientas auxiliares

### 9.1 `necesidadesCsvToJson.py`

Script de conversión que lee `necesidades.csv` (tabla con columnas para nombre positivo, nombre negativo, facetas positivas y negativas) y genera `json/config/necesidades.json`. Se ejecuta una sola vez cuando se actualiza la tabla de necesidades.

### 9.2 `notebook.ipynb`

Cuaderno Jupyter que sirve como **entorno de demostración y uso interactivo**. Permite crear personajes, invocar `markdownGenerator` y explorar los datos generados de forma iterativa.

### 9.3 `Makefile`

| Objetivo | Comando | Descripción |
|----------|---------|-------------|
| `init` | `pip install -r requirements.txt` | Instala las dependencias |
| `test` | `nosetests tests` | Ejecuta la suite de tests |

### 9.4 `requirements.txt`

Dependencias del proyecto (incluye al menos `requests`, `dijkstra`, `nose` para tests).

### 9.5 `setup.py`

Configuración de `setuptools` para empaquetar la librería. Incluye nombre, versión, descripción, autor y excluye los directorios `tests` y `docs` del paquete distribuible.

### 9.6 `MANIFEST.in`

Especifica ficheros adicionales (distintos del código Python) que deben incluirse en la distribución del paquete.

---

## 10. Flujo de uso típico

A continuación se muestra el flujo completo desde la creación de un personaje hasta la generación de su ficha:

```
1. Crear un personaje vacío
   Character({})
   └─ generateJsonStruct()        → estructura base con NaN
   └─ generateJsonFileName()      → reserva id en nombresPropios.json
   └─ save()                      → json/personaje_N.json

2. Solicitar datos (lazy evaluation)
   personaje.getNombre()          → pide al usuario si es PLACEHOLDER
   personaje.getSexo()            → aleatorio si no definido
   personaje.getNacimiento()      → deduce de contexto familiar
   personaje.getMuerte()          → base 40 + d20 recursivo
   personaje.getGenoma()          → herencia genética de padres (genera padres si no existen)
   personaje.getPersonalidad()    → Beta(6,6) o normal entre padres

3. Generar árbol familiar (bajo demanda)
   personaje.getPadre()           → carga o genera padre
   personaje.getMadre()           → cónyuge del padre
   personaje.getConyugue()        → genera cónyuge del sexo opuesto
   personaje.getHijos()           → genera hijos espaciados en el tiempo

4. Renderizar ficha
   markdownGenerator("personaje_1.json")
   ├─ getFirstHeader()            → "# Apellido Nombre"
   ├─ getSecondHeader()           → datos básicos
   ├─ getThirdHeader()            → trasfondo + línea temporal + árbol Mermaid
   ├─ getFourthHeader()           → personalidad: facetas, opiniones, necesidades
   └─ getFifthHeader()            → descripción física generada del genoma

5. Simulación poblacional (opcional)
   casamentera = Casamentera([1,2,3,...], begin=1000, end=1200)
   while casamentera.year_ < casamentera.end_:
       casamentera.iterar()
   # Resultado: personajes emparejados con hijos en json/personaje_*.json
```

---

*Documentación generada para chargen v0.2.1. Autor original: David Baselga (ElrikPiro).*
