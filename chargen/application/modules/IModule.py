"""Interface for the modules."""

from abc import ABC, abstractmethod

class IModule(ABC):
    """Interface for the modules."""

    fieldInterface_ : str = None
    instanceType_ : str = None
    params_ : dict = None
    dependencies : list[str] = None

    @abstractmethod
    def getFieldInterface(self):
        """Returns the field interface."""
        pass

    @abstractmethod
    def getInstanceType(self):
        """Returns the instance type."""
        pass

    @abstractmethod
    def getParams(self):
        """Returns the params."""
        pass

    @abstractmethod
    def getDependencies(self):
        """Returns the dependencies."""
        pass

    @abstractmethod
    def setParams(self, params):
        """Sets the params."""
        pass

    @abstractmethod
    def __dict__(self):
        """Returns the dictionary representation of the module."""
        pass

    @abstractmethod
    def resolve(self, character):
        """Resolves the module."""
        pass