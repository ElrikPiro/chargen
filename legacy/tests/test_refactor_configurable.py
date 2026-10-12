import json
import math
import os
import tempfile
import unittest

from context import chargen
from chargen.casamentera import isReachable
from chargen.core import JsonStore, loadJson, writeJson
from chargen.render import getEdadList


def seed_config_files(root):
    config_dir = os.path.join(root, "config")
    os.makedirs(config_dir, exist_ok=True)
    with open(os.path.join(config_dir, "nombresPropios.json"), "w") as f:
        json.dump({}, f)
    with open(os.path.join(config_dir, "familias.json"), "w") as f:
        json.dump({}, f)
    with open(os.path.join(config_dir, "localizaciones.json"), "w") as f:
        json.dump({}, f)
    with open(os.path.join(config_dir, "obras.json"), "w") as f:
        json.dump({"obra": 100}, f)


class FakeRng:
    def randint(self, a, b):
        return 5

    def choice(self, values):
        return values[0]

    def normalvariate(self, mean, std):
        return mean

    def betavariate(self, a, b):
        return 0.5

    def shuffle(self, values):
        return None


class DummyGraph:
    def __init__(self, json="config/localizaciones.json", io=None):
        self._json = json
        self._io = io

    def getShortestPath(self, origen, destino):
        return [[origen, destino], 1.0]


class RefactorConfigurableTest(unittest.TestCase):
    def test_core_json_store_uses_custom_root(self):
        with tempfile.TemporaryDirectory() as temp_root:
            store = JsonStore(temp_root)
            writeJson("config/demo.json", {"ok": True}, store)
            data = loadJson("config/demo.json", store)
            self.assertEqual(data, {"ok": True})

    def test_character_can_persist_to_injected_store(self):
        with tempfile.TemporaryDirectory() as temp_root:
            store = JsonStore(temp_root)
            seed_config_files(temp_root)

            c = chargen.Character({}, io=store)
            persisted = loadJson(c.file, store)

            self.assertEqual(persisted["nombre"], c.data["nombre"])

    def test_character_uses_injected_prompt_provider_for_birth(self):
        with tempfile.TemporaryDirectory() as temp_root:
            store = JsonStore(temp_root)
            seed_config_files(temp_root)

            c = chargen.Character(
                {
                    "nombre": 1,
                    "eventos": {
                        "nacimiento": math.nan,
                    },
                },
                io=store,
                prompt_func=lambda _: "123",
            )

            self.assertEqual(c.getNacimiento(), 123)

    def test_character_uses_injected_rng_for_rolls(self):
        with tempfile.TemporaryDirectory() as temp_root:
            store = JsonStore(temp_root)
            seed_config_files(temp_root)

            c = chargen.Character({"nombre": 1}, io=store, rng=FakeRng())
            self.assertEqual(c.rollEdadMuerte(40), 45)

    def test_is_reachable_allows_injected_graph_factory(self):
        self.assertTrue(isReachable("A", "B", graph_factory=DummyGraph))

    def test_render_uses_injected_store_for_works_catalog(self):
        with tempfile.TemporaryDirectory() as temp_root:
            store = JsonStore(temp_root)
            seed_config_files(temp_root)
            writeJson("config/obras.json", {"obra": 100}, store)

            c = chargen.Character(
                {
                    "nombre": 1,
                    "sexo": "Hombre",
                    "eventos": {"nacimiento": 90, "muerte": 110},
                },
                io=store,
            )

            edad_list = getEdadList(c, io=store)
            self.assertIn("obra (100): 10", edad_list)


if __name__ == "__main__":
    unittest.main()
