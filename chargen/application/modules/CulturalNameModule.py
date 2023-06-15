"""
CulturalNameModule module: contains the CulturalNameModule class that implements the INameModule interface.
"""

from chargen import INameModule
from chargen import Character
from chargen import ICultureModule
from chargen import CultureModuleFactory

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
        module : dict = character.modules_[self.getInstanceType()]
        params : dict = module.get("params_", {})

        cultureModule : ICultureModule = 
        for module in character.modules_.values():
            if module.__dict__().get("fieldInterface_", "") == "ICultureModule":
                cultureModule : ICultureModule = module
                break

        if cultureModule == None:
            return False

        if params["name"] == None or params["name"] == "":
            self.setParams({"name" : cultureModule.generateName()})

        return params["name"] != None and params["name"] != "" and isinstance(params["name"], str)

    def getName(self) -> str:
        return self.params_["name"]
