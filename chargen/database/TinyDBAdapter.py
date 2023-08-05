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
        queryType = query["queryType"]
        queryContent = query["query"]
        tinyDBQuery = Query()
        if queryType == "get":
            for key in queryContent:
                tinyDBQuery = tinyDBQuery & (tinyDBQuery[key] == queryContent[key])
            return self.db_.get(Query().id_ == query["query"]["id_"])
        elif queryType == "insert":
            self.db_.insert(queryContent)
            return queryContent
        elif queryType == "getAllIds":
            db = self.db_.all()
            # it must be a list of strings where each string is an id_
            return [str(item["id_"]) for item in db]
        pass

    #private methods
    def __isJsonExtension(self, path : str) -> bool:
        return path.endswith(".json")