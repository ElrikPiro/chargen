from chargen import Character
from chargen import DBAdapterFactory
from chargen import IDBAdapter
from datetime import datetime

class CharacterBuilder:
    
    settings_ : dict
    database_ : IDBAdapter

    def __init__(self, characterId : str = None, persistence : str = "tinyDB", modules : dict = {}) -> None:

        if characterId is None:
            """If no characterId is provided, use the current time as the characterId"""
            characterId = datetime.now().strftime("%Y%m%d%H%M%S")

        self.settings_ = {"persistence" : persistence, "characterId" : characterId, "modules" : modules}
        self.database_ = DBAdapterFactory.createAdapter(
            {
                "type" : self.settings_["persistence"],
                "path" : "characters.json"
            }
        )
        self.database_.connect()
        pass

    def build(self) -> Character:
        return Character()
    
