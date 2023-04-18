# Interface for the IDBAdapter interface
## This interface is used to abstract the database access layer from the rest of the application

queriesTypeList : list[dict] = [
    {}
]

queryModel : dict = {
    "queryType" : str,
    "query" : dict
}

class IDBAdapter():
    
    databaseId_ : dict

    def __init__(self, databaseId : dict):
        pass

    def connect(self):
        pass

    def query(self, query : dict) -> dict:
        pass