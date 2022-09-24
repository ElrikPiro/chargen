# -*- coding: utf-8 -*-

from context import chargen
import unittest

class casamenteraTest(unittest.TestCase):

    def test_suite(self):
        casametera = chargen.casamentera.Casamentera([2,3,4], 1991, 2091, False)
        self.assertIsNotNone(casametera)

if __name__ == '__main__':
    unittest.main()