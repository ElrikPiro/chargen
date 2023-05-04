"""Factory for creating modules."""

from chargen import IModule

class MockModuleStatic(IModule):
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
