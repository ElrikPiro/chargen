from chargen import INameModule
from chargen import Character

class InputNameModule(INameModule):
    """Interface for the name generating modules."""

    fieldInterface_ : str = None
    instanceType_ : str = None
    params_ : dict = None
    dependencies : list[str] = None

    name_ : str = None

    # def getFieldInterface(self) -> str:
    #     return "INameModule"

    # def getInstanceType(self) -> str:
    #     return "InputNameModule"

    # def getParams(self) -> dict:
    #     return {"name" : self.name_}

    # def getDependencies(self) -> list[str]:
    #     return []

    # def setParams(self, params) -> None:
    #     self.name_ = params["name"]
    #     pass

    # def __dict__(self) -> dict:
    #     return {"fieldInterface_" : self.fieldInterface_, "instanceType_" : self.instanceType_, "params_" : self.params_, "dependencies" : self.dependencies}

    # def resolve(self, character : Character) -> bool:
    #     """Resolves the module."""
    #     module : INameModule = character.modules_[self.getInstanceType()]
    #     pass

    # def getName(self) -> str:
    #     """Returns the name."""
    #     pass
