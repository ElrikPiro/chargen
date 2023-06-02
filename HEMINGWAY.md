# Hemingway bridge document

This is a document for self-answering the question: "What was I doing?"

## What I was doing

obsidian://open?vault=content&file=SecondBrain%2FPARA%2F1.%20Projects%2FSIDEPROJECT%2FGenerador%20de%20personajes%2FChargen%2FTareas%2FUH-0001.b.11.canvas
Debemos poder buildear un personaje que le pasemos un parametro "name" y nos devuelva como cached el valor de ese parametro. InputName, vaya.

stack
- Definir una CulturalNameModule que implemente INameModule
- Despues de esto ya podemos hacer un test de integracion que use la MockupCultureModule y la CulturalNameModule a nivel de CharacterBuilder
- Luego hará falta definir una API que utilice el módulo que estamos desarrollando para hacer un servicio web
- Una vez tengamos servicio web, podremos hacer un cliente web que use el servicio web