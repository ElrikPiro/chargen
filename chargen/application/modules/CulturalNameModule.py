"""
CulturalNameModule module: contains the CulturalNameModule class that implements the INameModule interface.
"""

from chargen import INameModule
from chargen import Character
from chargen import ICultureModule

class CulturalNameModule(INameModule):
    """Interface for the name generating modules."""

    fieldInterface_ : str = "INameModule"
    instanceType_ : str = "CulturalNameModule"
    params_ : dict = {"name" : None}
    dependencies : list[str] = ["ICultureModule"]

    def getFieldInterface(self) -> str:
        return self.fieldInterface_

    def getInstanceType(self) -> str:
        return self.instanceType_

    def getParams(self) -> dict:
        return self.params_

    def getDependencies(self) -> list[str]:
        return self.dependencies

    def setParams(self, params) -> None:
        """Overwrites only the params included in the passed params dict."""
        for key in params:
            self.params_[key] = params[key]

    def __dict__(self) -> dict:
        return {"fieldInterface_" : self.fieldInterface_, "instanceType_" : self.instanceType_, "params_" : self.params_, "dependencies" : self.dependencies}

    def resolve(self, character : Character) -> bool:
        """Resolves the module."""
        from ..ModuleFactory import ModuleFactory
        factory : ModuleFactory = ModuleFactory()
        
        module : dict = character.modules_[self.getInstanceType()]
        params : dict = module.get("params_", {})

        cultureModule : ICultureModule = None
        for module in character.modules_.keys():
            if module.get("fieldInterface_", "") == "ICultureModule":
                cultureModule : ICultureModule = factory.buildModule(module.get("instanceType_", None))
                break

        if cultureModule == None:
            return False

        if params["name"] == None or params["name"] == "":
            self.setParams({"name" : cultureModule.generateName()})
        
        if params["name"] != None and params["name"] != "" and isinstance(params["name"], str):
            character.modules_[self.getInstanceType()] = self.__dict__()
            return True
        else:
            return False

    def getName(self) -> str:
        return self.params_["name"]
