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

    def test_get_returnsCharacter(self):
        builder = chargen.CharacterBuilder()
        self.assertIsInstance(builder.get(), chargen.Character)

    def test_get_characterHasId(self):
        builder = chargen.CharacterBuilder()
        self.assertIsNotNone(builder.get().id_)

    def test_get_characterHasCorrectId(self):
        builder = chargen.CharacterBuilder(characterId = "test")
        self.assertEqual(builder.get().id_, "test")

    def test_get_characterModulesIsNotNone(self):
        builder = chargen.CharacterBuilder()
        self.assertIsNotNone(builder.get().modules_)

    def test_save_characterIsSaved(self):
        builder = chargen.CharacterBuilder()
        character = builder.get()
        builder.save()
        self.assertEqual(builder.database_.query({"queryType" : "get", "query" : {"id_" : character.id_}}), character.__dict__)

    def test_build_withStaticMockup_returnsTrue(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleStatic" : {"mockKey" : "mockValue"}})
        self.assertTrue(builder.build()[0])


if __name__ == '__main__':
    unittest.main()
