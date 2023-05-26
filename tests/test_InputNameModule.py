from .context import chargen

from random import randint
import unittest

def getInputNameModule():
    """Returns an InputNameModule."""
    moduleFactory = chargen.ModuleFactory()
    return moduleFactory.buildModule("InputNameModule")

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
        self.assertEqual(inputNameModule.getParams(), {"name" : None})

    def test_getDependencies_default_getsDependencies(self):
        """Tests that the default value for getDependencies is correct."""
        inputNameModule = getInputNameModule()
        self.assertEqual(inputNameModule.getDependencies(), [])
        

if __name__ == '__main__':
    unittest.main()
