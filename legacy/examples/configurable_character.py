import os
import json

from ..chargen import Character
from ..chargen.core import JsonStore


def bootstrap_store(root):
    config_dir = os.path.join(root, "config")
    os.makedirs(config_dir, exist_ok=True)
    for filename, content in {
        "nombresPropios.json": {},
        "familias.json": {},
        "localizaciones.json": {},
        "obras.json": {},
    }.items():
        path = os.path.join(config_dir, filename)
        if not os.path.exists(path):
            with open(path, "w") as fh:
                json.dump(content, fh)


def main():
    base_dir = os.path.dirname(__file__)
    isolated_root = os.path.join(base_dir, "tmp_json")
    store = JsonStore(isolated_root)
    bootstrap_store(isolated_root)

    character = Character({}, io=store)
    print(f"Created character in isolated store: {isolated_root}")
    print(f"Character file: {character.file}")


if __name__ == "__main__":
    main()
