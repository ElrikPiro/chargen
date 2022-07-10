import dijkstra

# grafo modelos de localizaciones
def mockup_localizacion():
    return {
        "nombre": "Centro del mundo",
        "tipo": "Ciudad",
        "descripcion": "La ciudad en el centro del mundo",
        "enlaces" : {#conexiones directas, unidireccionales
            # str(nombre) : int(dias de viaje)
        }
    }

class LocalizacionesNodo():
    
    nombre : str
    tipo : str
    descripcion : str
    enlaces : dict
    
    def __init__(self, localizacion : dict):
        self.nombre = localizacion["nombre"]
        self.tipo = localizacion.get("tipo", "")
        self.descripcion = localizacion.get("descripcion", "")
        self.enlaces = localizacion.get("enlaces", {})

    def toDict(self):
        return {
            "nombre": self.nombre,
            "tipo": self.tipo,
            "descripcion": self.descripcion,
            "enlaces": self.enlaces
        }

    def addEnlace(self, nodo, dias : float):
        self.enlaces[nodo.nombre] = dias

    def getEnlaces(self) -> dict:
        return self.enlaces

class GrafoLocalizaciones():

    nodos : dict

    def __init__(self):
        self.nodos = {}

    def setNodo(self, nodo : LocalizacionesNodo):
        self.nodos[nodo.nombre] = nodo

    def getNodo(self, nombre : str) -> LocalizacionesNodo:
        return self.nodos[nombre]

    def getShortestPath(self, origen : str, destino : str) -> list:
        grafo = dijkstra.Graph()

        for nodo in self.nodos.values():
            nodo : LocalizacionesNodo = nodo
            origin = nodo.nombre
            for destination in nodo.getEnlaces().keys():
                grafo.add_edge(origin, destination, nodo.getEnlaces()[destination])

        spf = dijkstra.DijkstraSPF(grafo, origen)
        return [spf.get_path(destino), spf.get_distance(destino)]