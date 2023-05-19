"""Factory for creating modules."""

from chargen import IModule
from chargen import IModuleMockups as mockup

class ModuleFactory:

    def __init__(self):
        """Initializes the factory."""
        self.moduleTypes_ = {
            "MockModuleStatic": mockup.MockModuleStatic,
            "MockModuleDynamic": mockup.MockModuleDynamic,
            "MockSubmodule": mockup.MockSubmodule,
        }

    def buildModule(self, module : str) -> IModule:
        """Builds a module."""
        return self.moduleTypes_[module]()

    pass
