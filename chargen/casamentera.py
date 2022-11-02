from cmath import nan
import math
import random
from . import character
from . import loadJson
from .render import getExpresion, getFullName
from .character import Character
from .grafolocalizaciones import GrafoLocalizaciones

def isReachable(localizacionA : str, localizacionB : str) -> bool:

        grafo = GrafoLocalizaciones(json="config/localizaciones.json")

        if localizacionA == localizacionB:
            return True

        return grafo.getShortestPath(localizacionA, localizacionB)[1] >= 0.0

def getDistanciaSocial(a : Character, b : Character) -> float:
    
    grafo = GrafoLocalizaciones(json="config/localizaciones.json")
    localizacionA = a.getLugarNacimientoId()
    localizacionB = b.getLugarNacimientoId()

    if localizacionA == localizacionB:
        return 0

    return grafo.getShortestPath(localizacionA, localizacionB)[1]


class Casamentera:
    
    poblacion_ : list[int]
    year_ : int
    end_ : int
    debug_ : bool
    usarLocalizaciones_ : bool

    def __init__(self, poblacion : list[int], begin : int, end : int, debug : bool = False, usarLocalizaciones : bool = False):
        self.poblacion_ = poblacion
        self.year_ = begin
        self.end_ = end
        self.debug_ = debug
        self.usarLocalizaciones_ = usarLocalizaciones

    def log(self, msg : str):
        if self.debug_:
            print(msg + '\n')

    def iterar(self):
        
        log = self.log
        log(f"Iterando sobre el año {self.year_}")

        self.cleanPopulation()
        poblacionValida : tuple[list[Character], list[int]] = self.getPoblacionValida(self.year_)

        #Evaluar la deseabilidad absoluta:
        listaDeseabilidades : dict = self.getDeseabilidadAbsoluta(poblacionValida[0])
        listaDivas : list = []
        
        #para cada personaje en la lista de deseabilidad
            
        for cIdx in list(listaDeseabilidades.keys()):

            #Si el personaje ya esta fuera del mercado, nos lo saltamos
            if self.poblacion_.count(cIdx) == 0 or listaDivas.count(cIdx) > 0:
                continue

            #   se calcula la fecha de matrimonio
            c : Character = Character({}, f"personaje_{cIdx}.json")
            fechaMatrimonio = self.year_

            if self.debug_:
                log(f"\tEvaluando al candidato {getFullName(c)}")
            
            #   se hace una lista similar para esa fecha, pero descartando al mismo sexo 
            # y a aquellos con una deseabilidad por debajo de la mitad de la del individuo evaluado
            locChar = "" if not self.usarLocalizaciones_ else c.getLugarNacimientoId() 
            poblacionApta = self.getPoblacionValida(fechaMatrimonio, c.getSexo(), localizacion=locChar)
            deseabilidadAbsCandidatos = self.getDeseabilidadAbsoluta(poblacionApta[0], min=float(listaDeseabilidades[cIdx])/2)
            
            
            #   se ordena aleatoriamente la lista y se evalua la deseabilidad relativa de 
            # del resto de la lista que supere ese valor, si se queda sin valores, el personaje
            # los primeros len(lista)/e quedandonos con el maximo valor.
            randomDeseabilidadesAbsIdx = list(deseabilidadAbsCandidatos.keys())
            for diva in listaDivas:
                if diva in randomDeseabilidadesAbsIdx:
                    randomDeseabilidadesAbsIdx.remove(diva)
            random.shuffle(randomDeseabilidadesAbsIdx)
            nSampleCandidates = int(len(randomDeseabilidadesAbsIdx)/math.e)
            
            log(f"\tEvaluando la deseabilidad relativa de los primeros n/e candidatos")
            minRequiredCandidate : int = 0
            aux = randomDeseabilidadesAbsIdx.copy()
            for idx in range(nSampleCandidates):
                minRequiredCandidate = max(minRequiredCandidate, self.getDeseabilidadRelativa(cIdx, aux[idx]))
                randomDeseabilidadesAbsIdx.remove(aux[idx])

            log(f"\tLos estandares son ahora una relativa de {minRequiredCandidate}")
            #   se busca el primer candidato que supere el valor minimo marcado
            #   Ese será el candidato escogido
            candidato = None
            for idx in randomDeseabilidadesAbsIdx:
                if self.getDeseabilidadRelativa(cIdx, idx) > minRequiredCandidate and cIdx != idx:
                    log(f"\tSe selecciona al candidato.")
                    candidato = Character({}, f"personaje_{idx}.json")
                    break
            
            # se casan, se añaden los hijos a la poblacion
            # y se elimina a ambos conyugues de la lista de candidatos
            if candidato != None:
                c : Character = Character({}, f"personaje_{cIdx}.json")
                eventos : dict = c.data["eventos"]
                eventos["matrimonio"] = fechaMatrimonio
                c.data["eventos"] = eventos
                c.save()
                self.poblacion_.remove(cIdx)
                self.poblacion_.remove(candidato.getNombreId())
                c.setConyugue(candidato)
                listaHijos = c.getHijos()
                log(f"\tSe casan y tienen {len(listaHijos)} hijo/s.")
                for hijoFile in listaHijos:
                    self.poblacion_.append(Character({}, hijoFile).getNombreId())
            else:
                log(f"\tEl personaje se queda sin candidatos por diva.")
                listaDivas.append(cIdx)
                c : Character = Character({}, f"personaje_{cIdx}.json")
                eventos : dict = c.data["eventos"]
                eventos["matrimonio"] = nan
                c.data["eventos"] = eventos
                c.save()
            
            continue
            
        #   desde el año actual+1 hasta 10 años en adelante se busca la fecha con el máximo
        # de población válida y se designa ese año como año actual

        log(f"Se evalúa cual debe ser el próximo año de iteración.")
        selectedYearAndCandidates : tuple(int, int) = (self.year_, 0)
        for candidateYear in range(self.year_+1, self.year_+11):
            nCandidatos = len(self.getPoblacionValida(candidateYear)[1])
            if selectedYearAndCandidates[1] < nCandidatos or selectedYearAndCandidates[1] <= 1:
                selectedYearAndCandidates = (candidateYear, nCandidatos)

        log(f"Con {selectedYearAndCandidates[1]} candidatos, el próximo año en ser evaluado será {selectedYearAndCandidates[0]}.")
        self.year_ = selectedYearAndCandidates[0]

        pass

    def getDeseabilidadRelativa(self, entrevistador, entrevistado, fenotipo=''):
         #   NOTA:
            #   como evaluar la deseabilidad relativa
                #   Se buscará que expresen el mismo nivel de recisividad en sus genes (1), mejor recisividad (2) o peor (0.5)
                c = Character({}, f"personaje_{entrevistador}.json")
                d = Character({}, f"personaje_{entrevistado}.json")
                log = self.log
                if self.debug_:
                    log(f"\t\t\tEvaluando deseabilidad relativa de {getFullName(d)} frente a {getFullName(c)}")

                genomaDatabase = loadJson("config/genoma.json") if fenotipo=='' else loadJson(fenotipo)

                genoma = c.getGenoma()
                especies = genoma.keys()
                atraccionGenetica = 1
                for especie in especies:
                    especieDB : dict = genoma[especie]
                    bodyparts = especieDB.keys()
                    for part in bodyparts:
                        partDB : dict = genoma[especie][part]
                        alelos = partDB.keys()
                        genoma[especie][part] = dict(genoma[especie]).get(part, {})
                        for alelo in alelos:
                            expresionC = getExpresion(genoma, c.getSexo(), f"{especie};{part};{alelo}")
                            expresionD = getExpresion(genoma, d.getSexo(), f"{especie};{part};{alelo}")
                            valueC = 0.5 if expresionC == "incompleto" else genomaDatabase[especie][part][alelo][expresionC]["dominancia"]
                            valueD = 0.5 if expresionD == "incompleto" else genomaDatabase[especie][part][alelo][expresionD]["dominancia"]
                            atraccionGenetica *= 1 if valueC == valueD else 2 if valueC < valueD else 0.5

                #   Se buscará que las opiniones y las facetas sean coincidentes
                personalidadC = c.getPersonalidad()
                personalidadD = d.getPersonalidad()

                opinionesC : dict = personalidadC["opiniones"]
                opinionesD : dict = personalidadD["opiniones"]
                atraccionIdeologica : float = 1.0
                for opinion in list(opinionesC.keys()):
                    valueC = opinionesC[opinion]
                    valueD = opinionesD[opinion]
                    atraccionIdeologica *= 1.0 if math.copysign(1, valueC) == math.copysign(1, valueD) else 1.0-abs(valueC-valueD)/100.0

                facetasC : dict = personalidadC["facetas"]
                facetasD : dict = personalidadD["facetas"]
                atraccionTemperamento : float = 1.0
                for faceta in list(facetasC.keys()):
                    valueC = facetasC[faceta]
                    valueD = facetasD[faceta]
                    atraccionTemperamento *= 1.0-abs(valueC-valueD)/100.0 if math.copysign(1, valueC) == math.copysign(1, valueD) else 1.0-abs(valueC-valueD)/100.0

                
                #   Se evaluará la consanguinidad: 0.25 hermanos, 0.5 padres e hijos, 0.75 abuelos, tios, primos y nietos
                consanginidad = 1
                    # Es hermano?
                    # Sacamos una lista de hermanos
                listaHermanos = c.getPadre().getHijos()
                listaHermanosIdx : list = []
                for hermano in listaHermanos:
                    listaHermanosIdx.append(Character({}, hermano).getNombreId())
                
                    #Es padre o hijo
                listaPadreHijosIdx : list = [
                    c.getPadre().getNombreId(),
                    c.getMadre().getNombreId()
                ]
                
                listaHijos = c.getHijos() if c.hasDescendants() else []
                for hijo in listaHijos:
                    listaPadreHijosIdx.append(Character({}, hijo).getNombreId())

                    #Es tio o primo?
                
                listaTios : list(str) = []
                listaTios += (c.getPadre().getHermanos()) if c.getPadre().hasMother() else []
                listaTios += (c.getMadre().getHermanos()) if c.getMadre().hasMother() else []
                listaPrimos : list(str) = []
                listaPrimosTiosIdx : list(int) = []
                for tio in listaTios:
                    cTio = Character({}, tio)
                    listaPrimosTiosIdx.append(cTio.getNombreId())
                    if cTio.hasDescendants():
                        listaPrimos += (cTio.getHijos())

                for primo in listaPrimos:
                    cPrimo = Character({}, primo)
                    listaPrimosTiosIdx.append(cPrimo.getNombreId())


                if entrevistado in listaHermanosIdx:
                    consanginidad = 0.25

                if entrevistado in listaPadreHijosIdx:
                    consanginidad = 0.5

                if entrevistado in listaPrimosTiosIdx:
                    consanginidad = 0.75

                distanciamiento = getDistanciaSocial(c, d) / 10.0 if self.usarLocalizaciones_ else 0.0

                diferenciaEdad = (c.getNacimiento() - d.getNacimiento()) / 20.0

                log(f"\t\t\t consanguinidad={consanginidad} temperamento={atraccionTemperamento} ideologia={atraccionIdeologica} genetica={atraccionGenetica}")
                log(f"\t\t\t total={consanginidad * (atraccionTemperamento + atraccionIdeologica + atraccionGenetica)}")
                return consanginidad * (atraccionTemperamento + atraccionIdeologica + atraccionGenetica) - distanciamiento - diferenciaEdad

    def getDeseabilidadAbsoluta(self, lista : list[Character], fenotipo = '', min : float = 0):
        
        log = self.log
        log(f"\tEvaluando deseabilidad absoluta de la población {lista} con fenotipo={fenotipo} y un valor mínimo de {min}")

        CLASES_SOCIALES_PONDERACION : dict = {
            "Noble" : 3,
            "Alta" : 2,
            "Media" : 1,
            "Ciudadano" : 1,
            "Pobre" : 0.5,
            "Paria" : 0.25
        }

        FACETAS_RELEVANTES_PONDERACION : dict = {
            "IMMODERATION" : -1,
            "POLITENESS" : 1,
            "LOVE_PROPENSITY" : 1,
            "STRESS_VULNERABILITY" : -1,
            "CONFIDENCE" : 1,
            "ANGER_PROPENSITY" : -1,
        }

        OPINIONES_RELEVANTES_PONDERACION : dict = {
            "ROMANCE" : 1,
            "MERRIMENT" : 1,
            "INDEPENDENCE" : -1,
            "FAMILY" : 1
        }

        listaDeseabilidades = {}
        genomaDatabase = loadJson("config/genoma.json") if fenotipo=='' else loadJson(fenotipo)

        
        for c in lista:
            
            idxStr = f"Personaje nº{c.getNombreId()}"
            if self.debug_:
                log(f"\t\tEvaluando deseabilidad absoluta de {getFullName(c, idxStr)}")
            
            sexo = c.getSexo()
            genoma = c.getGenoma()
            personalidad = c.getPersonalidad()
            genTot = 0
            genCount = 0

            # numero de genes recesivos vs dominantes expresados (media del total) GEN
            especies = genoma.keys()
            for especie in especies:
                especieDB : dict = genoma[especie]
                bodyparts = especieDB.keys()
                for part in bodyparts:
                    partDB : dict = genoma[especie][part]
                    alelos = partDB.keys()
                    genoma[especie][part] = dict(genoma[especie]).get(part, {})
                    for alelo in alelos:
                        expresion = getExpresion(genoma, sexo, f"{especie};{part};{alelo}")
                        genCount += 1
                        genTot += 0.5 if expresion == "incompleto" else genomaDatabase[especie][part][alelo][expresion]["dominancia"]

            GEN = genTot / genCount

            # clase social (3 nobleza, 2 clase alta, 1 clase media, 0.5 clase pobre, 0.25 paria) CLASE
            claseDeclarada = c.getPadre().getClaseSocial() if CLASES_SOCIALES_PONDERACION.get(c.getClaseSocial(), None) == None else c.getClaseSocial()

            CLASE = float(CLASES_SOCIALES_PONDERACION[claseDeclarada])

            # productorio de facetas relevantes normalizadas entre 0.5 y 2
            FACETA = 1
            for facet in list(FACETAS_RELEVANTES_PONDERACION.keys()) :
                FACETA *= (float(personalidad["facetas"][facet]) * FACETAS_RELEVANTES_PONDERACION[facet] + 50)*1.5/100 + 0.5

            # productorio de opiniones relevantes normalizadas entre 0.5 y 2
            OPINION = 1
            for opinion in list(OPINIONES_RELEVANTES_PONDERACION.keys()) :
                OPINION *= (float(personalidad["opiniones"][opinion]) * OPINIONES_RELEVANTES_PONDERACION[opinion] + 50)*1.5/100 + 0.5

            formula = GEN * CLASE * (FACETA + OPINION)
            log(f"\t\t\tGen={GEN} Clase={CLASE} Faceta={FACETA} Opinion={OPINION} Resultado={formula}.")
            if formula > min:
                listaDeseabilidades[c.getNombreId()] = formula
            else:
                log(f"\t\t\tDeseabilidad insuficiente, descartado.")

        #con la deseabilidad absoluta evaluada se hace un mapa de personaje/deseabilidad y se ordena
        log(f"\tOrdenando lista de desabilidades absolutas")
        listaDeseabilidades = dict(sorted(listaDeseabilidades.items(), key=lambda item: -item[1]))

        return listaDeseabilidades

    def getPoblacionValida(self, year, sexoCandidato = "", localizacion : str = "") -> tuple[list[Character], list[int]]:

        log = self.log
        log(f"Evaluando población válida en el año {year}")

        EDAD_FERTILIDAD_HOMBRE = 16
        EDAD_FERTILIDAD_MUJER = 14
        EDAD_MENOPAUSIA = 40

        poblacionValida : list[Character] = []
        newPoblacion : list[int] = []
        
        for it in self.poblacion_:
            log(f"\tEvaluando personaje nº{it}")
            c = Character({}, f"personaje_{it}.json")
            fechaMuerte = c.getMuerte()
            if fechaMuerte <= year:
                log(f"\t\tDescartado por estar muerto")
                continue

            fechaNacimiento = c.getNacimiento()
            sexo = c.getSexo()
            minFertilidad = fechaNacimiento + (EDAD_FERTILIDAD_HOMBRE if sexo else EDAD_FERTILIDAD_MUJER)
            maxFertilidad = fechaNacimiento + (fechaMuerte if sexo else EDAD_MENOPAUSIA)
            if year < minFertilidad or year >= maxFertilidad:
                log(f"\t\tDescartado por no ser fertil. {year} no está en rango [{minFertilidad},{maxFertilidad}]")
                continue

            if sexoCandidato != "" and sexoCandidato == sexo:
                log(f"\t\tDescartado por no ser del sexo contario.")
                continue

            if c.hasSpouse():
                log(f"\t\tDescartado por estar ya casado.")
                continue

            miLocalizacion = c.getLugarNacimientoId()

            if localizacion != "" and not isReachable(miLocalizacion, localizacion):
                log(f"\t\tDescartado por no ser localizable")
                continue

            poblacionValida.append(c)
            newPoblacion.append(it)
            log(f"\t\tPersonaje aceptado.")

        return (poblacionValida, newPoblacion)

    def cleanPopulation(self):

        EDAD_MENOPAUSIA = 40

        poblacion : list[int] = []

        year = self.year_
        
        for it in self.poblacion_:
            c = Character({}, f"personaje_{it}.json")
            fechaMuerte = c.getMuerte()
            if fechaMuerte <= year:
                continue

            fechaNacimiento = c.getNacimiento()
            sexo = c.getSexo() == "Hombre"
            maxFertilidad = fechaNacimiento + (fechaMuerte if sexo else EDAD_MENOPAUSIA)
            if year >= maxFertilidad:
                continue

            if c.hasSpouse():
                continue

            poblacion.append(it)
        
        self.poblacion_ = poblacion