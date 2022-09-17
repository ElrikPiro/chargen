import math
import random
from . import character
from . import loadJson
from .render import getExpresion
from character import Character

class Casamentera:
    
    poblacion_ : list[int]
    year_ : int
    end_ : int

    def __init__(self, poblacion : list[int], begin : int, end : int):
        self.poblacion_ = poblacion
        self.year_ = begin
        self.end_ = end

    def iterar(self):
        poblacionValida : tuple[list[Character], list[int]] = self.getPoblacionValida(self.year_)

        #Evaluar la deseabilidad absoluta:
        listaDeseabilidades : dict = self.getDeseabilidadAbsoluta(poblacionValida[0])
        listaDivas : list = []
        
        #para cada personaje en la lista de deseabilidad
            
        for cIdx in list(listaDeseabilidades.items()):
            
            #Si el personaje ya esta fuera del mercado, nos lo saltamos
            if self.poblacion_.count(cIdx) == 0 or listaDivas.count(cIdx) > 0:
                continue

            #   se calcula la fecha de matrimonio
            c : Character = poblacionValida[0][cIdx]
            fechaMatrimonio = c.getMatrimonio()
            
            #   se hace una lista similar para esa fecha, pero descartando al mismo sexo 
            # y a aquellos con una deseabilidad por debajo de la mitad de la del individuo evaluado
            poblacionApta = self.getPoblacionValida(fechaMatrimonio, c.getSexo())
            deseabilidadAbsCandidatos = self.getDeseabilidadAbsoluta(poblacionApta[0], min=float(listaDeseabilidades[cIdx])/2)
            
            
            #   se ordena aleatoriamente la lista y se evalua la deseabilidad relativa de 
            # del resto de la lista que supere ese valor, si se queda sin valores, el personaje
            # los primeros len(lista)/e quedandonos con el maximo valor.
            randomDeseabilidadesAbsIdx = list(deseabilidadAbsCandidatos.keys())
            random.shuffle(randomDeseabilidadesAbsIdx)
            nSampleCandidates = int(len(randomDeseabilidadesAbsIdx)/math.e)
            
            minRequiredCandidate : int = 0
            for idx in range(nSampleCandidates):
                max(minRequiredCandidate, self.getDeseabilidadRelativa(cIdx, idx))
                randomDeseabilidadesAbsIdx.remove(idx)

            #   se busca el primer candidato que supere el valor minimo marcado
            #   Ese será el candidato escogido
            candidato = None
            for idx in randomDeseabilidadesAbsIdx:
                if self.getDeseabilidadRelativa(cIdx, idx) > minRequiredCandidate:
                    candidato = poblacionValida[0][idx]
                    break
            
            # se casan, se añaden los hijos a la poblacion
            # y se elimina a ambos conyugues de la lista de candidatos
            if candidato != None:
                self.poblacion_.remove(cIdx)
                self.poblacion_.remove(candidato)
                c.setConyugue(candidato)
                listaHijos = c.getHijos()
                for hijoFile in listaHijos:
                    self.poblacion_.append(Character({}, hijoFile).getNombreId())
            else:
                listaDivas.append(cIdx)
            
            continue
            
        #   desde el año actual+1 hasta 10 años en adelante se busca la fecha con el máximo
        # de población válida y se designa ese año como año actual

        selectedYearAndCandidates : tuple(int, int) = (self.year_, 0)
        for candidateYear in range(self.year_+1, self.year_+11):
            nCandidatos = len(self.getPoblacionValida(candidateYear)[1])
            if selectedYearAndCandidates[1] <= nCandidatos:
                selectedYearAndCandidates = (candidateYear, nCandidatos)

        pass

    def getDeseabilidadRelativa(self, entrevistador, entrevistado, fenotipo=''):
         #   NOTA:
            #   como evaluar la deseabilidad relativa
                #   Se buscará que expresen el mismo nivel de recisividad en sus genes (1), mejor recisividad (2) o peor (0.5)
                c = Character({}, f"personaje_{entrevistador}.json")
                d = Character({}, f"personaje_{entrevistado}.json")

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
                            valueC = 0.5 if expresionC == "incompleto" else genomaDatabase[especie][part][alelo]["dominancia"]
                            valueD = 0.5 if expresionD == "incompleto" else genomaDatabase[especie][part][alelo]["dominancia"]
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
                for faceta in list(opinionesC.keys()):
                    valueC = facetasC[faceta]
                    valueD = facetasD[faceta]
                    atraccionTemperamento *= 2.0-abs(valueC-valueD) if math.copysign(1, valueC) == math.copysign(1, valueD) else 1.0-abs(valueC-valueD)/100.0

                
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
                listaHijos = c.getHijos()
                for hijo in listaHijos:
                    listaPadreHijosIdx.append(Character({}, hijo).getNombreId())

                    #Es tio o primo?
                listaTios : list(str) = []
                listaTios.append(c.getPadre().getHermanos())
                listaTios.append(c.getMadre().getHermanos())
                listaPrimos : list(str) = []
                listaPrimosTiosIdx : list(int) = []
                for tio in listaTios:
                    cTio = Character({}, tio)
                    listaPrimosTiosIdx.append(cTio.getNombreId())
                    if cTio.hasDescendants():
                        listaPrimos.append(cTio.getHijos())

                for primo in listaPrimos:
                    cPrimo = Character({}, primo)
                    listaPrimosTiosIdx.append(cPrimo.getNombreId())


                if entrevistado in listaHermanosIdx:
                    consanginidad = 0.25

                if entrevistado in listaPadreHijosIdx:
                    consanginidad = 0.5

                if entrevistado in listaPrimosTiosIdx:
                    consanginidad = 0.75

                
                return consanginidad * (atraccionTemperamento + atraccionIdeologica + atraccionGenetica)

    def getDeseabilidadAbsoluta(self, lista : list[Character], fenotipo = '', min : float = 0):
        
        CLASES_SOCIALES_PONDERACION : dict = {
            "Noble" : 3,
            "Alta" : 2,
            "Media" : 1,
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
                        genTot += 0.5 if expresion == "incompleto" else genomaDatabase[especie][part][alelo]["dominancia"]

            GEN = genTot / genCount

            # clase social (3 nobleza, 2 clase alta, 1 clase media, 0.5 clase pobre, 0.25 paria) CLASE
            CLASE = float(CLASES_SOCIALES_PONDERACION[c.getClaseSocial()])

            # productorio de facetas relevantes normalizadas entre 0.5 y 2
            FACETA = 1
            for facet in list(FACETAS_RELEVANTES_PONDERACION.keys()) :
                FACETA *= (float(personalidad["facetas"][facet]) * FACETAS_RELEVANTES_PONDERACION[facet] + 50)*1.5/100 + 0.5

            # productorio de opiniones relevantes normalizadas entre 0.5 y 2
            OPINION = 1
            for opinion in list(OPINIONES_RELEVANTES_PONDERACION.keys()) :
                OPINION *= (float(personalidad["opiniones"][opinion]) * OPINIONES_RELEVANTES_PONDERACION[opinion] + 50)*1.5/100 + 0.5

            formula = GEN * CLASE * (FACETA + OPINION)
            if formula > min:
                listaDeseabilidades[c.getNombreId()] = formula

        #con la deseabilidad absoluta evaluada se hace un mapa de personaje/deseabilidad y se ordena
        listaDeseabilidades = dict(sorted(listaDeseabilidades.items(), key=lambda item: item[1]))

        return listaDeseabilidades

    def getPoblacionValida(self, year, sexoCandidato = "") -> tuple[list[Character], list[int]]:

        EDAD_FERTILIDAD_HOMBRE = 16
        EDAD_FERTILIDAD_MUJER = 14
        EDAD_MENOPAUSIA = 40

        poblacionValida : list[Character] = []
        newPoblacion : list[int] = []
        
        for it in self.poblacion_:
            c = Character({}, f"personaje_{it}.json")
            fechaMuerte = c.getMuerte()
            if fechaMuerte <= year:
                continue

            fechaNacimiento = c.getNacimiento()
            edad = year - fechaNacimiento
            sexo = c.getSexo() == "Hombre"
            minFertilidad = fechaNacimiento + (EDAD_FERTILIDAD_HOMBRE if sexo else EDAD_FERTILIDAD_MUJER)
            maxFertilidad = fechaNacimiento + (fechaMuerte if sexo else EDAD_MENOPAUSIA)
            if year < minFertilidad or year >= maxFertilidad:
                continue

            if sexoCandidato != "" and sexoCandidato == sexo:
                continue

            if c.hasSpouse():
                continue

            poblacionValida.append(c)
            newPoblacion.append(it)

        return (poblacionValida, newPoblacion)