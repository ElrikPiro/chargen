from ..chargen import Character
from ..chargen.casamentera import Casamentera


def main():
    char_a = Character({"nombre": -100, "sexo": "Hombre", "eventos": {"nacimiento": 0}, "clase_social": "Media"})
    char_b = Character({"nombre": -101, "sexo": "Mujer", "eventos": {"nacimiento": 0}, "clase_social": "Media"})

    engine = Casamentera([-100, -101], begin=20, end=30, debug=False)
    engine.iterar()

    updated = Character({}, char_a.file)
    spouse = updated.data.get("parientes", {}).get("conyugue")
    print(f"Spouse for {updated.file}: {spouse}")


if __name__ == "__main__":
    main()
