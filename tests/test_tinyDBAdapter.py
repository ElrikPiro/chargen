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
        db.insert({"id_" : 253, "name" : "test"})
        db.insert({"id_" : 254, "name" : "test"})
        databaseId = {"path" : "test.json"}
        tinyDBAdapter = TinyDBAdapter(databaseId)
        tinyDBAdapter.connect()
        query = {"queryType" : "get", "query" : {"id_" : 253}}
        result = tinyDBAdapter.query(query)
        self.assertEqual(result["name"], "test")
        db.truncate()

    def test_query_insert_success(self):
        """Test that the inserted object is returned when queried by id."""
        db = TinyDB("test.json")
        db.truncate()
        databaseId = {"path" : "test.json"}
        tinyDBAdapter = TinyDBAdapter(databaseId)
        tinyDBAdapter.connect()
        query = {"queryType" : "insert", "query" : {"id" : 253, "name" : "test"}}
        tinyDBAdapter.query(query)
        result = db.get(Query().id == 253)
        self.assertEqual(result["name"], "test")
        db.truncate()

    def test_query_getAllIds_success(self):
        """Test that the inserted object is returned when queried by id."""
        db = TinyDB("test.json")
        db.truncate()
        db.insert({"id_" : 253, "name" : "test"})
        db.insert({"id_" : 254, "name" : "test"})
        databaseId = {"path" : "test.json"}
        tinyDBAdapter = TinyDBAdapter(databaseId)
        tinyDBAdapter.connect()
        query = {"queryType" : "getAllIds", "query" : {}}
        result = tinyDBAdapter.query(query)
        self.assertEqual(len(result), 2)
        # Check that it should return a list of ids and ONLY ids
        self.assertTrue(all(isinstance(x, str) for x in result))
        db.truncate()

if __name__ == '__main__':
    unittest.main()
