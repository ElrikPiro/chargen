SHELL := /bin/bash

venv:
	python3 -m venv venv

init: venv
	venv/bin/pip install --upgrade pip
	source venv/bin/activate && pip install -r requirements.txt

clean:
	rm -rf venv

.PHONY: init clean