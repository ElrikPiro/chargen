from .character import Character
from . import resetPlaceHolder
from os import linesep

def getFirstHeader(personaje : Character) -> str:
    """La cabecera será '# Apellido Nombre'"""
    familia = personaje.getFamilia()
    
    if familia == "PLACEHOLDER":
        familia = input("PLACEHOLDER en familia, introduce nombre de la familia:" )
        id = personaje.getFamiliaId()
        resetPlaceHolder("config/familias.json", id, familia)
    
    nombre = personaje.getNombre()
    if nombre == "PLACEHOLDER":
        nombre = input("PLACEHOLDER en nombre, introduce nombre del personaje:" )
        id = personaje.getNombreId()
        resetPlaceHolder("config/nombresPropios.json", id, nombre)
    
    return "# " + familia + " " + nombre

def getEdadList(personaje : Character) -> str:
    """TODO"""
    return ""

def getSecondHeader(personaje : Character) -> str:
    """Cabecera de datos básicos"""
    retval = ""
    retval += "## Datos Básicos" + linesep
    retval += "- Nombre: " + personaje.getNombre() + linesep
    retval += "- Sexo: " + personaje.getSexo() + linesep
    retval += "- Edad:" + linesep
    retval += "\t- Año Nacimiento: " + personaje.getNacimiento() + linesep
    retval += "\t- Año Muerte: " + personaje.getMuerte() + linesep

    choice = input("¿Añadir edad en eventos concretos? [S|N] ")
    
    if choice == "S":
        retval += getEdadList(personaje) + linesep

    """TO BE CONTINUED"""

    return retval


def markdownGenerator(jsonFile : str) -> str:
    endl = linesep
    personaje = Character({}, jsonFile)

    retval = getFirstHeader(personaje) + endl
    retval = getSecondHeader(personaje) + endl

    return retval