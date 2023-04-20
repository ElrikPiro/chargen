"""Database Adapter Factory"""

from chargen.database.IDBAdapter import IDBAdapter

class DBAdapterFactory():
    """Factory class for database adapters"""

    @staticmethod
    def createAdapter(databaseId : dict) -> IDBAdapter:
        """Creates a database adapter based on the databaseId"""
        if databaseId["type"] == "tinyDB":
            from chargen.database.TinyDBAdapter import TinyDBAdapter
            return TinyDBAdapter(databaseId)
        pass