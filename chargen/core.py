# -*- coding: utf-8 -*-
from . import helpers
import json

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
    nameId = len(nameDir) + 1
    nameDir[nameId] = "PLACEHOLDER"
    writeJson(path, nameDir)
    return nameId

def generateNewLugar():
    path = "config/localizaciones.json"
    nameDir = loadJson(path)
    nameId = len(nameDir) + 1
    nameDir[nameId] = {"nombre" : "PLACEHOLDER", "xyzw" : [0,0,0,0]}
    writeJson(path, nameDir)
    return nameId


class RelationType:
    NONE = 0
    MOTHER = 1
    SPOUSE = 2
    DESCENDANT = 3
    GOD = 4