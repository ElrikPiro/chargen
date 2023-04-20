# -*- coding: utf-8 -*-

from .context import chargen
from chargen import TinyDBAdapter
from tinydb import TinyDB, Query

import unittest


class TinyDBAdapterTest(unittest.TestCase):
    """TinyDBAdapter test cases."""

    def test_connect_onValidCall_dbAttributeNotNull(self):
        """Test that the db attribute is not None after a successful connect call."""
        databaseId = {"path" : "test.json"}
        tinyDBAdapter = TinyDBAdapter(databaseId)
        tinyDBAdapter.connect()
        self.assertIsNotNone(tinyDBAdapter.db_)

    def test_connect_noPathInDatabaseId_throwsException(self):
        """Test that the db attribute is not None after a successful connect call."""
        databaseId = {}
        tinyDBAdapter = TinyDBAdapter(databaseId)
        with self.assertRaises(Exception):
            tinyDBAdapter.connect()

    def test_connect_invalidPath_throwsException(self):
        """Test that the db attribute is not None after a successful connect call."""
        databaseId = {"path" : "test"}
        tinyDBAdapter = TinyDBAdapter(databaseId)
        with self.assertRaises(Exception):
            tinyDBAdapter.connect()

    def test_connect_pathNotString_throwsException(self):
        """Test that the db attribute is not None after a successful connect call."""
        databaseId = {"path" : 1}
        tinyDBAdapter = TinyDBAdapter(databaseId)
        with self.assertRaises(Exception):
            tinyDBAdapter.connect()

    def test_query_getById_success(self):
        """Test that the inserted object is returned when queried by id."""
        db = TinyDB("test.json")
        db.truncate()
        db.insert({"id" : 253, "name" : "test"})
        db.insert({"id" : 254, "name" : "test"})
        databaseId = {"path" : "test.json"}
        tinyDBAdapter = TinyDBAdapter(databaseId)
        tinyDBAdapter.connect()
        query = {"queryType" : "get", "query" : {"id" : 253}}
        result = tinyDBAdapter.query(query)
        self.assertEqual(result["name"], "test")
        db.truncate()

if __name__ == '__main__':
    unittest.main()
