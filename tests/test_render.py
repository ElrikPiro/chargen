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

class characterTest(unittest.TestCase):

    def test_generateDeformityText_noDeformities_expectedText(self):
        testChar = generateChar(sexo="Hombre", id=-1)
        text = chargen.render.generateDeformityText(testChar)
        self.assertEqual(text, "No tiene signos de deformidad.")
        pass

    def test_generateDeformityText_deformity_notUnexpectedText(self):
        #comprueba que no te diga que no tiene signos de deformidad
        pass