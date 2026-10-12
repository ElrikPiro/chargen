#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
VENV_DIR="${SCRIPT_DIR}/.venv"

python3 -m venv "${VENV_DIR}"
source "${VENV_DIR}/bin/activate"

python -m pip install --upgrade pip setuptools wheel
python -m pip install -r "${REPO_DIR}/requirements.txt"

pushd "${REPO_DIR}" >/dev/null
rm -rf build dist chargen.egg-info
python setup.py sdist bdist_wheel
python -m pip install --force-reinstall dist/chargen-*.whl
popd >/dev/null

pushd "${SCRIPT_DIR}" >/dev/null
python run_simulation.py --years 200 --chars-per-location 20 --birth-year 1301 --render-year 1501 --render-location Unnamed
popd >/dev/null

# "/home/dbaselga/git/chargen/PoC/.venv/bin/python" "/home/dbaselga/git/chargen/PoC/run_simulation.py" --years 200 --chars-per-location 20 --birth-year 1301 --render-year 1501 --render-location Unnamed