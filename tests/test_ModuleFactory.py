# -*- coding: utf-8 -*-

from .context import chargen
from chargen import ModuleFactory
import unittest


class ModuleFactoryTest(unittest.TestCase):
    """Test the ModuleFactory class."""

    def test_constructor_noExcept(self):
        """Test that the constructor does not throw an exception."""
        factory = chargen.ModuleFactory()

    def test_buildModule_noExcept(self):
        """Test that the buildModule function does not throw an exception."""
        factory = chargen.ModuleFactory()
        factory.buildModule("MockModuleStatic")

    def test_buildModuleModuleDoesntExist_exception(self):
        """Test that the buildModule function throws an exception when the module doesn't exist."""
        factory = chargen.ModuleFactory()
        with self.assertRaises(KeyError):
            factory.buildModule("DoesntExist")

    def test_buildInputNameModule_noExcept(self):
        """Test that the buildInputNameModule function does not throw an exception."""
        factory = chargen.ModuleFactory()
        factory.buildModule("InputNameModule")


if __name__ == '__main__':
    unittest.main()