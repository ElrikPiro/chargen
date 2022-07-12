from .character import Character
from . import resetPlaceHolder, loadJson
from os import linesep

def fixPlaceholders(personaje : Character, methodology = "default", prompt = ""):
    print("Arreglando placeholders de: " + personaje.file)

    familia = personaje.getFamilia()
    if familia == "Indefinido":
        id = personaje.getFamiliaId()
        personaje.data["familia"] = input("("+prompt+") PLACEHOLDER en familia, introduce nombre de la familia:" )
        resetPlaceHolder("config/familias.json", id, personaje.data["familia"], isFamilia=True)
    
    nombre = personaje.getNombre()
    if nombre == "PLACEHOLDER":
        nombre = input("("+prompt+") PLACEHOLDER en nombre, introduce nombre del personaje:" )
        id = personaje.getNombreId()
        resetPlaceHolder("config/nombresPropios.json", str(id), nombre)

    lugarNacimiento = personaje.getLugarNacimiento()
    if lugarNacimiento["nombre"] == "PLACEHOLDER":
        lugarNacimiento["nombre"] = input("("+prompt+") PLACEHOLDER en lugar de nacimiento, introduce nombre del lugar:" )
        id = personaje.getLugarNacimientoId()
        resetPlaceHolder("config/localizaciones.json", id, lugarNacimiento, isLugar=True)

    lugarResidencia = personaje.getLugarResidencia()
    if lugarResidencia["nombre"] == "PLACEHOLDER":
        lugarResidencia["nombre"] = input("("+prompt+") PLACEHOLDER en lugar de residencia, introduce nombre del lugar:" )
        id = personaje.getLugarResidenciaId()
        resetPlaceHolder("config/localizaciones.json", id, lugarResidencia, isLugar=True)

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
        edad = personaje.getEdad(key)
        if edad == edad:
            retval += "\t- " + key + " (" + str(obras[key]) + "): " + str(edad) + linesep

    return retval

def fromTupleToMarkdownTable(tuple : tuple) -> str:
    retval = "| "
    for item in tuple:
        retval += str(item) + " | "
    retval += linesep
    return retval

def generateListaNecesidades(personalidad) -> str:
    retval = ""
    necesidadesDb : dict = loadJson("config/necesidades.json")["necesidades"]
    dictPersonalidad : dict = dict(personalidad["facetas"]) | dict(personalidad["opiniones"])
    listaNecesidades : list = []

    for necesidad in necesidadesDb:
        strength = 0
        #facetas positivas
        addNeeds : list = necesidad[2]
        for addNeed in addNeeds:
            if addNeed in dictPersonalidad:
                strength += dictPersonalidad[addNeed]

        #factetas negativas
        removeNeeds : list = necesidad[3]
        for removeNeed in removeNeeds:
            if removeNeed in dictPersonalidad:
                strength -= dictPersonalidad[removeNeed]

        if strength >= 0:
            name = necesidad[0]
        else:
            name = necesidad[1]

        listaNecesidades.append((name, int(strength*2)))
    
    listaNecesidades.sort(key=lambda x: x[1], reverse=True)
    listaNecesidades = [("Necesidad", "Valor (0-100)"), ("---","---")] + listaNecesidades[0:10]
    for line in listaNecesidades:
        retval += fromTupleToMarkdownTable(line)

    return retval

def getFirstHeader(personaje : Character) -> str:
    personaje.reload()
    """La cabecera será '# Apellido Nombre'"""
    return "# " + getFullName(personaje, "Personaje principal") + linesep

def getSecondHeader(personaje : Character) -> str:
    """Cabecera de datos básicos"""

    personaje.reload()

    retval = ""
    retval += "## Datos Básicos" + linesep
    retval += "- Nombre: " + personaje.getNombre() + linesep
    retval += "- Sexo: " + personaje.getSexo() + linesep
    
    retval += "- Edad:" + linesep
    retval += "\t- Año Nacimiento: [[Año " + str(personaje.getNacimiento()) + "]]" + linesep
    retval += "\t- Año Muerte: [[Año " + str(personaje.getMuerte()) + "]]" + linesep
    retval += getEdadList(personaje) + linesep

    retval += "- Familia: [[Familia " + personaje.getFamilia() + "]]" + linesep
    genoma = personaje.getGenoma(fenotipo='config/rokugani.json')
    especies = list(genoma.keys())
    retval += "\t- Especies: " + str(especies) + linesep
    retval += "\t- Clase social: " + personaje.getClaseSocial() + linesep

    retval += "- Lugar de nacimiento: [[" + personaje.getLugarNacimiento()["nombre"] + "]]" + linesep
    retval += "- Lugar de residencia: [[" + personaje.getLugarResidencia()["nombre"] + "]]" + linesep
    
    personaje.save()

    return retval

