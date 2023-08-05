from chargen import Character
from chargen import DBAdapterFactory
from chargen import IDBAdapter
from chargen import ModuleFactory
from chargen import IModule
from datetime import datetime

class CharacterBuilder:
    
    settings_ : dict
    database_ : IDBAdapter
    character_ : Character = None

    def __init__(self, characterId : str = None, persistence : str = "tinyDB", modules : dict = {}) -> None:

        readDb : bool = False
        if characterId is None:
            """If no characterId is provided, use the current time as the characterId"""
            characterId = datetime.now().strftime("%Y%m%d%H%M%S")
        else:
            """If a characterId is provided, use it"""
            readDb = True
            

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
        queryResult = self.database_.query({"queryType" : "get", "query" : {"id_" : characterId}})
        if readDb and queryResult is not None:
            self.settings_["modules"] = queryResult["modules_"]
            self.character_ = Character(
                self.settings_["characterId"], 
                self.settings_["modules"]
            )
        pass

    def get(self) -> Character:
        if self.character_ is None:
            self.character_ = Character(self.settings_["characterId"], self.settings_["modules"])
        return self.character_
    
    def save(self):
        self.database_.query({"queryType" : "insert", "query" : {"id_" : self.character_.id_, "modules_" : self.character_.modules_}}) 
        pass

    def build(self) -> tuple[bool, str]:
        """Instantiates all the modules and tries to resolve them"""
        factory = ModuleFactory()
        character = self.get()
        self.character_.id_ = character.id_
        retval : bool = True
        comment : str = ""

        for moduleKey in character.modules_:
            module : IModule = factory.buildModule(moduleKey)
            module.setParams(character.modules_[moduleKey])
            """If the module has dependencies, resolve them first"""
            for dependency in module.getDependencies():
                compliantDependencies = list(filter(lambda x : factory.buildModule(x).getFieldInterface() == dependency, character.modules_.keys()))
                
                if len(compliantDependencies) == 0:
                    """Fail fast"""
                    comment += f"Module {moduleKey} has dependency {dependency} which is not present in the character\n"
                    retval = False
                else:
                    """If the character has the dependency, resolve it first"""
                    dependencyModule : IModule = factory.buildModule(compliantDependencies.pop())
                    dependencyModule.setParams(character.modules_[dependencyModule.getInstanceType()])
                    resolved = dependencyModule.resolve(character)
                    if not resolved:
                        comment += f"Module {dependency} could not be resolved\n"
                    retval = retval and resolved

            resolved = module.resolve(character)
            if not resolved:
                comment += f"Module {moduleKey} could not be resolved\n"
            self.character_.modules_[moduleKey].update(module.getParams())
            retval = retval and resolved

        return [retval, comment]
    
    def getCharacterIds(self) -> list[str]:
        return self.database_.query({"queryType" : "getAllIds", "query" : {}})
    
