from typing import Type
from .ICultureModule import ICultureModule
from importlib import import_module

class CultureModuleFactory:
    """Factory for creating culture modules."""

    @staticmethod
    def createCultureModule(moduleName: str) -> ICultureModule:
        """Creates a culture module of the specified name."""
        module = import_module(moduleName)
        for name in dir(module):
            obj = getattr(module, name)
            try:
                if issubclass(obj, ICultureModule) and obj is not ICultureModule:
                    return obj()
            except TypeError:
                pass
        raise ValueError(f"No ICultureModule found in module {moduleName}")