from fastapi import FastAPI
from chargen import CharacterBuilder

app = FastAPI()

characterBuilder = CharacterBuilder()

@app.get("/getCharacter")
async def getCharacter(id_ : str):
    try:
        characterBuilder = CharacterBuilder(id_)
        return characterBuilder.get()
    except Exception as e:
        raise e
        return {"error" : str(e)}
    
@app.post("/buildCharacter")
async def buildCharacter(character : dict):
    try:
        characterBuilder = CharacterBuilder(characterId=character["id_"], modules=character["modules"])
        BuildResult = characterBuilder.build()
        characterBuilder.save()
        if not BuildResult[0]:
            raise Exception(BuildResult[1])
        return {"character" : characterBuilder.get()}
    except Exception as e:
        raise e
        return {"error" : str(e)}