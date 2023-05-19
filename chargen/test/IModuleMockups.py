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

class MockModuleDynamic(IModule):

    mockKey_ : str = None
    mockValue_ : int = None
    cached_ : str = None

    def getFieldInterface(self):
        """Returns the field interface."""
        pass

    def getInstanceType(self):
        """Returns the instance type."""
        return "MockModuleDynamic"

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
            self.mockValue_ = int(self.mockKey_)*2
            self.cached_ = self.mockValue_
        except KeyError:
            return False
        except ValueError:
            return False

        character.modules_[self.getInstanceType()] = self.__dict__()

        return True

pass

class MockSubmodule(IModule):
    """This mockup fails if the character has both MockModuleStatic and MockModuleDynamic."""
    
    def getFieldInterface(self):
        """Returns the field interface."""
        pass
    
    def getInstanceType(self):
        """Returns the instance type."""
        return "MockSubmodule"
    
    def getParams(self):
            """Returns the params."""
            pass
    
    def getDependencies(self):
            """Returns the dependencies."""
            return ["MockModuleStatic", "MockModuleDynamic"]
    
    def setParams(self, params):
            """Sets the params."""
            pass
    
    def resolve(self, character):
            params = character.modules_[self.getInstanceType()]
            
            try:
                mockStatic = character.modules_["MockModuleStatic"]
                mockDynamic = character.modules_["MockModuleDynamic"]
            except KeyError:
                return False
    
            character.modules_[self.getInstanceType()] = self.__dict__()
    
            return True