from context import chargen
import unittest

def generateChar(
        sexo : str, 
        id : int, 
        clase : str = "Media", 
        nacimiento : int = 0,
        localizacion : str = "PLACEHOLDER"
                                ) -> chargen.Character:
    char = chargen.Character({
            "nombre": id,
            "sexo": sexo,
            "clase_social": clase,
            "eventos": {
                "nacimiento": nacimiento
            },
            "lugar_nacimiento" : localizacion
        }
    )

    char.getPersonalidad()
    char.getGenoma()

    char.save()

    return char

def addDeformity(c : chargen.Character):
    hashed = 1
    genoma = c.getGenoma()
    genoma["humano"]["cabeza"]["calvicie"]["paterno"]["hash"] = hashed
    genoma["humano"]["cabeza"]["calvicie"]["materno"]["hash"] = hashed


class characterTest(unittest.TestCase):

    def test_generateDeformityText_noDeformities_expectedText(self):
        testChar = generateChar(sexo="Hombre", id=-1)
        text = chargen.render.generateDeformityText(testChar)
        self.assertEqual(text, "No tiene signos de deformidad.")
        pass

    def test_generateDeformityText_deformity_notUnexpectedText(self):
        testChar = generateChar(sexo="Hombre", id=-1)
        addDeformity(testChar)
        text = chargen.render.generateDeformityText(testChar)
        self.assertNotEqual(text, "No tiene signos de deformidad.")
        pass