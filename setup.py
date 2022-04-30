# -*- coding: utf-8 -*-

# Learn more: https://github.com/kennethreitz/setup.py

from setuptools import setup, find_packages


with open('README.rst') as f:
    readme = f.read()

with open('LICENSE') as f:
    license = f.read()

setup(
    name='chargen',
    version='0.1.0',
    description='Python based character generator for worldbuilding',
    long_description=readme,
    author='David Baselga',
    author_email='emperador.albino@gmail.com',
    url='https://github.com/ElrikPiro/chargen',
    license=license,
    packages=find_packages(exclude=('tests', 'docs'))
)

