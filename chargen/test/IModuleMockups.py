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
        return True

    pass