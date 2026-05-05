from ..chargen import Character
from ..chargen.render import markdownGenerator


def main():
    character = Character({})
    markdown = markdownGenerator(character.file, prompt_func=lambda _: "Example")
    print(markdown[:800])


if __name__ == "__main__":
    main()
