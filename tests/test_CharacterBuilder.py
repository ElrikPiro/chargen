from .context import chargen

from random import randint
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

    def test_get_characterModulesIsEmpty(self):
        builder = chargen.CharacterBuilder()
        self.assertEqual(builder.get().modules_, {})

    def test_get_characterModulesIsCorrect(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleStatic" : {"mockKey" : "mockValue"}})
        builder.build()
        self.assertTrue(builder.get().modules_["MockModuleStatic"]["cached"] == "mockValue")

    def test_save_characterIsSaved(self):
        builder = chargen.CharacterBuilder()
        character = builder.get()
        builder.save()
        self.assertEqual(builder.database_.query({"queryType" : "get", "query" : {"id_" : character.id_}}), character.__dict__)

    def test_build_withStaticMockup_returnsTrue(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleStatic" : {"mockKey" : "mockValue"}})
        self.assertTrue(builder.build()[0])

    def test_build_withStaticMockup_returnsComment(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleStatic" : {"mockKey" : "mockValue"}})
        self.assertEqual(builder.build()[1], "")

    def test_build_withStaticMockupAndInvalidKey_returnsFalse(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleStatic" : {"invalid" : "mockValue"}})
        self.assertFalse(builder.build()[0])

    def test_build_withStaticMockupAndInvalidKey_returnsComment(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleStatic" : {"invalid" : "mockValue"}})
        self.assertEqual(builder.build()[1], "Module MockModuleStatic could not be resolved\n")

    def test_build_withDynamicMockup_returnsTrue(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleDynamic" : {"mockKey" : 1}})
        self.assertTrue(builder.build()[0])

    def test_build_withDynamicMockup_returnsComment(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleDynamic" : {"mockKey" : 1}})
        self.assertEqual(builder.build()[1], "")

    def test_build_withDynamicMockupAndInvalidKey_returnsFalse(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleDynamic" : {"invalid" : 1}})
        self.assertFalse(builder.build()[0])

    def test_build_withDynamicMockupAndInvalidKey_returnsComment(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleDynamic" : {"invalid" : 1}})
        self.assertEqual(builder.build()[1], "Module MockModuleDynamic could not be resolved\n")

    def test_build_withDynamicMockupAndInvalidValue_returnsFalse(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleDynamic" : {"mockKey" : "invalid"}})
        self.assertFalse(builder.build()[0])

    def test_build_withDynamicMockupAndInvalidValue_returnsComment(self):
        builder = chargen.CharacterBuilder(modules={"MockModuleDynamic" : {"mockKey" : "invalid"}})
        self.assertEqual(builder.build()[1], "Module MockModuleDynamic could not be resolved\n")

    def test_get_characterModulesDynamicIsCorrect(self):
        param = randint(0, 100)
        builder = chargen.CharacterBuilder(modules={"MockModuleDynamic" : {"mockKey" : param}})
        builder.build()
        self.assertTrue(builder.get().modules_["MockModuleDynamic"]["cached"] == param*2)



if __name__ == '__main__':
    unittest.main()
