# Implements the TinyDBAdapter class
## This class implements the IDBAdapter interface for the TinyDB database
## WORK IN PROGRESS

from . import IDBAdapter
from tinydb import TinyDB, Query

class TinyDBAdapter(IDBAdapter.IDBAdapter):

    db_ : TinyDB = None
    databaseId_ : dict = None

    def __init__(self, databaseId : dict):
        self.databaseId_ = databaseId

    def connect(self):
        path = self.databaseId_["path"]
        if not self.__isJsonExtension(path):
            raise Exception("Invalid path extension")
        self.db_ = TinyDB(self.databaseId_["path"])
        pass

    def query(self, query : dict) -> dict:
        pass

    #private methods
    def __isJsonExtension(self, path : str) -> bool:
        return path.endswith(".json")