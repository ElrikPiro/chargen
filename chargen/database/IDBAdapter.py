# Interface for the IDBAdapter interface
## This interface is used to abstract the database access layer from the rest of the application

from abc import abstractmethod


queriesTypeList : list[dict] = [
    {}
]

queryModel : dict = {
    "queryType" : str,
    "query" : dict
}

class IDBAdapter():

    def __init__(self, databaseId : dict):
        databaseId_ = databaseId
        pass

    @abstractmethod
    def connect(self):
        pass
    
    @abstractmethod
    def query(self, query : dict) -> dict:
        pass