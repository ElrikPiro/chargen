from chargen import IModule

class MockModuleStatic(IModule):

    mockKey_ : str = None
    cached_ : str = None

    def getFieldInterface(self):
        """Returns the field interface."""
        pass

    def getInstanceType(self):
        """Returns the instance type."""
        return "MockModuleStatic"

    def getParams(self):
        """Returns the params."""
        pass

    def getDependencies(self):
        """Returns the dependencies."""
        pass

    def setParams(self, params):
        """Sets the params."""
        pass

    def __dict__(self) -> dict:
        """Returns the dictionary representation of the module."""
        return {"mockKey" : self.mockKey_, "cached" : self.cached_}

    def resolve(self, character):
        params = character.modules_[self.getInstanceType()]
        
        try:
            self.mockKey_ = params["mockKey"]
            self.cached_ = params["mockKey"]
        except KeyError:
            return False

        character.modules_[self.getInstanceType()] = self.__dict__()

        return True

    pass