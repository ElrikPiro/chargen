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
    genoma = personaje.getGenoma()
    especies = list(genoma.keys())
    retval += "\t- Especies: " + str(especies) + linesep
    retval += "\t- Clase social: " + personaje.getClaseSocial() + linesep

    retval += "- Lugar de nacimiento: [[" + personaje.getLugarNacimiento()["nombre"] + "]]" + linesep
    retval += "- Lugar de residencia: [[" + personaje.getLugarResidencia()["nombre"] + "]]" + linesep
    
    personaje.save()

    return retval

def getMermaidFamilyTree(personaje : Character, depthUp : int, depthDown : int) -> str:

    personaje.reload()
    retval = "```mermaid" + linesep
    retval += "flowchart TD" + linesep

    #hacer un for each de todos los personajes ascendentes
    ancestros : list = [personaje]
    up = depthUp
    while up > 0:
        newAncestros : list = []
        for ancestro in ancestros:
            newAncestros.append(ancestro.getPadre())
            newAncestros.append(ancestro.getMadre())
        ancestros = newAncestros
        up -= 1

    #imprimir resultados
    down = depthUp + depthDown
    matrimonios : dict = {}
    while down > 0:
        nextGen : list = []
        for ancestro in ancestros:
            value = ancestro if ancestro.getSexo() == "Hombre" else ancestro.getConyugue()
            marido = value
            mujer = value.getConyugue()
            if marido.getNombreId() not in matrimonios:
                matrimonios[marido.getNombreId()] = mujer.getNombreId()
                retval += "subgraph mat{0}_{1}[ ]".format(marido.getNombreId(), mujer.getNombreId()) + linesep
                retval += "\t"+ marido.file + "[" + getFullName(marido, "Personaje nº{0}".format(marido.getNombreId())) + "]" + linesep
                retval += "\t"+ mujer.file + "[" + getFullName(mujer, "Personaje nº{0}".format(mujer.getNombreId())) + "]" + linesep
                retval += "end" + linesep

                listaHijos = marido.getHijos()
                for hijo in listaHijos:
                    hijoChar = Character({}, hijo)
                    retval += "mat{0}_{1} --> {2}[{3}]".format(marido.getNombreId(), mujer.getNombreId(), hijo, getFullName(hijoChar, "Personaje nº{0}".format(hijoChar.getNombreId()))) + linesep
                    nextGen.append(hijoChar)
        ancestros = nextGen
        down -= 1
    

    retval += "```" + linesep
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

    DEPTH_UP = 1
    DEPTH_DOWN = 1
    retval += getMermaidFamilyTree(personaje, DEPTH_UP, DEPTH_DOWN)

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
    
def getExpresion(genoma : dict, sexo : str, query : str) -> str:
    """Devuelve que gen va a expresarse"""
    lista = query.split(";")
    gen = genoma
    for step in lista:
        gen = gen.get(step)

    paterno : dict = gen.get("paterno")
    paternoKey = list(paterno.keys())[0]
    paternoFeat : dict = paterno.get(paternoKey)
    materno : dict = gen.get("materno")
    maternoKey = list(materno.keys())[0]
    maternoFeat : dict = materno.get(maternoKey)

    if paternoKey == maternoKey:
        return paternoKey
    
    domPaterno = paternoFeat.get("dominancia")
    domMaterno = maternoFeat.get("dominancia")
    if domPaterno == domMaterno:
        expresado = paternoKey if sexo == "Hombre" else maternoKey
    else:
        expresado = paternoKey if domPaterno > domMaterno else maternoKey
    
    if paternoFeat.get("incompleto") != None:
        expresado = "incompleto"

    return expresado

def getDescCabello_(rizado, matizPelo, satPelo, claridadPelo):
    """Devuelve la descripcion del cabello"""
    valorRizado = rizado if rizado != "incompleto" else "ondulado"
    retval = f"{valorRizado}, "

    posibilidades = {
        "Marron" : 0,
        "Rojo" : 1,
        "Blanco" : 2
    }
    valorMatiz = posibilidades.get(matizPelo)
    

    posibilidades = {
        "Profundo" : 0,
        "Neutro" : 1,
        "incompleto" : 2
    }
    valorSaturacion = posibilidades.get(satPelo)

    posibilidades = {
        "Claro" : 0,
        "Oscuro" : 1
    }
    valorClaridad = posibilidades.get(claridadPelo)

    valor = valorMatiz*100 + valorSaturacion*10 + valorClaridad
    posibilidades = {
        0 : "de color castaño claro. ",
        1 : "de color castaño. ",
        10 : "de color rubio platino. ",
        11 : "de color azabache. ",
        20 : "de color rubio oscuro. ",
        21 : "de color castaño oscuro. ",
        100 : "rojo intenso. ",
        101 : "castaño rojizo. ",
        110 : "naranja pálido. ",
        111 : "rojo oscuro. ",
        120 : "dorado cobrizo. ",
        121 : "rojo caoba. ",
        200 : "de color plateado. ",
        201 : "de color gris ceniza. ",
        210 : "de color blanco. ",
        211 : "de color gris oscuro. ",
        220 : "de color plata. ",
        221 : "de color gris plata. ",
    }

    retval += posibilidades.get(valor)

    return retval

