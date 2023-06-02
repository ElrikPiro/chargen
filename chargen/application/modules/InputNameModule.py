from chargen import INameModule
from chargen import Character

class InputNameModule(INameModule):
    """Interface for the name generating modules."""

    fieldInterface_ : str = "INameModule"
    instanceType_ : str = "InputNameModule"
    params_ : dict = {"name" : None}
    dependencies : list[str] = []

    def getFieldInterface(self) -> str:
        return self.fieldInterface_

    def getInstanceType(self) -> str:
        return "InputNameModule"

    def getParams(self) -> dict:
        return self.params_

    def getDependencies(self) -> list[str]:
        return []

    def setParams(self, params) -> None:
        """Overwrites only the params included in the passed params dict."""
        for key in params:
            self.params_[key] = params[key]

    def __dict__(self) -> dict:
        return {"fieldInterface_" : self.fieldInterface_, "instanceType_" : self.instanceType_, "params_" : self.params_, "dependencies" : self.dependencies}

    def resolve(self, character : Character) -> bool:
        """Resolves the module."""
        module : dict = character.modules_[self.getInstanceType()]
        params : dict = module.get("params_", {})
        return params["name"] != None and params["name"] != "" and isinstance(params["name"], str)

    def getName(self) -> str:
        """Returns the name."""
        return self.params_["name"]