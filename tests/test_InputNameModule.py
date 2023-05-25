from .context import chargen

from random import randint
import unittest

class InputNameModuleTest(unittest.TestCase):
    """CharacterBuilder test cases."""

    def test_getFieldInterface_default_getsFieldInterface(self):
        """Tests that the default value for getFieldInterface is correct."""
        # module = chargen.InputNameModule()
        # self.assertEqual(module.getFieldInterface(), "INameModule")
        

if __name__ == '__main__':
    unittest.main()
