from .character import Character
from . import resetPlaceHolder, loadJson
from os import linesep

def fixPlaceholders(personaje : Character, methodology = "default", prompt = ""):
    print("Arreglando placeholders de: " + personaje.file)

    familia = personaje.getFamilia()
    if familia == "PLACEHOLDER":
        familia = input("("+prompt+") PLACEHOLDER en familia, introduce nombre de la familia:" )
        id = personaje.getFamiliaId()
        resetPlaceHolder("config/familias.json", id, familia)
    
    nombre = personaje.getNombre()
    if nombre == "PLACEHOLDER":
        nombre = input("("+prompt+") PLACEHOLDER en nombre, introduce nombre del personaje:" )
        id = personaje.getNombreId()
        resetPlaceHolder("config/nombresPropios.json", id, nombre)

    lugarNacimiento = personaje.getLugarNacimiento()
    if lugarNacimiento["nombre"] == "PLACEHOLDER":
        lugarNacimiento["nombre"] = input("("+prompt+") PLACEHOLDER en lugar de nacimiento, introduce nombre del lugar:" )
        id = personaje.getLugarNacimientoId()
        resetPlaceHolder("config/localizaciones.json", id, lugarNacimiento)

    lugarResidencia = personaje.getLugarResidencia()
    if lugarResidencia["nombre"] == "PLACEHOLDER":
        lugarResidencia["nombre"] = input("("+prompt+") PLACEHOLDER en lugar de residencia, introduce nombre del lugar:" )
        id = personaje.getLugarResidenciaId()
        resetPlaceHolder("config/localizaciones.json", id, lugarResidencia)

    claseSocial = personaje.getClaseSocial()
    if claseSocial == "PLACEHOLDER":
        personaje.data["clase_social"] = input("("+prompt+") PLACEHOLDER en clase social, introduce nombre de la clase:" )
        personaje.save()


def getFullName(personaje : Character, prompt : str) -> str:
    fixPlaceholders(personaje, "default", prompt)
    return personaje.getFamilia() + " " + personaje.getNombre()

def getEdadList(personaje : Character) -> str:
    retval = ""

    obras : dict = loadJson("config/obras.json")
    for key in list(obras.keys()):
        retval += "\t- " + key + " (" + str(obras[key]) + "): " + str(personaje.getEdad(key)) + linesep

    return retval

def getFirstHeader(personaje : Character) -> str:
    """La cabecera será '# Apellido Nombre'"""
    return "# " + getFullName(personaje, "Personaje principal") + linesep

def getSecondHeader(personaje : Character) -> str:
    """Cabecera de datos básicos"""
    retval = ""
    retval += "## Datos Básicos" + linesep
    retval += "- Nombre: " + personaje.getNombre() + linesep
    retval += "- Sexo: " + personaje.getSexo() + linesep
    
    retval += "- Edad:" + linesep
    retval += "\t- Año Nacimiento: " + str(personaje.getNacimiento()) + linesep
    retval += "\t- Año Muerte: " + str(personaje.getMuerte()) + linesep
    retval += getEdadList(personaje) + linesep

    retval += "- Familia: " + personaje.getFamilia() + linesep
    genoma = personaje.getGenoma()
    especies = list(genoma.keys())
    retval += "\t- Especies: " + str(especies) + linesep
    retval += "\t- Clase social: " + personaje.getClaseSocial() + linesep

    retval += "- Lugar de nacimiento: " + personaje.getLugarNacimiento()["nombre"] + linesep
    retval += "- Lugar de residencia: " + personaje.getLugarResidencia()["nombre"] + linesep

    return retval

def getThirdHeader(personaje : Character) -> str:
    """Trasfondo"""
    retval = ""
    retval += "## Trasfondo" + linesep
    retval += "#### Situacion familiar" + linesep

    padre = personaje.getPadre()
    madre = personaje.getMadre()

    retval += "- Padre: [[" + getFullName(padre, "Padre") + "]]" + linesep
    retval += "- Madre: [[" + getFullName(madre, "Madre") + "]]" + linesep
    retval += linesep + linesep
    
    retval += "- Hermanos:" + linesep
    for hermano in personaje.getHermanos():
        bro = Character({}, hermano)
        pr = "hermano" if bro.getSexo() == "Hombre" else "hermana"
        retval += "\t- [[" + getFullName(bro, pr) + "]]" + linesep
    retval += linesep + linesep

    retval += "- Conyugue:  [[" + getFullName(personaje.getConyugue(), "Pareja") + "]]" + linesep
    retval += linesep + linesep

    retval += "- Hijos:" + linesep
    for hijo in personaje.getHijos():
        bro = Character({}, hijo)
        pr = "hijo" if bro.getSexo() == "Hombre" else "hija"
        retval += "\t- [[" + getFullName(bro, pr) + "]]" + linesep

    return retval

def markdownGenerator(jsonFile : str) -> str:
    endl = linesep
    personaje = Character({}, jsonFile)

    retval = getFirstHeader(personaje) + endl
    retval += getSecondHeader(personaje) + endl
    retval += getThirdHeader(personaje) + endl

    return retval