from chargen import IModule

def check(mock):
    try:
        if mock["cached"] == None:
            return False
        else:
            return True
    except KeyError:
        return False

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
    
    mockKey_ : str = None
    mockValue_ : int = None
    cached_ : str = None

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
    
    def __dict__(self) -> dict:
        """Returns the dictionary representation of the module."""
        return {"cached" : self.cached_, "mockValue" : self.mockValue_}

    def resolve(self, character):
            params = character.modules_[self.getInstanceType()]
            
            try:
                mockStatic = character.modules_["MockModuleStatic"]
                if not check(mockStatic):
                    auxMockStatic = MockModuleStatic()
                    if not auxMockStatic.resolve(character):
                        return False
                    else:
                        character.modules_["MockModuleStatic"] = auxMockStatic.__dict__()
                mockStaticValue = character.modules_["MockModuleStatic"]["cached"]    

                mockDynamic = character.modules_["MockModuleDynamic"]
                if not check(mockDynamic):
                    auxMockDynamic = MockModuleDynamic()
                    if not auxMockDynamic.resolve(character):
                        return False
                    else:
                        character.modules_["MockModuleDynamic"] = auxMockDynamic.__dict__()
                mockDynamicValue = character.modules_["MockModuleDynamic"]["cached"]

                self.mockValue_ = character.modules_["MockSubmodule"]["mockValue"]
                self.cached_ = mockStaticValue + mockDynamicValue + self.mockValue_
                
            except KeyError:
                return False
    
            character.modules_[self.getInstanceType()] = self.__dict__()
    
            return True