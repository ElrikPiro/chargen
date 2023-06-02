"""Interface for the name generating modules."""

from chargen import IModule
from chargen import Character
from abc import ABC, abstractmethod

class INameModule(IModule):
    """Interface for the name generating modules."""

    fieldInterface_ : str = "INameModule"
    instanceType_ : str = None  # type: ignore
    params_ : dict = {"name" : None}
    dependencies : list[str] = []

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

    @abstractmethod
    def getName(self) -> str:
        """Returns the name."""
        pass
