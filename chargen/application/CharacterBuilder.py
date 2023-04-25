from chargen import Character
from datetime import datetime

class CharacterBuilder:
    
    settings_ : dict

    def __init__(self, characterId : str = None, persistence : str = "tinyDB", modules : dict = {}) -> None:

        if characterId is None:
            """If no characterId is provided, use the current time as the characterId"""
            characterId = datetime.now().strftime("%Y%m%d%H%M%S")

        self.settings_ = {"persistence" : persistence, "characterId" : characterId, "modules" : modules}
        pass

    def build(self) -> Character:
        return Character()
    
