from ..chargen import Character


def main():
    character = Character({})
    print(f"Created character file: {character.file}")
    print(f"Name id: {character.getNombreId()}")
    print(f"Sex: {character.getSexo()}")


if __name__ == "__main__":
    main()
