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


def markdownGenerator(jsonFile : str) -> str:
    endl = linesep
    personaje = Character({}, jsonFile)

    retval = getFirstHeader(personaje) + endl

    return retval