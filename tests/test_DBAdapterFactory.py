# -*- coding: utf-8 -*-

from .context import chargen
from chargen import DBAdapterFactory
import unittest

class DBAdapterFactoryTest(unittest.TestCase):
    """TinyDBAdapter test cases."""

    def test_createAdapterTinyDB_returnsTinyDBAdapter(self):
        """Test that the db attribute is not None after a successful connect call."""
        databaseId = {"type" : "tinyDB", "path" : "test.json"}
        dbAdapter = DBAdapterFactory.createAdapter(databaseId)
        self.assertEqual(dbAdapter.__class__.__name__, "TinyDBAdapter")
        pass

    def test_createAdapterInvalid_exception(self):
        """Test on an invalid adapter type, an exception is thrown."""
        databaseId = {"type" : "invalid", "path" : "test.json"}
        with self.assertRaises(Exception):
            dbAdapter = DBAdapterFactory.createAdapter(databaseId)
        pass

if __name__ == '__main__':
    unittest.main()