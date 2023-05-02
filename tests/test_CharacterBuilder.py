from .context import chargen

import unittest


class CharacterBuilderTest(unittest.TestCase):
    """CharacterBuilder test cases."""

    def test_init_default_characterIdIsCurrentTime(self):
        builder = chargen.CharacterBuilder()
        self.assertIsNotNone(builder.settings_["characterId"])

    def test_init_default_persistenceIsTinyDB(self):
        builder = chargen.CharacterBuilder()
        self.assertEqual(builder.settings_["persistence"], "tinyDB")

    def test_init_default_modulesIsEmpty(self):
        builder = chargen.CharacterBuilder()
        self.assertEqual(builder.settings_["modules"], {})

    def test_init_characterIdIsSet_characterIdIsSet(self):
        builder = chargen.CharacterBuilder(characterId = "test")
        self.assertEqual(builder.settings_["characterId"], "test")

    def test_init_persistenceIsInvalid_exception(self):
        with self.assertRaises(Exception):
            chargen.CharacterBuilder(persistence = "invalid")

    def test_build_returnsCharacter(self):
        builder = chargen.CharacterBuilder()
        self.assertIsInstance(builder.build(), chargen.Character)

    def test_build_characterHasId(self):
        builder = chargen.CharacterBuilder()
        self.assertIsNotNone(builder.build().id_)

    def test_build_characterHasCorrectId(self):
        builder = chargen.CharacterBuilder(characterId = "test")
        self.assertEqual(builder.build().id_, "test")

    def test_build_characterModulesIsNotNone(self):
        builder = chargen.CharacterBuilder()
        self.assertIsNotNone(builder.build().modules_)

    def test_save_characterIsSaved(self):
        builder = chargen.CharacterBuilder()
        character = builder.build()
        builder.save()
        self.assertEqual(builder.database_.query({"queryType" : "get", "query" : {"id_" : character.id_}}), character.__dict__)


if __name__ == '__main__':
    unittest.main()
