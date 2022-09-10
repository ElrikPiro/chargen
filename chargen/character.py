from .core import loadJson, writeJson, generateNewNameId, generateNewFamilyId, generateNewLugar
from .core import RelationType
import random
import math

nan = float("nan")

class Caller:
    def __init__(self, file='', relation=RelationType.NONE):
        self.file = file
        self.relation = relation

class Character:
    def __init__(self, deductions, jsonRef='', caller=Caller()):
        if jsonRef != '':
            self.data = loadJson(jsonRef)
            self.file = jsonRef
            if caller.relation != RelationType.NONE:
                self.updateJsonStruct(deductions, caller)
                self.save()
        else:
            self.data = deductions
            self.generateJsonStruct(deductions, caller)
            self.generateJsonFileName()
            self.save()
        return
    
    def reload(self):
        self.data = loadJson(self.file)
        return self.data

    def updateJsonStruct(self, deductions, caller=Caller()):
        if caller.relation != RelationType.NONE:
            self.data = self.data | deductions
    
    def generateJsonStruct(self, deductions, caller=Caller()):
        if caller.relation == RelationType.NONE:
            self.data = {
                "nombre" : nan,
                "eventos" : {
                    "nacimiento" : nan,
                    "muerte" : nan,
                    "matrimonio" : nan 
                },
                "sexo" : nan,
                "edad" : {"" : nan},
                "familia" : nan,
                "lugar_nacimiento" : nan,
                "lugar_residencia" : nan,
                "clase_social" : nan,
                "parientes" : {
                    "padre" : nan,
                    "madre" : nan,
                    "hermanos" : nan,
                    "conyugue" : nan,
                    "hijos" : {
                        "len": nan,
                        "lista": []
                    }
                },    
            }
            self.data = self.data | deductions
            print("Default jsonfile, fill as desired")
        else:
            self.updateJsonStruct(deductions, caller)

    def generateJsonFileName(self):
        nombrefichero = "personaje_" + str(self.getNombreId())
        self.file = nombrefichero+".json"

    def save(self):
        writeJson(self.file, self.data)

    def getNombreId(self):
        nombre = self.data.get("nombre", nan)
        if nombre != nombre:
            nombre = generateNewNameId()
            self.data["nombre"] = nombre
        return nombre

    def hasMother(self):
        parientes = self.data.get("parientes", nan)
        if parientes != parientes:
            return 0
        madre = parientes.get("madre", nan)
        if madre != madre:
            return 0
        return 1

    def hasFather(self):
        parientes = self.data.get("parientes", nan)
        if parientes != parientes:
            return 0
        madre = parientes.get("padre", nan)
        if madre != madre:
            return 0
        return 1

    def rollEdadMuerte(self, base=40, sign=1):
        roll = random.randint(1,20)
        muerte = base
        if roll == 1:
            return muerte + self.rollEdadMuerte(0, -1)
        elif roll == 20:
            return muerte + self.rollEdadMuerte(0, 1)
        else:
            return muerte + roll*sign

    def generateMuerte(self):
        nacimiento = self.getNacimiento()
        self.data["eventos"]["muerte"] = nacimiento + self.rollEdadMuerte()
        self.save()

    def getMuerte(self):
        eventos = self.data.get("eventos", nan)
        if eventos != eventos:
            self.generateMuerte()
        muerte = eventos.get("muerte", nan)    
        if(muerte != muerte):
            self.generateMuerte()
        return self.data["eventos"]["muerte"]

    def rollEdadMatrimonio(self, base=16, sign=1):
        roll = random.randint(0,9)
        matrimonio = base
        if roll == 9:
            return matrimonio + self.rollEdadMatrimonio(0, 1)
        else:
            return matrimonio + roll*sign

    def generateMatrimonio(self):
        nacimiento = self.getNacimiento()
        muerte = self.getMuerte()
        matrimonio = nacimiento + self.rollEdadMatrimonio()
        if matrimonio > muerte:
            self.data["eventos"]["matrimonio"] = "Soltero"
        else:
            self.data["eventos"]["matrimonio"] = matrimonio
        self.save()

    def getMatrimonio(self):
        eventos = self.data.get("eventos", nan)
        if eventos != eventos:
            self.generateMatrimonio()
        matrimonio = eventos.get("matrimonio", nan)    
        if(matrimonio != matrimonio):
            self.generateMatrimonio()
        return self.data["eventos"]["matrimonio"]

    def hasSpouse(self):
        parientes = self.data.get("parientes", nan)
        if parientes != parientes:
            return 0
        conyugue = parientes.get("conyugue", nan)
        if conyugue != conyugue:
            return 0
        return 1

    def hasDescendants(self):
        parientes = self.data.get("parientes", nan)
        if parientes != parientes:
            return 0
        hijos = parientes.get("hijos", nan)
        if hijos != hijos:
            return 0
        lista = hijos.get("lista", [])
        if len(lista) == 0:
            return 0
        return 1

    def generateNacimiento(self):
        if self.hasMother():
            madre = self.data["parientes"]["madre"]
            madre = Character({}, madre)
            self.data["eventos"]["nacimiento"] = madre.getMatrimonio()
        elif self.hasSpouse():
            self.data["eventos"]["nacimiento"] = self.getMatrimonio() - self.rollEdadMatrimonio()
        elif self.hasDescendants():
            primogenito = self.data["parientes"]["hijos"]["lista"][0]
            primogenito = Character({}, primogenito)
            self.data["eventos"]["nacimiento"] = primogenito.getNacimiento() - self.rollEdadMatrimonio()
        else:
            print("Datos insuficientes para fecha de nacimiento")
            self.data["eventos"]["nacimiento"] =  int(input("Introduzca manualmente la fecha preferida: "))

    def getNacimiento(self):
        eventos = self.data.get("eventos", nan)
        if eventos != eventos:
            self.generateNacimiento()
        nacimiento = eventos.get("nacimiento", nan)    
        if(nacimiento != nacimiento):
            self.generateNacimiento()
        return self.data["eventos"]["nacimiento"]

    def getSexo(self):
        sexo = self.data.get("sexo", nan)
        if sexo != sexo:
            opciones = ["Mujer", "Hombre"]
            sexo = random.choice(opciones)
            self.save()
        return sexo

    def getEdad(self, obra : str):
        obras = loadJson("config/obras.json")
        anyoObra = obras.get(obra, nan)

        if math.isnan(anyoObra):
            return math.nan

        anyosDesdeNacimiento = anyoObra - self.getNacimiento()
        anyosDesdeMuerte = anyoObra - self.getMuerte()
        if anyosDesdeNacimiento > 0 and anyosDesdeMuerte < 0:
            return anyosDesdeNacimiento
        elif anyosDesdeNacimiento > 0 and anyosDesdeMuerte >= 0:
            return "Fallecido"
        else:
            return nan

    def getNombre(self):
        nombreId = self.getNombreId()
        nombresPropios = loadJson("config/nombresPropios.json")
        return nombresPropios.get(str(nombreId), nombreId)
    
    def deducirFamilia(self):
        if self.hasFather() == 0:
            return generateNewFamilyId()
        padre = self.data.get("parientes").get("padre")
        padre = Character({}, padre, Caller(self.file, RelationType.DESCENDANT))
        return padre.getFamiliaId()


    def getFamiliaId(self):
        familia = self.data.get("familia", nan)
        if familia != familia:
            familia = self.deducirFamilia()
            self.data["familia"] = familia
            self.save()
        return familia

    def getFamilia(self):
        familiaId = self.getFamiliaId()
        familias = loadJson("config/familias.json")

        # Es un placeholder?
        isPlaceholder = familiaId.find("PLACEHOLDER") != -1

        if isPlaceholder:
            redirect = familias.get(str(familiaId), familiaId)
            if redirect.find("PLACEHOLDER") != -1:
                return "Indefinido"
            else:
                self.data["familia"] = redirect
                self.save()
                return redirect
        else:
            return familiaId

    def deducirLugarNacimiento(self):
        if self.hasFather() == 0:
            return generateNewLugar()
        padre = self.data.get("parientes").get("padre")
        padre = Character({}, padre, Caller(self.file, RelationType.DESCENDANT))
        return padre.getLugarNacimientoId()

    def getLugarNacimientoId(self):
        lugar_nacimiento = self.data.get("lugar_nacimiento", nan)
        if lugar_nacimiento != lugar_nacimiento:
            lugar_nacimiento = self.deducirLugarNacimiento()
            self.data["lugar_nacimiento"] = lugar_nacimiento
            self.save()
        return lugar_nacimiento

    def getLugarResidenciaId(self):
        lugar_residencia = self.data.get("lugar_residencia", nan)
        if lugar_residencia != lugar_residencia:
            sexo = self.getSexo()
            if sexo == "Mujer" and self.hasSpouse() != 0:
                conyugue = self.data.get("parientes").get("conyugue")
                conyugue = Character({}, conyugue, Caller(self.file, RelationType.SPOUSE))
                lugar_residencia = conyugue.getLugarResidenciaId()
            else:
                lugar_residencia = self.getLugarNacimientoId()

            self.data["lugar_residencia"] = lugar_residencia
            self.save()
        return lugar_residencia

    def getLugarNacimiento(self):
        lugarId = self.getLugarNacimientoId()
        lugar = loadJson("config/localizaciones.json")

        # Es un placeholder?
        isPlaceholder = lugarId.find("PLACEHOLDER") != -1

        if isPlaceholder:
            redirect = lugar.get(str(lugarId), lugarId)
            if redirect["nombre"].find("PLACEHOLDER") != -1:
                return redirect
            else:
                self.data["lugar_nacimiento"] = redirect["nombre"]
                self.save()
                return lugar.get(str(redirect["nombre"]), redirect)
        else:
            return lugar.get(str(lugarId), lugarId)

    def getLugarResidencia(self):
        lugarId = self.getLugarResidenciaId()
        lugar = loadJson("config/localizaciones.json")
        
        # Es un placeholder?
        isPlaceholder = lugarId.find("PLACEHOLDER") != -1

        if isPlaceholder:
            redirect = lugar.get(str(lugarId), lugarId)
            if redirect["nombre"].find("PLACEHOLDER") != -1:
                return redirect
            else:
                self.data["lugar_nacimiento"] = redirect["nombre"]
                self.save()
                return lugar.get(str(redirect["nombre"]), redirect)
        else:
            return lugar.get(str(lugarId), lugarId)

    def getClaseSocial(self):
        clase_social = self.data.get("clase_social", nan)
        if clase_social != clase_social:
            sexo = self.getSexo()
            if sexo == "Mujer" and self.hasSpouse() != 0:
                conyugue = self.data.get("parientes").get("conyugue")
                conyugue = Character({}, conyugue, Caller(self.file, RelationType.SPOUSE))
                clase_social = conyugue.getClaseSocial()
            elif self.hasFather() == 1:
                padre = self.data.get("parientes").get("padre")
                padre = Character({}, padre, Caller(self.file, RelationType.DESCENDANT))
                clase_social = padre.getClaseSocial()
            else:
                clase_social = "PLACEHOLDER"

            self.data["clase_social"] = clase_social
            self.save()
        return clase_social

    def getPadre(self):
        if self.hasFather() == 1:
            return Character({}, self.data["parientes"]["padre"])
        else:
            padre = Character(
                {
                    "eventos": {
                        "matrimonio": self.data["eventos"]["nacimiento"]
                    },
                    "sexo": "Hombre",
                    "familia": self.getFamiliaId(),
                    "lugar_residencia": self.getLugarNacimientoId(),
                    "clase_social": self.getClaseSocial(),
                    "parientes": {
                        "hijos" : {
                            "len": nan,
                            "lista": [
                                self.file
                            ]
                        },
                    }
                },
                '',
                Caller(self.file, RelationType.DESCENDANT)
            )

            self.data["parientes"]["padre"] = padre.file
            self.save()
            return padre
    
    def setConyugue(self, c):
        conyugue = Character(
                {
                    "eventos": {
                        "matrimonio": self.getMatrimonio()
                    },
                    "sexo": "Mujer" if self.getSexo()=="Hombre" else "Hombre",
                    "lugar_residencia": self.getLugarResidenciaId(),
                    "clase_social": self.getClaseSocial() if self.getSexo()=="Hombre" else nan,
                    "parientes": {
                        "conyugue": self.file,
                        "hijos" : self.data["parientes"].get("hijos", {"len": nan, "lista": []})
                    }
                },
                c.file,
                Caller(self.file, RelationType.SPOUSE)
            )
        self.data["parientes"]["conyugue"] = conyugue.file
        self.save()

    def getConyugue(self):
        if self.hasSpouse() == 1:
            return Character({}, self.data["parientes"]["conyugue"])
        else:
            conyugue = Character(
                {
                    "eventos": {
                        "matrimonio": self.getMatrimonio()
                    },
                    "sexo": "Mujer" if self.getSexo()=="Hombre" else "Hombre",
                    "lugar_residencia": self.getLugarResidenciaId(),
                    "clase_social": self.getClaseSocial() if self.getSexo()=="Hombre" else nan,
                    "parientes": {
                        "conyugue": self.file,
                        "hijos" : self.data["parientes"].get("hijos", {"len": nan, "lista": []})
                    }
                },
                '',
                Caller(self.file, RelationType.SPOUSE)
            )
            self.data["parientes"]["conyugue"] = conyugue.file
            self.save()
            return conyugue

    def getMadre(self):
        if self.hasMother() == 1:
            return Character({}, self.data["parientes"]["madre"])
        else:
            padre = self.getPadre()
            madre = padre.getConyugue()
            self.data["parientes"]["madre"] = madre.file
            self.save()
            return madre

    def rollOffsetNacimiento(self, base=0):
        roll = random.randint(0,5)
        offset = base
        if roll == 5:
            return self.rollOffsetNacimiento(offset + roll)
        else:
            return offset + roll

    def generateNumeroHijos(self):
        return int(round(math.fabs(random.normalvariate(2.5, 2))))

    def generateNewChild(self, paterna, materna, nacimiento) -> str:
        newChild = Character({
            "nombre": generateNewNameId(),
            "eventos": {
                "nacimiento": nacimiento
            },
            "sexo": random.choice(["Hombre", "Mujer"]),
            "familia": paterna.getFamiliaId(),
            "lugar_nacimiento": materna.getLugarResidenciaId(),
            "clase_social": paterna.getClaseSocial(),
            "parientes": {
                "padre": paterna.file,
                "madre": materna.file
            }
        }, '', Caller(self.file, RelationType.PARENT))
        return newChild.file

    def getHijos(self):
        paterna = self if self.getSexo() == "Hombre" else self.getConyugue()
        materna = self if self.getSexo() != "Hombre" else self.getConyugue()
        if self.hasDescendants() == 1:
            hijos = self.data.get("parientes", {}).get("hijos", {"len": nan, "lista": []})
            longitud = hijos["len"]
            listaHijos = hijos["lista"]
        else:
            hijos = self.getConyugue().data.get("parientes", {}).get("hijos", {"len": nan, "lista": []})
            longitud = hijos["len"]
            listaHijos = hijos["lista"]
        
        if math.isnan(longitud):
            longitud = self.generateNumeroHijos()

        while len(listaHijos) < longitud:
            deadline = min(
                paterna.getMuerte(), 
                materna.getMuerte(), 
                materna.getNacimiento()+40
            )
            lastChild = None if len(listaHijos) <= 0 else Character({}, listaHijos[len(listaHijos)-1], Caller(self.file, RelationType.PARENT))
            currentYear = self.getMatrimonio() if lastChild == None else lastChild.getNacimiento()
            targetYear = currentYear + self.rollOffsetNacimiento()

            if targetYear >= deadline:
                longitud = len(listaHijos)
                break
            else:
                listaHijos.append(self.generateNewChild(paterna, materna, targetYear))
                
        self.data["parientes"]["hijos"] = {"len" : len(listaHijos), "lista": listaHijos}
        conyugue = self.getConyugue()
        conyugue.data["parientes"]["hijos"] = {"len" : len(listaHijos), "lista": listaHijos}
        conyugue.save()
        self.save()
        return listaHijos
            

    def getHermanos(self):
        return self.getMadre().getHijos()

    def getPersonalidad(self):
        personalidadDB = loadJson("config/personalidad.json")
        listaFacetas : list = personalidadDB["facetas"]
        listaCreencias : list = personalidadDB["opiniones"]
        personalidad = self.data.get("personalidad", nan)
        if personalidad == personalidad:
            return personalidad

        personalidad = {
            "facetas" : {},
            "opiniones" : {}
        }

        padre = self.getPadre().data.get("personalidad", nan)
        madre = self.getMadre().data.get("personalidad", nan)

        #Personalidad
        if padre != padre or madre != madre:
            #distribucion alpha-beta*100-50 si sus padres no tienen personalidad
            for faceta in listaFacetas:
                personalidad["facetas"][faceta] = random.betavariate(6, 6)*100-50
        else:
            #distribucion normal entre personalidad de padre y madre con std = abs(PAPA-MAMA)/2
            for faceta in listaFacetas:
                mean = (padre["facetas"][faceta] + madre["facetas"][faceta])/2
                std = math.fabs(padre["facetas"][faceta]-madre["facetas"][faceta])/2
                personalidad["facetas"][faceta] = max(min(random.normalvariate(mean, std),50),-50)

        #Creencias
        for creencia in listaCreencias:
            personalidad["opiniones"][creencia] = random.betavariate(6, 6)*100-50
        
        self.data["personalidad"] = personalidad
        self.save()

        return personalidad

    def hasGenoma(self):
        miGenoma = self.data.get("genoma", {})
        return miGenoma != {}

    def getGenProgenitor(self, database, progenitor, especie, bodypart, alelo, gen) -> str:
        #esta determinado ya?
        miGenoma = self.data.get("genoma", {})
        dato = dict(miGenoma[especie][bodypart][alelo]).get(gen, {})
        if dato != {}:
            return list(dato.keys())[0]
        
        dato = progenitor.get(especie, {}).get(bodypart, {}).get(alelo, {})
        if dato == {}:
            clave = random.choice(list(dict(database[especie][bodypart][alelo]).keys()))
            return clave
        else:
            valor = random.choice(list(dato.values()))
            if valor == {}:
                clave = random.choice(list(dict(database[especie][bodypart][alelo]).keys()))
                return clave
            else:
                return list(valor.keys())[0]
        

    def getGenoma(self, fenotipo=''):
        genomaDatabase = loadJson("config/genoma.json") if fenotipo=='' else loadJson(fenotipo)
        miGenoma = self.data.get("genoma", dict({}))

        isPapaGenomico = self.getPadre().data["genoma"] if self.getPadre().hasGenoma() else {}
        isMamaGenomica = self.getMadre().data["genoma"] if self.getMadre().hasGenoma() else {}

        #determinamos especie (permitimos multiespecie)
        if miGenoma == {} and (isPapaGenomico != {} or isMamaGenomica != {}):
            miGenoma = {}
            especimenes = [isPapaGenomico.keys(), isMamaGenomica.keys()]
            for progenitor in especimenes:
                for it in progenitor:
                    miGenoma[it] = {}
        elif miGenoma == {}:
            miGenoma["humano"] = {}

        #para cada especie comprobamos si existe cada parte del cuerpo y si no la añadimos
        especies = miGenoma.keys()
        for especie in especies:
            especieDB : dict = genomaDatabase[especie]
            bodyparts = especieDB.keys()
            for part in bodyparts:
                partDB : dict = genomaDatabase[especie][part]
                alelos = partDB.keys()
                miGenoma[especie][part] = dict(miGenoma[especie]).get(part, {})
                for alelo in alelos:
                    miGenoma[especie][part][alelo] = dict(miGenoma[especie][part]).get(alelo, {"paterno": {}, "materno": {}})
                    self.data["genoma"] = miGenoma
                    clavePadre = self.getGenProgenitor(genomaDatabase, isPapaGenomico, especie, part, alelo, "paterno")
                    claveMadre = self.getGenProgenitor(genomaDatabase, isMamaGenomica, especie, part, alelo, "materno")
                    miGenoma[especie][part][alelo]["paterno"][clavePadre] = genomaDatabase[especie][part][alelo][clavePadre]
                    miGenoma[especie][part][alelo]["materno"][claveMadre] = genomaDatabase[especie][part][alelo][claveMadre]
            
        self.data["genoma"] = miGenoma
        self.save()

        return miGenoma

