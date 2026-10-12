#!/usr/bin/env python3
import argparse
import builtins
import json
import shutil
from pathlib import Path

from chargen import Character
from chargen.casamentera import Casamentera
from chargen.core import JsonStore
from chargen.render import markdownGenerator
import chargen.core as chargen_core


LOCATIONS = ["Unnamed"]
DEFAULT_RENDER_YEAR = 1501
DEFAULT_RENDER_LOCATION = "Unnamed"
BASE_DIR = Path(__file__).resolve().parent
BASE_JSON_DIR = BASE_DIR / "json"
RUNTIME_JSON_DIR = BASE_DIR / "runtime_json"
RENDER_ROOT_DIR = BASE_DIR / "renders"


def parse_args():
    parser = argparse.ArgumentParser(
        description="Create a PoC population and run 200 years of matchmaking."
    )
    parser.add_argument("--years", type=int, default=200, help="Years to simulate")
    parser.add_argument(
        "--chars-per-location",
        type=int,
        default=50,
        help="Initial characters generated for each location",
    )
    parser.add_argument(
        "--birth-year",
        type=int,
        default=0,
        help="Birth year assigned to the initial population",
    )
    parser.add_argument(
        "--render-year",
        type=int,
        default=DEFAULT_RENDER_YEAR,
        help="Year used to filter alive characters for markdown render",
    )
    parser.add_argument(
        "--render-location",
        default=DEFAULT_RENDER_LOCATION,
        help="Residence location used to filter markdown render",
    )
    return parser.parse_args()


def clean_previous_population(store):
    for character_file in Path(store.root).glob("personaje_*.json"):
        character_file.unlink()


def prepare_runtime_store():
    if RUNTIME_JSON_DIR.exists():
        shutil.rmtree(RUNTIME_JSON_DIR)
    shutil.copytree(BASE_JSON_DIR, RUNTIME_JSON_DIR)
    return JsonStore(str(RUNTIME_JSON_DIR))


def create_initial_population(chars_per_location, birth_year, store):
    population = []

    for location in LOCATIONS:
        for idx in range(chars_per_location):
            sexo = "Hombre" if idx % 2 == 0 else "Mujer"
            character = Character(
                {
                    "sexo": sexo,
                    "eventos": {"nacimiento": birth_year},
                    "lugar_nacimiento": location,
                    "clase_social": "Media",
                },
                io=store,
            )
            character.getMuerte()
            character.save()

            population.append(character.getNombreId())

    return population


def run_matchmaking(population, years, start_year, store):
    end_year = start_year + years
    engine = Casamentera(
        population,
        begin=start_year,
        end=end_year,
        debug=False,
        usarLocalizaciones=True,
        io=store,
    )

    while engine.year_ < engine.end_:
        engine.iterar()


def summarize(initial_population_size, store):
    character_files = list(Path(store.root).glob("personaje_*.json"))
    married = 0

    for character_file in character_files:
        with character_file.open() as fh:
            data = json.load(fh)
        spouse = data.get("parientes", {}).get("conyugue")
        if isinstance(spouse, str):
            married += 1

    print(f"Initial population: {initial_population_size}")
    print(f"Total generated characters: {len(character_files)}")
    print(f"Characters with spouse: {married}")


def get_render_output_dir(year, location):
    safe_location = location.lower().replace(" ", "_")
    return RENDER_ROOT_DIR / f"year_{year}_{safe_location}"


def build_non_interactive_prompt():
    counters = {
        "family": 0,
        "name": 0,
        "place": 0,
    }

    def _prompt(message):
        text = message.lower()
        if "familia" in text:
            counters["family"] += 1
            return f"House_{counters['family']}"
        if "nombre del personaje" in text:
            counters["name"] += 1
            return f"Name_{counters['name']}"
        if "nombre del lugar" in text:
            counters["place"] += 1
            return f"Place_{counters['place']}"
        if "clase social" in text:
            return "Media"
        return "AutoValue"

    return _prompt


def render_alive_characters(store, year, location):
    output_dir = get_render_output_dir(year, location)
    if output_dir.exists():
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    prompt_func = build_non_interactive_prompt()
    selected = []

    for character_file in sorted(Path(store.root).glob("personaje_*.json")):
        character = Character({}, character_file.name, io=store, prompt_func=prompt_func)
        if not (character.getNacimiento() <= year < character.getMuerte()):
            continue

        residencia = character.getLugarResidencia()
        residencia_nombre = residencia.get("nombre") if isinstance(residencia, dict) else residencia
        if residencia_nombre != location:
            continue

        selected.append(character.file)

    previous_input = builtins.input
    builtins.input = prompt_func
    try:
        for character_json in selected:
            markdown = markdownGenerator(character_json, io=store, prompt_func=prompt_func)
            markdown_path = output_dir / f"{Path(character_json).stem}.md"
            markdown_path.write_text(markdown, encoding="utf-8")
    finally:
        builtins.input = previous_input

    print(f"Rendered characters for year {year} in {location}: {len(selected)}")
    print(f"Render directory: {output_dir}")


def main():
    args = parse_args()
    store = prepare_runtime_store()
    chargen_core.DEFAULT_JSON_STORE = store
    clean_previous_population(store)

    population = create_initial_population(
        chars_per_location=args.chars_per_location,
        birth_year=args.birth_year,
        store=store,
    )
    run_matchmaking(
        population,
        years=args.years,
        start_year=args.birth_year,
        store=store,
    )
    summarize(initial_population_size=len(population), store=store)
    render_alive_characters(store, year=args.render_year, location=args.render_location)


if __name__ == "__main__":
    main()
