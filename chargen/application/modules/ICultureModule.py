from abc import abstractmethod
from .IModule import IModule

class ICultureModule(IModule):
    """Interface for the culture modules."""

    fieldInterface_ : str = "ICultureModule"

    @abstractmethod
    def getCulture(self) -> str:
        """Returns the culture name."""
        pass

    @abstractmethod
    def generateName(self) -> str:
        """Generates a name."""
        pass
