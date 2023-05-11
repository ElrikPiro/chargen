"""Interface for the modules."""

from abc import ABC, abstractmethod
from chargen.domain.Character import Character

class IModule(ABC):
    """Interface for the modules."""

    fieldInterface_ : str = None
    instanceType_ : str = None
    params_ : dict = None
    dependencies : list[str] = None

    @abstractmethod
    def getFieldInterface(self) -> str:
        """Returns the field interface."""
        pass

    @abstractmethod
    def getInstanceType(self) -> str:
        """Returns the instance type."""
        pass

    @abstractmethod
    def getParams(self) -> dict:
        """Returns the params."""
        pass

    @abstractmethod
    def getDependencies(self) -> list[str]:
        """Returns the dependencies."""
        pass

    @abstractmethod
    def setParams(self, params) -> None:
        """Sets the params."""
        pass

    @abstractmethod
    def __dict__(self) -> dict:
        """Returns the dictionary representation of the module."""
        pass

    @abstractmethod
    def resolve(self, character : Character) -> bool:
        """Resolves the module."""
        pass