def getDescOjos_(tamanoOjos, matizOjos, satOjos, claridadOjos, pestanas, cejas, achinaos):
    """Obtiene la descripción de los ojos"""
    retval = f"{tamanoOjos}, " if tamanoOjos != "incompleto" else ""
    retval += "redondos" if achinaos != "presente" else "rasgados"
    

    posibilidades = {
        "Marron" : 0,
        "Azul" : 1,
        "Verde" : 2,
        "Rojos" : 3
    }
    valorMatiz = posibilidades.get(matizOjos)
    

    posibilidades = {
        "Profundo" : 0,
        "Neutro" : 1,
        "incompleto" : 2
    }
    valorSaturacion = posibilidades.get(satOjos)

    posibilidades = {
        "Claros" : 0,
        "Oscuros" : 1
    }
    valorClaridad = posibilidades.get(claridadOjos)

    valor = valorMatiz*100 + valorSaturacion*10 + valorClaridad
    posibilidades = {
        0 : "ambar",
        1 : "cacao",
        10 : "miel",
        11 : "ebano",
        20 : "dorado",
        21 : "castaños",
        100 : "azul cielo",
        101 : "azul marino",
        110 : "gris pálido",
        111 : "gris oscuro",
        120 : "azul pálido",
        121 : "azul oscuro",
        200 : "verde vivo",
        201 : "verde jade",
        210 : "verde pálido",
        211 : "verde bosque",
        220 : "verde esmeralda",
        221 : "verde oliva",
        300 : "escarlata",
        301 : "rojo oscuro",
        310 : "rosa pálido",
        311 : "rosa oscuro",
        320 : "calabaza",
        321 : "castaño rojizo",
    }
    colorOjos = posibilidades.get(valor)

    retval += f" y de color {colorOjos} con pestañas {pestanas} y bajo unas cejas {cejas}. "

    return retval

def getMofletesYHoyuelos_(mofletes, hoyuelos):
    """Obtiene los mofletes y hoyuelos"""
    retval = ""

    if mofletes == "incompleto" and hoyuelos == "no presente":
        return ""
    
    if hoyuelos == "presente":
        retval += " con hoyuelos en sus mofletes"
    else:
        retval += " con mofletes"

    if mofletes != "incompleto":
        retval += f" {mofletes}"
    
    


    return retval

def getNariz_(puenteNas, anchoNas, tamanoNas):
    #Puente nariz : aguileña, recta
    #Ancho : ancha, estrecha
    #Tamano: grande, pequeña, incompleto

    retval = ""
    
    if tamanoNas != "incompleto":
        retval += f"de tamaño {tamanoNas}, "

    retval += f"con forma {anchoNas} y {puenteNas}"

    return retval

