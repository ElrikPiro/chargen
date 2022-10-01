# -*- coding: utf-8 -*-

from math import nan
from threading import local
from chargen.character import Character
from chargen.casamentera import Casamentera
from context import chargen
import random
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

    def test_iterar_edadesMuyDiferentesPeroPosible_terminanCasados(self):
        
        voteAndRepeat : int = 0

        for i in range(0,10):
            sexo = random.choice(["Hombre", "Mujer"])
            contrario = "Hombre" if sexo == "Mujer" else "Mujer"
            diva = generateChar(sexo, id=-1, clase="Media")
            tirillas = generateChar(contrario, id=-2, clase="Media", nacimiento=10)

            casamentera = Casamentera([-1, -2], 20, 30, False)
            while casamentera.year_ < casamentera.end_:
                casamentera.iterar()

            diva = Character({}, diva.file)
            divaParientes : dict = diva.data.get("parientes")
            self.assertIsNotNone(divaParientes)
            divaConyugue : str = divaParientes.get("conyugue")

            voteAndRepeat += 1 if divaConyugue == tirillas.file else 0

        self.assertGreaterEqual(voteAndRepeat, 5)

    def test_getPoblacionValida_localizacionesIrrastreables_soloAparecenCompatibles(self):
        
        voteAndRepeat : int = 0
        
        sexo = random.choice(["Hombre", "Mujer"])
        contrario = "Hombre" if sexo == "Mujer" else "Mujer"
        ciudadA = generateChar(sexo, id=-1, clase="Media", localizacion="Ciudad")
        ciudadB = generateChar(contrario, id=-2, clase="Media", localizacion="Ciudad")
        pueblo = generateChar(contrario, id=-3, clase="Media", localizacion="Pueblo")

        casamentera = Casamentera([-1, -2, -3], 20, 30, False, usarLocalizaciones=True)
        tuplalistas = casamentera.getPoblacionValida(casamentera.year_, contrario, localizacion="Ciudad")
        idx : list[int] = tuplalistas[1]

        voteAndRepeat += 0 if idx.count(-3) else 1
        voteAndRepeat += 1 if idx.count(-2) else 0

        self.assertEqual(voteAndRepeat, 2)

        pass

    def test_iterar_localizacionesIrrastreables_personajesNoSeCasan(self):

        voteAndRepeat : int = 0

        for i in range(0,10):
            sexo = random.choice(["Hombre", "Mujer"])
            contrario = "Hombre" if sexo == "Mujer" else "Mujer"
            diva = generateChar(sexo, id=-1, clase="Media")
            tirillas = generateChar(contrario, id=-2, clase="Media")

            casamentera = Casamentera([-1, -2], 20, 30, False, usarLocalizaciones=True)
            casamentera.iterar()

            diva = Character({}, diva.file)
            divaParientes : dict = diva.data.get("parientes")
            self.assertIsNotNone(divaParientes)
            divaConyugue : str = divaParientes.get("conyugue")

            voteAndRepeat += 1 if divaConyugue == tirillas.file else 0

        self.assertEqual(voteAndRepeat, 0)

        pass

    #La gente pierde con la edad

    #refactorizacion

    #blackboxtests de las funciones
        


if __name__ == '__main__':
    unittest.main()