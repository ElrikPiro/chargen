# -*- coding: utf-8 -*-
from . import helpers
import json
import requests

def get_hmm():
    """Get a thought."""
    return 'hmmm...'


def hmm():
    """Contemplation..."""
    if helpers.get_answer():
        print(get_hmm())

def helloWorld():
    hmm()
    print("Uh, yes, hello world")

def getJsonFromUrl(url : str) -> dict:
    response = requests.get(url)
    return response.json()

def loadJson(jsonRef) -> dict:
    with open("json/"+jsonRef) as jsonFile:
        jsonObj = json.load(jsonFile)
        jsonFile.close()
    return jsonObj

def writeJson(jsonRef, jsonData):
    with open("json/"+jsonRef, 'w') as jsonFile:
        json.dump(jsonData, jsonFile, indent=4)
        jsonFile.close()

def generateNewNameId():
    path = "config/nombresPropios.json"
    nameDir = loadJson(path)
    nameId = len(nameDir) + 1
    nameDir[nameId] = "PLACEHOLDER"
    writeJson(path, nameDir)
    return nameId

def generateNewFamilyId():
    path = "config/familias.json"
    nameDir = loadJson(path)
    nameId = "PLACEHOLDER_{}".format(len(nameDir) + 1)
    nameDir[nameId] = "PLACEHOLDER"
    writeJson(path, nameDir)
    return nameId

def generateNewLugar():
    path = "config/localizaciones.json"
    nameDir = loadJson(path)
    nameId = "PLACEHOLDER_{}".format(len(nameDir) + 1)
    nameDir[nameId] = {"nombre" : "PLACEHOLDER"}
    writeJson(path, nameDir)
    return nameId

def resetPlaceHolder(config : str, key : int, value, isLugar : bool = False, isFamilia : bool = False):
    path = config
    nameDir = loadJson(path)
    
    if isLugar:
        nombre = value["nombre"]
        #checks if value is already in the dictionary
        if not (nombre in nameDir):
            nameDir[nombre] = {"nombre" : nombre}

        nameDir[key] = {"nombre" : nombre}
        writeJson(path, nameDir)
    elif isFamilia:
        nameDir[key] = value
        nameDir[value] = value
        writeJson(path, nameDir)
    else:
        nameDir[key] = value
        writeJson(path, nameDir)

class RelationType:
    NONE = 0
    PARENT = 1
    SPOUSE = 2
    DESCENDANT = 3
    GOD = 4