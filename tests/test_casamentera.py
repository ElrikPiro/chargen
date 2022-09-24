# -*- coding: utf-8 -*-

from math import nan
from chargen.character import Character
from chargen.casamentera import Casamentera
from context import chargen
import random
import unittest

def generateChar(sexo : str, id : int, clase : str = "Media", nacimiento : int = 0) -> chargen.Character:
    char = chargen.Character({
            "nombre": id,
            "sexo": sexo,
            "clase_social": clase,
            "eventos": {
                "nacimiento": nacimiento
            }
        }
    )
    return char

class casamenteraTest(unittest.TestCase):

    def test_suite(self):
        casametera = Casamentera([2,3,4], 1991, 2091, False)
        self.assertIsNotNone(casametera)

    def test_iterar_unaDivaYunTirillas_laDivaNoTieneFechaMatrimonio(self):
        sexo = random.choice(["Hombre", "Mujer"])
        contrario = "Hombre" if sexo == "Mujer" else "Mujer"
        diva = generateChar(sexo, id=-1, clase="Noble")
        tirillas = generateChar(contrario, id=-2, clase="Paria")
        
        casamentera = Casamentera([-1, -2], 20, 30, False)
        casamentera.iterar()
        
        diva = Character({}, diva.file)
        divaEventos : dict = diva.data.get("eventos")
        self.assertIsNotNone(divaEventos)
        divaMatrimonio = divaEventos.get("matrimonio", nan)
        self.assertFalse(divaMatrimonio == divaMatrimonio, 
            "La diva tiene fecha de matrimonio cuando nunca debería haberse casado"
        )
        


if __name__ == '__main__':
    unittest.main()