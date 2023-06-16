import unittest
from chargen import ModuleFactory
from chargen import CharacterBuilder

def GetCulturalNameModule():
    """Returns a cultural name module"""
    return ModuleFactory().buildModule("CulturalNameModule")

def GetCharacter():
    """Returns a character"""
    return CharacterBuilder("TestCulturalNameModule", modules={"MockupCulture": ModuleFactory().buildModule("MockupCulture").__dict__()}).get()

class TestCulturalNameModule(unittest.TestCase):

    def test_getFieldInterface(self):
        module = GetCulturalNameModule()
        self.assertEqual(module.getFieldInterface(), "INameModule")

    def test_getInstanceType(self):
        module = GetCulturalNameModule()
        self.assertEqual(module.getInstanceType(), "CulturalNameModule")

    def test_getDependencies(self):
        module = GetCulturalNameModule()
        self.assertEqual(module.getDependencies(), ["ICultureModule"])

    def test_setParams(self):
        module = GetCulturalNameModule()
        module.setParams({"name": "John"})
        self.assertEqual(module.getParams(), {"name": "John"})

    def test_resolve_returnsFalseIfCultureModuleIsNone(self):
        module = GetCulturalNameModule()
        character = GetCharacter()
        character.modules_[module.getInstanceType()] = module.__dict__()
        character.modules_.pop("MockupCulture")
        self.assertFalse(module.resolve(character))

    def test_resolve_generatesNameIfNameIsNone(self):
        module = GetCulturalNameModule()
        character = GetCharacter()
        module.setParams({"name": None})
        character.modules_[module.getInstanceType()] = module.__dict__()
        module.resolve(character)
        self.assertNotEqual(module.getName(), None)

    def test_resolve_returnsTrueIfNameIsNotEmptyString(self):
        module = GetCulturalNameModule()
        character = GetCharacter()
        module.setParams({"name": "John"})
        character.modules_[module.getInstanceType()] = module.__dict__()
        self.assertTrue(module.resolve(character))

    def test_resolve_returnsTrueIfNameIsEmptyString(self):
        module = GetCulturalNameModule()
        character = GetCharacter()
        module.setParams({"name": ""})
        character.modules_[module.getInstanceType()] = module.__dict__()
        self.assertTrue(module.resolve(character))

    def test_resolve_returnsFalseIfNameIsNotString(self):
        module = GetCulturalNameModule()
        character = GetCharacter()
        module.setParams({"name": 123})
        character.modules_[module.getInstanceType()] = module.__dict__()
        self.assertFalse(module.resolve(character))

    def test_getName(self):
        module = GetCulturalNameModule()
        module.setParams({"name": "John"})
        self.assertEqual(module.getName(), "John")

if __name__ == '__main__':
    unittest.main()