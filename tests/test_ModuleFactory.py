# -*- coding: utf-8 -*-

from .context import chargen
from chargen import ModuleFactory
import unittest


class ModuleFactoryTest(unittest.TestCase):
    """Test the ModuleFactory class."""

    def test_constructor_noExcept(self):
        """Test that the constructor does not throw an exception."""
        factory = chargen.ModuleFactory()


if __name__ == '__main__':
    unittest.main()