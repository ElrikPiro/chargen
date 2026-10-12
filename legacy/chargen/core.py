# -*- coding: utf-8 -*-
import json
import os
import requests


class JsonStore:
    def __init__(self, root="json"):
        self.root = root

    def _resolve(self, jsonRef):
        return os.path.join(self.root, jsonRef)

    def load(self, jsonRef):
        with open(self._resolve(jsonRef)) as jsonFile:
            return json.load(jsonFile)

    def write(self, jsonRef, jsonData):
        path = self._resolve(jsonRef)
        parent = os.path.dirname(path)
        if parent:
            os.makedirs(parent, exist_ok=True)
        with open(path, "w") as jsonFile:
            json.dump(jsonData, jsonFile, indent=4)


DEFAULT_JSON_STORE = JsonStore()


def getJsonFromUrl(url: str) -> dict:
    response = requests.get(url)
    return response.json()


def loadJson(jsonRef, store=None) -> dict:
    io = DEFAULT_JSON_STORE if store is None else store
    return io.load(jsonRef)


def writeJson(jsonRef, jsonData, store=None):
    io = DEFAULT_JSON_STORE if store is None else store
    io.write(jsonRef, jsonData)


def generateNewNameId(store=None):
    path = "config/nombresPropios.json"
    nameDir = loadJson(path, store)
    nameId = len(nameDir) + 1
    nameDir[nameId] = "PLACEHOLDER"
    writeJson(path, nameDir, store)
    return nameId


def generateNewFamilyId(store=None):
    path = "config/familias.json"
    nameDir = loadJson(path, store)
    nameId = "PLACEHOLDER_{}".format(len(nameDir) + 1)
    nameDir[nameId] = "PLACEHOLDER"
    writeJson(path, nameDir, store)
    return nameId


def generateNewLugar(store=None):
    path = "config/localizaciones.json"
    nameDir = loadJson(path, store)
    nameId = "PLACEHOLDER_{}".format(len(nameDir) + 1)
    nameDir[nameId] = {"nombre": "PLACEHOLDER"}
    writeJson(path, nameDir, store)
    return nameId


def resetPlaceHolder(config: str, key: str, value, isLugar: bool = False, isFamilia: bool = False, store=None):
    path = config
    nameDir = loadJson(path, store)
    
    if isLugar:
        nombre = value["nombre"]
        #checks if value is already in the dictionary
        if not (nombre in nameDir):
            nameDir[nombre] = {
                "nombre": nombre,
                "tipo": "Indeterminado",
                "descripcion": "Indeterminado",
                "enlaces" : {
                    nombre : 0.1,
                }
            }

        nameDir[key] = {"nombre": nombre}
        writeJson(path, nameDir, store)
    elif isFamilia:
        nameDir[key] = value
        nameDir[value] = value
        writeJson(path, nameDir, store)
    else:
        nameDir[key] = value
        writeJson(path, nameDir, store)

class RelationType:
    NONE = 0
    PARENT = 1
    SPOUSE = 2
    DESCENDANT = 3
