from . import character

class Casamentera:
    
    poblacion_ : list[int]
    year_ : int

    def __init__(self, poblacion : list[int], year : int):
        self.poblacion_ = poblacion
        self.year_ = year

    def iterar(self):
        #Validar toda la población en el año actual
        
        #Evaluar la deseabilidad absoluta:
            # numero de genes recesivos vs dominantes expresados (media del total) GEN
            # clase social (3 nobleza, 2 clase alta, 1 clase media, 0.5 clase pobre, 0.25 paria) CLASE
            # productorio de facetas relevantes normalizadas entre 0.5 y 2
            # productorio de opiniones relevantes normalizadas entre 0.5 y 2

        #con la deseabilidad absoluta evaluada se hace un mapa de personaje/deseabilidad y se ordena
        
        #para cada personaje en la lista de deseabilidad
            #   se calcula la fecha de matrimonio
            #   se hace una lista similar para esa fecha, pero descartando al mismo sexo 
            # y a aquellos con una deseabilidad por debajo de la mitad de la del individuo evaluado
            #   se ordena aleatoriamente la lista y se evalua la deseabilidad relativa de 
            # los primeros len(lista)/e quedandonos con el maximo valor.
            # 
            #   Ese será el candidato escogido, se casan, se añaden los hijos a la poblacion
            # y se elimina a ambos conyugues de la lista de candidatos
            #   NOTA:
            #   como evaluar la deseabilidad relativa
                #   Se buscará que expresen el mismo nivel de recisividad en sus genes (1), mejor recisividad (2) o peor (0.5)
                #   Se buscará que las opiniones y las facetas sean coincidentes
                #   Se asignará un valor entre 0.5 y 2 según la distancia, teniendo en cuenta la distancia máxima y la mínima
                # de los candidatos.
                #   Se evaluará la consanguinidad: 0.25 hermanos, 0.5 padres e hijos, 0.75 abuelos, tios, primos y nietos
        
        #   desde el año actual+1 hasta 10 años en adelante se busca la fecha con el máximo
        # de población válida y se designa ese año como año actual
        pass