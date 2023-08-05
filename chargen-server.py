from fastapi import FastAPI
from chargen import CharacterBuilder

app = FastAPI()

characterBuilder = CharacterBuilder()

def formatCharacter(character):

    id = character.id_
    modules_ = character.modules_
    modules = {}
    for moduleKey in modules_.keys():
        module = modules_[moduleKey]
        modules[moduleKey] = module

    return {
        "id_" : id,
        "modules": modules,
    }

@app.get("/getCharacter")
async def getCharacter(id_ : str):
    try:
        characterBuilder = CharacterBuilder(id_)
        return formatCharacter(characterBuilder.get())
    except Exception as e:
        #raise e
        return {"error" : str(e)}
    
@app.post("/buildCharacter")
async def buildCharacter(character : dict):
    try:
        characterBuilder = CharacterBuilder(characterId=character["id_"], modules=character["modules"])
        BuildResult = characterBuilder.build()
        characterBuilder.save()
        if not BuildResult[0]:
            raise Exception(BuildResult[1])
        return formatCharacter(characterBuilder.get())
    except Exception as e:
        #raise e
        return {"error" : str(e)}
    
@app.get("/getCharacterIds")
async def getCharacterIds():
    try:
        return characterBuilder.getCharacterIds()
    except Exception as e:
        #raise e
        return {"error" : str(e)}