def generateDescripcion(personaje : Character) -> str:
    genoma = personaje.getGenoma()
    nombre = personaje.getNombre()
    sexo = personaje.getSexo()
    lutColorPiel = ["pálida", "clara", "bronceada", "de un color marrón claro", "de un color marrón oscuro", "negra"]
    lutAltura = ["increiblemente pequeña", "notablemente reducida", "media tirando a la baja", "media tirando a lo alto", "notablemente alta", "increiblemente grande"]

    somatotipo = getExpresion(genoma, sexo, "humano;otros;somatotipo")

    query = "humano;general;piel "
    listaPieles = "ABCDE"
    color = 0
    for letra in listaPieles:
        valor = getExpresion(genoma, sexo, query + letra)
        color += 1 if valor == "oscura" else 0

    query = "humano;general;altura "
    listaAlturas = "ABCDE"
    altura = 0
    for letra in listaAlturas:
        valor = getExpresion(genoma, sexo, query + letra)
        altura += 1 if valor == "alto" else 0

    pecas = getExpresion(genoma, sexo, "humano;general;pecas") == "presentes" and color < 4
    vello = getExpresion(genoma, sexo, "humano;general;vello corporal") == "abundante" and sexo == "Hombre"
    abdomen = getExpresion(genoma, sexo, "humano;abdomen;Acumulacion grasa")
    pecho = getExpresion(genoma, sexo, "humano;torso;Tama\u00c3\u00b1o senos") if sexo == "Mujer" else "ausente"
    
    rizado = getExpresion(genoma, sexo, "humano;cabeza;rizado del cabello")
    matizPelo = getExpresion(genoma, sexo, "humano;cabeza;Matiz cabello")
    satPelo = getExpresion(genoma, sexo, "humano;cabeza;Saturacion cabello")
    claridadPelo = getExpresion(genoma, sexo, "humano;cabeza;Claridad cabello")

    pestanas = getExpresion(genoma, sexo, "humano;cabeza;pesta\u00c3\u00b1as")
    cejas = getExpresion(genoma, sexo, "humano;cabeza;cejas")
    achinaos = getExpresion(genoma, sexo, "humano;cabeza;pliegue mongoles")
    tamanoOjos = getExpresion(genoma, sexo, "humano;cabeza;tama\u00c3\u00b1o ojos")
    matizOjos = getExpresion(genoma, sexo, "humano;cabeza;Matiz ojos")
    satOjos = getExpresion(genoma, sexo, "humano;cabeza;Saturacion ojos")
    claridadOjos = getExpresion(genoma, sexo, "humano;cabeza;Claridad ojos")

    formaRostro = getExpresion(genoma, sexo, "humano;cabeza;Forma de la cara")

    mofletes = getExpresion(genoma, sexo, "humano;cabeza;Mofletes")
    hoyuelos = getExpresion(genoma, sexo, "humano;cabeza;hoyuelos")

    puenteNas = getExpresion(genoma, sexo, "humano;cabeza;puente nariz")
    anchoNas = getExpresion(genoma, sexo, "humano;cabeza;ancho nariz")
    tamanoNas = getExpresion(genoma, sexo, "humano;cabeza;tama\u00c3\u00b1o nariz")

    anchoLips = getExpresion(genoma, sexo, "humano;cabeza;ancho de labios")

    descGenero = "un hombre" if sexo == "Hombre" else "una mujer"
    descConstitucion = "atletica" if somatotipo == "incompleto" else \
        ("gruesa" if somatotipo == "ectomorfo" else "delgada")
    descColor = lutColorPiel[color]
    descEstatura = lutAltura[altura]
    descPielGeneral = ""
    if pecas and vello:
        descPielGeneral = ", llena de pecas y con abundante vello corporal"
    elif pecas:
        descPielGeneral = ", llena de pecas"
    elif vello:
        descPielGeneral = ", con abundante vello corporal"
    descAbdomen = "Tiene el vientre curvado" if abdomen == "incompleto" else \
        ("Tiene el vientre abultado" if abdomen == "Elevada" else "Tiene el vientre recto")
    descPecho = "." if pecho == "ausente" else \
        (" y tiene un pecho prominente" if pecho == "Prominente" else \
            (" y tiene un pecho promedio" if pecho == "incompleto" else " y tiene un pecho escaso"))
    descCabello = getDescCabello_(rizado, matizPelo, satPelo, claridadPelo)
    descOjos = getDescOjos_(tamanoOjos, matizOjos, satOjos, claridadOjos, pestanas, cejas, achinaos)
    descFormaCabeza = {
        "Cuadrada" : "cuadrado",
        "Redonda" : "redondo",
        "incompleto" : "ovalado"
    }
    descFormaCabeza = descFormaCabeza.get(formaRostro, formaRostro)
    descMofletesYHoyuelos = getMofletesYHoyuelos_(mofletes, hoyuelos)
    descNariz = getNariz_(puenteNas, anchoNas, tamanoNas)#WIP

    retval = f"{nombre} es {descGenero} de estatura {descEstatura} y constitución {descConstitucion}. "
    retval += f"Su piel es {descColor}{descPielGeneral}. "
    retval += f"{descAbdomen}{descPecho}" + linesep

    retval += f"Su cabello es {descCabello} Sus ojos son {descOjos} "
    retval += f"Tiene el rostro {descFormaCabeza}{descMofletesYHoyuelos}. "
    retval += f"Su nariz es {descNariz} y su boca tiene unos labios {anchoLips}."  + linesep

    return retval

def getFifthHeader(personaje : Character) -> str:
    """Aspecto físico"""

    personaje.reload()
    retval = ""
    retval += "## Aspecto físico" + linesep
    retval += f"{generateDescripcion(personaje)}" + linesep
    return retval

def markdownGenerator(jsonFile : str) -> str:
    endl = linesep
    personaje = Character({}, jsonFile)

    retval = getFirstHeader(personaje) + endl
    retval += getSecondHeader(personaje) + endl
    retval += getThirdHeader(personaje) + endl
    retval += getFourthHeader(personaje) + endl
    retval += getFifthHeader(personaje) + endl

    return retval