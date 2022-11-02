# -*- coding: utf-8 -*-

from context import chargen
import unittest

class exampleTest(unittest.TestCase):

    def test_suite(self):
        print("run suite")
        self.assertIsNone(None)

if __name__ == '__main__':
    unittest.main()
