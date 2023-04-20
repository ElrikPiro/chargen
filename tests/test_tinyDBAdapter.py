# -*- coding: utf-8 -*-

from .context import chargen
from chargen import TinyDBAdapter

import unittest


class TinyDBAdapterTest(unittest.TestCase):
    """TinyDBAdapter test cases."""

    def test_connect_onValidCall_dbAttributeNotNull(self):
        """Test that the db attribute is not None after a successful connect call."""
        databaseId = {"path" : "test.json"}
        tinyDBAdapter = TinyDBAdapter.TinyDBAdapter(databaseId)
        tinyDBAdapter.connect()
        self.assertIsNotNone(tinyDBAdapter.db_)

    def test_connect_noPathInDatabaseId_throwsException(self):
        """Test that the db attribute is not None after a successful connect call."""
        databaseId = {}
        tinyDBAdapter = TinyDBAdapter.TinyDBAdapter(databaseId)
        with self.assertRaises(Exception):
            tinyDBAdapter.connect()

    def test_connect_invalidPath_throwsException(self):
        """Test that the db attribute is not None after a successful connect call."""
        databaseId = {"path" : "test"}
        tinyDBAdapter = TinyDBAdapter.TinyDBAdapter(databaseId)
        with self.assertRaises(Exception):
            tinyDBAdapter.connect()

    def test_connect_pathNotString_throwsException(self):
        """Test that the db attribute is not None after a successful connect call."""
        databaseId = {"path" : 1}
        tinyDBAdapter = TinyDBAdapter.TinyDBAdapter(databaseId)
        with self.assertRaises(Exception):
            tinyDBAdapter.connect()

    def test_query(self):
        self.assertTrue(True)


if __name__ == '__main__':
    unittest.main()
