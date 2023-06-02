from chargen.domain.Character import Character
from chargen.application.modules.INameModule import INameModule
from .context import chargen

from random import randint
import unittest

def getInputNameModule():
    """Returns an InputNameModule."""
    moduleFactory = chargen.ModuleFactory()
    return moduleFactory.buildModule("InputNameModule")

def getMockInputNameModule():
    """Returns a mock InputNameModule."""
    inputNameModule = getInputNameModule()
    inputNameModule.setParams({"name": "John"})
    return inputNameModule

def getMockCharacter():
    """Returns a mock Character that has an InputNameModule module."""
    character = Character()
    character.modules_ = {}
    character.modules_["InputNameModule"] = getMockInputNameModule().__dict__()
    return character

class InputNameModuleTest(unittest.TestCase):
    """CharacterBuilder test cases."""

    def test_getFieldInterface_default_getsFieldInterface(self):
        """Tests that the default value for getFieldInterface is correct."""
        inputNameModule = getInputNameModule()
        self.assertEqual(inputNameModule.getFieldInterface(), "INameModule")

    def test_getInstanceType_default_getsInstanceType(self):
        """Tests that the default value for getInstanceType is correct."""
        inputNameModule = getInputNameModule()
        self.assertEqual(inputNameModule.getInstanceType(), "InputNameModule")

    def test_getParams_default_getsParams(self):
        """Tests that the default value for getParams is correct."""
        inputNameModule = getInputNameModule()
        inputNameModule.setParams({"name": "John"})
        self.assertEqual(inputNameModule.getParams(), {"name" : "John"})

    def test_getDependencies_default_getsDependencies(self):
        """Tests that the default value for getDependencies is correct."""
        inputNameModule = getInputNameModule()
        self.assertEqual(inputNameModule.getDependencies(), [])

    def test_setParams_default_setsParams(self):
        """Tests that the default value for setParams is correct."""
        inputNameModule = getInputNameModule()
        inputNameModule.setParams({"name": "John"})
        self.assertEqual(inputNameModule.getParams(), {"name": "John"})

    def test_setParams_overwritesOnlyIncludedParams(self):
        """Tests that setParams overwrites only the params included in the passed params dict."""
        inputNameModule = getInputNameModule()
        inputNameModule.setParams({"name": "John", "age": 30})
        self.assertEqual(inputNameModule.getParams(), {"name": "John", "age": 30})
        inputNameModule.setParams({"name": "Jane"})
        self.assertEqual(inputNameModule.getParams(), {"name": "Jane", "age": 30})

    def test_dict_default_returnsDict(self):
        """Tests that the default value for __dict__ is correct."""
        inputNameModule = getInputNameModule()
        inputNameModule.setParams({"name": "John"})
        self.assertEqual(inputNameModule.__dict__(), {"fieldInterface_" : "INameModule", "instanceType_" : "InputNameModule", "params_" : {"name": "John"}, "dependencies" : []})
        
    def test_resolve_default_resolvesModule(self):
        """Tests that the default value for resolve is correct."""
        inputNameModule = getMockInputNameModule()
        character = getMockCharacter()
        self.assertTrue(inputNameModule.resolve(character))

    def test_getName_default_returnsName(self):
        """Tests that the default value for getName is correct."""
        inputNameModule : INameModule = getMockInputNameModule() # type: ignore
        self.assertEqual(inputNameModule.getName(), "John")

if __name__ == '__main__':
    unittest.main()
