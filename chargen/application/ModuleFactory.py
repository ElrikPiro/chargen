"""Factory for creating modules."""

from chargen import IModule

class MockModuleStatic(IModule):
    def getFieldInterface(self):
        """Returns the field interface."""
        pass

    def getInstanceType(self):
        """Returns the instance type."""
        pass

    def getParams(self):
        """Returns the params."""
        pass

    def getDependencies(self):
        """Returns the dependencies."""
        pass

    def setParams(self, params):
        """Sets the params."""
        pass

    def __dict__(self):
        """Returns the dictionary representation of the module."""
        pass

    def resolve(self, character):
        """Resolves the module."""
        pass
    pass

class ModuleFactory:

    def __init__(self):
        """Initializes the factory."""
        self.moduleTypes_ = {
            "MockModuleStatic": MockModuleStatic,
        }

    def buildModule(self, module : str):
        """Builds a module."""
        return self.moduleTypes_[module]()

    pass
