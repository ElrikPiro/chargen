import json

# load file as string
with open("necesidades.csv", "r") as f:
    data = f.read()

# split string into list of lines
lines = data.splitlines()

#str positivo, str negativo, personalidad que suma, personalidad que resta
retval = []
for line in lines:
    line = line.split(",")
    strPositivo = line[0]
    strNegativo = line[1]
    sumPersonalidad = line[2].split(";")
    subPersonalidad = line[3].split(";")
    retval.append((strPositivo, strNegativo, sumPersonalidad, subPersonalidad))

retval = {"necesidades" : retval}

# save as json
with open("necesidades.json", "w") as f:
    json.dump(retval, f, indent=4)