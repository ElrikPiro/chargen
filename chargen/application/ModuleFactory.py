"""Factory for creating modules."""

from chargen import IModule
from chargen import IModuleMockups as mockup
from chargen.test.MockupCulture import MockupCulture
from chargen.application.modules.InputNameModule import InputNameModule
from chargen.application.modules.CulturalNameModule import CulturalNameModule

class ModuleFactory:

    def __init__(self):
        """Initializes the factory."""
        self.moduleTypes_ = {
            "MockModuleStatic": mockup.MockModuleStatic,
            "MockModuleDynamic": mockup.MockModuleDynamic,
            "MockSubmodule": mockup.MockSubmodule,
            "MockupCulture": MockupCulture,
            "InputNameModule": InputNameModule,
            "CulturalNameModule": CulturalNameModule,
        }

    def buildModule(self, module : str) -> IModule:
        """Builds a module."""
        return self.moduleTypes_[module]()

    pass
