from dataclasses import dataclass

@dataclass
class Character:
    id_ : str = None # type: ignore
    modules_ : dict = None # type: ignore