from chargen import Character
from chargen import DBAdapterFactory
from chargen import IDBAdapter
from datetime import datetime

class CharacterBuilder:
    
    settings_ : dict
    database_ : IDBAdapter
    character_ : Character

    def __init__(self, characterId : str = None, persistence : str = "tinyDB", modules : dict = {}) -> None:

        if characterId is None:
            """If no characterId is provided, use the current time as the characterId"""
            characterId = datetime.now().strftime("%Y%m%d%H%M%S")

        if modules is None:
            """If no modules are provided, use an empty dictionary"""
            modules = {}

        self.settings_ = {"persistence" : persistence, "characterId" : characterId, "modules" : modules}
        self.database_ = DBAdapterFactory.createAdapter(
            {
                "type" : self.settings_["persistence"],
                "path" : "characters.json"
            }
        )
        self.database_.connect()
        pass

    def get(self) -> Character:
        self.character_ = Character(self.settings_["characterId"], self.settings_["modules"])
        return self.character_
    
    def save(self):
        self.database_.query({"queryType" : "insert", "query" : self.character_.__dict__})
        pass

    def build(self) -> tuple[bool, str]:
        """Instantiates all the modules and tries to resolve them"""
        return [False, "Not implemented"]
    
