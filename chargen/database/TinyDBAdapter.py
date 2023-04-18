# Implements the TinyDBAdapter class
## This class implements the IDBAdapter interface for the TinyDB database
## WORK IN PROGRESS

from . import IDBAdapter

class TinyDBAdapter(IDBAdapter.IDBAdapter):

    def __init__(self, databaseId : dict):
        super().__init__(databaseId)

    def connect(self):
        pass

    def query(self, query : dict) -> dict:
        pass