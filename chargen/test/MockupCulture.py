import random
from chargen.application.modules.ICultureModule import ICultureModule
from chargen.domain.Character import Character

class MockupCulture(ICultureModule):
    """Mockup culture module."""

    instanceType_ = "MockupCulture"
    params_ = {}
    dependencies_ = []

    def __init__(self, name: str = "Mockup Culture"):
        """Initializes the mockup culture module."""
        self.name_ = name

    def getCulture(self) -> str:
        """Returns the culture name."""
        return self.name_

    def generateName(self) -> str:
        """Generates a name."""
        return random.choice(["John", "Jane", "Joe"])
    
    def getFieldInterface(self) -> str:
        """Returns the field interface."""
        return self.fieldInterface_

    def getInstanceType(self) -> str:
        """Returns the instance type."""
        return self.instanceType_

    def getParams(self) -> dict:
        """Returns the params."""
        return self.params_

    def getDependencies(self) -> list[str]:
        """Returns the dependencies."""
        return self.dependencies_

    def setParams(self, params) -> None:
        """Sets the params."""
        self.params_ = params

    def __dict__(self) -> dict:
        """Returns the dictionary representation of the module."""
        return {
            "fieldInterface_" : self.fieldInterface_,
            "instanceType_" : self.instanceType_,
            "params_" : self.params_,
            "dependencies_" : self.dependencies_
        }

    def resolve(self, character : Character) -> bool:
        """Resolves the module."""
        character.modules_[self.getInstanceType()] = self.__dict__()
        return True
