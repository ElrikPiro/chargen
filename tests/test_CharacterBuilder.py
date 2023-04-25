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
        with self.assertRaises(ValueError):
            chargen.CharacterBuilder(persistence = "invalid")


if __name__ == '__main__':
    unittest.main()
