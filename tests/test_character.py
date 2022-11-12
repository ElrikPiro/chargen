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

    def test_getGenProgenitor_getHashParameterTrue_returnsASemiColonSeparatedStr(self):
        testChar = generateChar(sexo="Hombre", id=-1)
        genProgenitor = testChar.getGenProgenitor(
            chargen.loadJson("config/genoma.json"),
            testChar.getPadre().data["genoma"] if testChar.getPadre().hasGenoma() else {},
            "humano", "cabeza", "calvicie", 
            "paterno",
            True
        )

        listData = genProgenitor.split(";")
        self.assertEqual(len(listData), 2, "Should be 2")

        pass

    def test_getGenProgenitor_hasGenoma_getsHash(self):
        testChar = generateChar(sexo="Hombre", id=-1)
        genProgenitor = testChar.getGenProgenitor(
            chargen.loadJson("config/genoma.json"),
            testChar.getPadre().data["genoma"] if testChar.getPadre().hasGenoma() else {},
            "humano", "cabeza", "calvicie", 
            "paterno",
            True
        )

        listData = genProgenitor.split(";")

        self.assertIsNot(int(listData[1]), 0)

        pass

    def test_getGenProgenitor_hasParents_HashInherithed(self):
        testChar = generateChar(sexo="Hombre", id=-1)
        testChar.data["parientes"] = {"hijos" : {"len": 1, "lista" : []}}
        testHijoFile = testChar.getHijos()[0]

        listaGenesPadre : list = list()
        listaGenesPadre.append(
            testChar.getGenProgenitor(
                chargen.loadJson("config/genoma.json"),
                testChar.getPadre().data["genoma"] if testChar.getPadre().hasGenoma() else {},
                "humano", "cabeza", "calvicie", 
                "paterno",
                True
            ).split(";")[1]
        )
        listaGenesPadre.append(
            testChar.getGenProgenitor(
                chargen.loadJson("config/genoma.json"),
                testChar.getPadre().data["genoma"] if testChar.getPadre().hasGenoma() else {},
                "humano", "cabeza", "calvicie", 
                "materno",
                True
            ).split(";")[1]
        )

        testHijo = chargen.Character({}, testHijoFile)
        testHijo.getGenoma()
        genPadre = testChar.getGenProgenitor(
            chargen.loadJson("config/genoma.json"),
            testHijo.getPadre().data["genoma"] if testChar.getPadre().hasGenoma() else {},
            "humano", "cabeza", "calvicie", 
            "materno",
            True
        ).split(";")[1]

        self.assertTrue(genPadre in listaGenesPadre)

        pass

    def test_hasDeformation_twoSameGenes_True(self):
        alelo : dict = {
            "materno" : {
                "no presente": {
                    "dominancia": 0
                },
                "hash": 1
            },
            "paterno": {
                "no presente": {
                    "dominancia": 0
                },
                "hash": 1
            }
        }

        self.assertTrue(chargen.character.hasDeformation(alelo))

        pass

if __name__ == '__main__':
    unittest.main()