def getThirdHeader(personaje : Character) -> str:
    """Trasfondo"""

    personaje.reload()
    eventos : list = list()
    eventos.append( (int(personaje.getNacimiento()), "Nacimiento") )

    retval = ""
    retval += "## Trasfondo" + linesep
    retval += "#### Situacion familiar" + linesep

    padre = personaje.getPadre()
    madre = personaje.getMadre()

    retval += "- Padre: [[" + getFullName(padre, "Padre") + "]]" + linesep
    retval += "- Madre: [[" + getFullName(madre, "Madre") + "]]" + linesep
    eventos.append( (int(padre.getMuerte()), "muerte de [[" + getFullName(padre, "Padre") + "]]") )
    eventos.append( (int(madre.getMuerte()), "muerte de [[" + getFullName(madre, "Madre") + "]]") )
    retval += linesep + linesep
    
    retval += "- Hermanos:" + linesep
    for hermano in personaje.getHermanos():
        if hermano != personaje.file:
            bro = Character({}, hermano)
            pr = "hermano" if bro.getSexo() == "Hombre" else "hermana"
            retval += "\t- [[" + getFullName(bro, pr) + "]]" + linesep
            eventos.append( (int(bro.getNacimiento()), "nacimiento de su "+pr+" [[" + getFullName(bro, pr) + "]]") )
            eventos.append( (int(bro.getMuerte()), "muerte de su "+pr+" [[" + getFullName(bro, pr) + "]]") )
    retval += linesep + linesep

    conyugue = personaje.getConyugue()
    retval += "- Conyugue:  [[" + getFullName(conyugue, "Pareja") + "]]" + linesep
    conyugue.reload()
    eventos.append( (int(personaje.getMatrimonio()), "boda con [[" + getFullName(conyugue, "Pareja") + "]]") )
    eventos.append( (int(conyugue.getMuerte()), "muerte de [[" + getFullName(conyugue, "Pareja") + "]]") )
    retval += linesep + linesep

    retval += "- Hijos:" + linesep
    for hijo in personaje.getHijos():
        bro = Character({}, hijo)
        pr = "hijo" if bro.getSexo() == "Hombre" else "hija"
        retval += "\t- [[" + getFullName(bro, pr) + "]]" + linesep
        eventos.append( (int(bro.getNacimiento()), "nacimiento de su "+pr+" [[" + getFullName(bro, pr) + "]]") )
        eventos.append( (int(bro.getMuerte()), "muerte de su "+pr+" [[" + getFullName(bro, pr) + "]]") )
    retval += linesep + linesep
    
    retval += "#### Linea temporal" + linesep
    muerte = personaje.getMuerte()
    eventos.append( (int(muerte), "muerte.") )

    eventos.sort(key=lambda x: x[0])
    for evento in eventos:
        if evento[0] <= muerte:
            retval += "- [[Año " + str(evento[0]) + "]] : " + evento[1] + linesep

    personaje.save()

    return retval

def getFourthHeader(personaje : Character) -> str:
    """Personalidad"""

    personaje.reload()
    retval = ""
    retval += "## Personalidad" + linesep
    retval += "### Facetas" + linesep
    personalidad : dict = personaje.getPersonalidad()

    listaFacetas = list(dict(personalidad["facetas"]).items())
    listaFacetas.sort(key=lambda x: -x[1])
    listaFacetas = listaFacetas[0:5] + listaFacetas[len(listaFacetas)-5:len(listaFacetas)]
    retval += fromTupleToMarkdownTable(("Faceta", "Valor (-50:50)"))
    retval += fromTupleToMarkdownTable(("---", "---"))
    for faceta in listaFacetas:
        retval += fromTupleToMarkdownTable((faceta[0], int(faceta[1])))
    
    retval += linesep + linesep
    retval += "### Opiniones" + linesep

    listaOpiniones = list(dict(personalidad["opiniones"]).items())
    listaOpiniones.sort(key=lambda x: -x[1])
    listaOpiniones = listaOpiniones[0:5] + listaOpiniones[len(listaOpiniones)-5:len(listaOpiniones)]
    retval += fromTupleToMarkdownTable(("Opinion", "Valor (-50:50)"))
    retval += fromTupleToMarkdownTable(("---", "---"))
    for opinion in listaOpiniones:
        retval += fromTupleToMarkdownTable((opinion[0], int(opinion[1])))

    retval += linesep + linesep
    retval += "### Necesidades" + linesep

    retval += generateListaNecesidades(personalidad)

    return retval
    

def markdownGenerator(jsonFile : str) -> str:
    endl = linesep
    personaje = Character({}, jsonFile)

    retval = getFirstHeader(personaje) + endl
    retval += getSecondHeader(personaje) + endl
    retval += getThirdHeader(personaje) + endl
    retval += getFourthHeader(personaje) + endl 

    return retval