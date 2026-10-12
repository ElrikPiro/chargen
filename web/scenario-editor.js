const ENUMS = {
  schema_version: [1, 2, 3],
  rules_version: [1, 2, 3],
  type: ["deaths", "work_incapacity", "production_loss", "stock_destruction", "route_closure", "displacement"],
  mode: ["exact", "probabilistic", "dominant", "additive"],
  overflow: ["error", "clamp_with_warning"],
  kind: ["institution", "religion", "family", "collective", "single_person", "institution", "partnership", "marriage"],
  scope: ["world", "settlement"],
  unit: ["person", "household"],
  gametes: ["ovum", "sperm", "other"],
  required_contributor_gamete: ["ovum", "sperm", "other"],
  transport_cost_account: ["treasury", "external_sink"],
  buyer_priority: ["seeded_logical_rank", "group_key_order"],
  asset_continuity: ["retain_group_assets", "proportional_share_transfer"],
  empty_group_disposition: ["retain_dormant", "transfer_to_consumer_group", "transfer_to_treasury"],
  minimum_age: [],
};

const UNION_VARIANTS = {
  type: ENUMS.type,
  mode: ENUMS.mode,
};

const ENUM_LABELS = {
  exact: "Exacto", probabilistic: "Probabilístico", dominant: "Dominante", additive: "Aditivo",
  error: "Error", clamp_with_warning: "Ajustar con aviso", institution: "Institución", religion: "Religión",
  family: "Familia", collective: "Colectivo", single_person: "Una persona", partnership: "Pareja", marriage: "Matrimonio",
  world: "Mundo", settlement: "Asentamiento", person: "Persona", household: "Hogar",
  ovum: "Óvulo", sperm: "Espermatozoide", other: "Otro", treasury: "Tesorería", external_sink: "Destino externo",
  seeded_logical_rank: "Orden lógico aleatorio", group_key_order: "Orden de clave de grupo",
  retain_group_assets: "Conservar activos del grupo", proportional_share_transfer: "Transferir proporcionalmente",
  retain_dormant: "Conservar inactivo", transfer_to_consumer_group: "Transferir al grupo consumidor",
  transfer_to_treasury: "Transferir a la tesorería", biological_parent: "Progenitor biológico",
  adoptive_parent: "Progenitor adoptivo", caregiver: "Persona cuidadora",
};

const LABELS = {
  world_id: "Identificador del mundo", schema_version: "Versión del esquema", rules_version: "Versión de reglas",
  seed: "Semilla aleatoria", start_year: "Año inicial", settlements: "Asentamientos", initial_people: "Personas iniciales",
  initial_households: "Hogares iniciales", initial_unions: "Uniones iniciales", initial_family_links: "Vínculos familiares iniciales",
  trait_catalog: "Catálogo de rasgos", initial_population_profiles: "Perfiles de población inicial", policies: "Políticas demográficas",
  genetics: "Catálogo genético", territory: "Territorio y migración", culture: "Cultura e instituciones", economy: "Economía",
  scheduled_events: "Agenda de eventos", id: "Identificador", logical_id: "Identificador lógico", name: "Nombre", kind: "Tipo",
  settlement_id: "Asentamiento", members: "Integrantes", birth_year: "Año de nacimiento", death_year: "Año de fallecimiento",
  household_id: "Hogar", traits: "Rasgos", genome: "Genoma", reproductive: "Perfil reproductivo", gametes: "Gametos",
  can_gestate: "Puede gestar", fertility_enabled: "Fertilidad habilitada", start: "Desde (incluido)", end_exclusive: "Hasta (excluido)",
  parent: "Progenitor o cuidador", child: "Hijo o hija", validity: "Vigencia", parent_weight: "Peso de progenitores",
  local_weight: "Peso local", variation_max: "Variación máxima", minimum: "Mínimo", maximum: "Máximo", enabled: "Habilitado",
  probability_ppm: "Probabilidad (por millón)", candidate_limit: "Límite de candidatos", maximum_age: "Edad máxima", minimum_age: "Edad mínima",
  route_revision: "Revisión de rutas", routes: "Rutas", migration: "Política de migración", institutions: "Instituciones y religiones",
  organizations: "Política de organizaciones", dimensions: "Dimensiones culturales", goods: "Bienes", needs: "Necesidades",
  profession_policies: "Políticas de profesiones", initial_group_endowments: "Dotaciones iniciales", scheduled_displacements: "Desplazamientos programados",
  professions: "Profesiones", profession_options_by_settlement: "Profesiones por asentamiento", scheduled_events: "Agenda de eventos",
  start_year: "Año de inicio", duration_years: "Duración (años)", priority: "Prioridad", population_filter: "Filtro de población",
  effects: "Efectos", selection: "Selección de afectados", data: "Parámetros del efecto", exact: "Cantidad exacta",
  probabilistic: "Probabilístico", deaths: "Muertes", work_incapacity: "Incapacidad laboral", production_loss: "Pérdida de producción",
  stock_destruction: "Destrucción de existencias", route_closure: "Cierre de rutas", displacement: "Desplazamiento",
  probability_ppm: "Probabilidad (por millón)", loss_ppm: "Pérdida (por millón)", route_ids: "Rutas afectadas", destination: "Destino",
  mortality: "Mortalidad", default_probability_ppm: "Probabilidad base de fallecimiento (por millón)", age_bands: "Tramos de edad",
  unions: "Uniones", formation_probability_ppm: "Probabilidad de formar una unión (por millón)",
  separation_probability_ppm: "Probabilidad de separación (por millón)", formation_attempts_per_person: "Intentos de unión por persona",
  max_active_unions_per_person: "Uniones activas máximas por persona", prohibited_kinship_generations: "Generaciones de parentesco prohibidas",
  allow_widowed_remarriage: "Permitir nuevas uniones tras enviudar", births: "Natalidad y nacimientos",
  minimum_gestational_age: "Edad mínima para gestar", maximum_gestational_age: "Edad máxima para gestar",
  require_active_union: "Exigir una unión activa", required_contributor_gamete: "Gameto requerido de quien contribuye",
  minimum_contributor_age: "Edad mínima de quien contribuye", allow_unknown_contributor: "Permitir contribución desconocida",
  maximum_births_per_person_per_year: "Nacimientos máximos por persona y año", minimum_years_between_births: "Años mínimos entre nacimientos",
  infant_mortality_probability_ppm: "Probabilidad de mortalidad infantil (por millón)", child_reproductive_profiles: "Perfiles reproductivos de descendencia",
  adoption: "Adopción y cuidados", adopt_orphans: "Adoptar a menores sin cuidadores", adoption_probability_ppm: "Probabilidad de adopción (por millón)",
  minor_age: "Edad máxima para considerarse menor", minimum_caregiver_age: "Edad mínima de quien cuida",
  maximum_adoptions_per_year: "Adopciones máximas por año", maximum_children_per_household: "Menores máximos por hogar",
  maximum_caregivers_per_child: "Personas cuidadoras máximas por menor", move_to_adoptive_household: "Trasladar al hogar adoptivo",
  add_household_care_links: "Añadir vínculos de cuidado del hogar",
};

const hasOwn = (value, key) => Object.prototype.hasOwnProperty.call(value, key);
const copy = (value) => JSON.parse(JSON.stringify(value));
const setOwn = (target, key, value) => {
  Object.defineProperty(target, key, { value, enumerable: true, configurable: true, writable: true });
  return target;
};

function labelFor(key) {
  return LABELS[key] ?? String(key).replaceAll("_", " ").replace(/(^|\s)\p{L}/gu, (part) => part.toLocaleUpperCase("es"));
}

export function enumLabelFor(value) {
  return ENUM_LABELS[value] ?? labelFor(value);
}

function schemaAt(node, root) {
  let current = node ?? {};
  const seen = new Set();
  while (current?.$ref && root && !seen.has(current.$ref)) {
    seen.add(current.$ref);
    const pointer = current.$ref.replace(/^#\//, "").split("/").map((part) => part.replaceAll("~1", "/").replaceAll("~0", "~"));
    current = pointer.reduce((value, key) => value?.[key], root) ?? {};
  }
  if (Array.isArray(current.allOf) && current.allOf.length) {
    const parts = current.allOf.map((part) => schemaAt(part, root));
    const properties = Object.assign({}, ...parts.map((part) => part.properties ?? {}), current.properties ?? {});
    const required = [...new Set([...parts.flatMap((part) => part.required ?? []), ...(current.required ?? [])])];
    const mergedAdditional = [...parts].reverse().find((part) => part.additionalProperties !== undefined)?.additionalProperties;
    current = { ...Object.assign({}, ...parts), ...current, properties, required, ...(mergedAdditional === undefined ? {} : { additionalProperties: current.additionalProperties ?? mergedAdditional }) };
  }
  return current;
}

export function resolveSchema(node, schemaDocument) {
  return schemaAt(node, schemaDocument);
}

function matchesVariant(value, variant, root) {
  const schema = schemaAt(variant, root);
  if (!value || typeof value !== "object" || !schema.properties) return false;
  return Object.entries(schema.properties).some(([key, field]) => {
    const resolved = schemaAt(field, root);
    return hasOwn(value, key) && (hasOwn(resolved, "const") ? value[key] === resolved.const : Array.isArray(resolved.enum) && resolved.enum.includes(value[key]));
  });
}

function variantList(schema, root) {
  const options = schema.oneOf ?? schema.anyOf;
  return Array.isArray(options) ? options.map((variant, index) => ({ schema: schemaAt(variant, root), index })) : [];
}

function activeVariant(value, variants, root) {
  const selected = variants.find(({ schema }) => matchesVariant(value, schema, root));
  if (selected) return selected.index;
  const valueType = Array.isArray(value) ? "array" : value === null ? "null" : typeof value;
  return variants.find(({ schema }) => {
    const types = Array.isArray(schema.type) ? schema.type : [schema.type];
    return types.some((type) => type === valueType || (type === "integer" && valueType === "number"));
  })?.index ?? 0;
}

export function defaultForKey(key, schema = {}) {
  if (hasOwn(schema, "default")) return copy(schema.default);
  if (hasOwn(schema, "const")) return copy(schema.const);
  if (Array.isArray(schema.enum) && schema.enum.length) return copy(schema.enum[0]);
  if (schema.type === "boolean") return false;
  if (schema.type === "integer" || schema.type === "number") return 0;
  if (schema.type === "string") return "";
  if (schema.type === "array") return [];
  if (schema.type === "object" || schema.properties || schema.additionalProperties) return {};
  if (Array.isArray(ENUMS[key]) && ENUMS[key].length) return copy(ENUMS[key][0]);
  if (/^(enabled|allow_|require_|can_|continue_|move_|add_|adopt_|fertility_)/.test(key)) return Boolean(schema.default ?? false);
  if (/year|age|count|limit|size|weight|probability|priority|revision|minimum|maximum|capacity|cost|price|quantity|batches|ppm|value|score|depth|start|end|seed|^id$|version/.test(key)) return 0;
  if (key === "kind") return "family";
  if (key.endsWith("_id") || key.endsWith("_name") || key === "name" || key === "variant" || key === "profession_id") return "";
  return "";
}

export function defaultFromSchema(schemaInput, key, root) {
  const schema = schemaAt(schemaInput, root);
  if (hasOwn(schema, "default")) return copy(schema.default);
  if (hasOwn(schema, "const")) return copy(schema.const);
  if (Array.isArray(schema.enum) && schema.enum.length) return copy(schema.enum[0]);
  if (schema.type === "null") return null;
  if (Array.isArray(schema.type)) {
    if (schema.type.includes("string")) return "";
    if (schema.type.includes("integer") || schema.type.includes("number")) return 0;
    if (schema.type.includes("boolean")) return false;
    if (schema.type.includes("array")) return [];
    if (schema.type.includes("object")) return {};
  }
  const variants = variantList(schema, root);
  if (variants.length) {
    const index = activeVariant({}, variants, root);
    return defaultFromSchema(variants[index].schema, key, root);
  }
  if (schema.type === "object" || schema.properties || schema.additionalProperties) {
    const result = {};
    for (const requiredKey of schema.required ?? []) {
      const propertySchema = hasOwn(schema.properties ?? {}, requiredKey) ? schema.properties[requiredKey] : {};
      setOwn(result, requiredKey, defaultFromSchema(propertySchema, requiredKey, root));
    }
    return result;
  }
  if (schema.type === "array") return [];
  return defaultForKey(key, schema);
}

export function defaultArrayItem(arraySchema, key, root) {
  const schema = schemaAt(arraySchema, root);
  return defaultFromSchema(schema.items ?? {}, key, root);
}

export function addOptionalProperty(target, key, propertySchema, root) {
  if (hasOwn(target, key)) return target;
  const result = { ...target };
  return setOwn(result, key, defaultFromSchema(propertySchema, key, root));
}

export function nullableDefault(value, schema, key, root) {
  return value === null ? defaultFromSchema(schema, key, root) : null;
}

export function schemaAllowsNull(schemaInput, root) {
  const schema = schemaAt(schemaInput, root);
  return schema.type === "null" || (Array.isArray(schema.type) && schema.type.includes("null"))
    || [schema.oneOf, schema.anyOf].some((variants) => Array.isArray(variants) && variants.some((variant) => schemaAt(variant, root).type === "null"));
}

export function parseEditorValue(value, schema = {}) {
  const types = Array.isArray(schema.type) ? schema.type : [schema.type];
  if (types.includes("number") || types.includes("integer")) {
    if (value === "") return null;
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  if (types.includes("boolean")) return value === true || value === "true";
  if (types.includes("string")) return String(value);
  return String(value);
}

function setAt(root, path, value) {
  if (path.length === 0) return value;
  let cursor = root;
  for (let index = 0; index < path.length - 1; index += 1) {
    const key = path[index];
    if (!hasOwn(cursor, key)) setOwn(cursor, key, {});
    cursor = cursor[key];
  }
  setOwn(cursor, path[path.length - 1], value);
  return root;
}

function createElement(tag, className, text) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (text != null) element.textContent = text;
  return element;
}

function makeButton(text, title, onClick, className = "small-button secondary") {
  const button = createElement("button", className, text);
  button.type = "button";
  if (title) button.setAttribute("aria-label", title);
  button.addEventListener("click", onClick);
  return button;
}

export function inferredEnum(key, value, schema = {}, path = []) {
  if (schema.enum || hasOwn(schema, "const") || ["string", "number", "integer"].includes(schema.type)) return null;
  if (key === "kind") {
    if (path.some((part) => ["initial_households", "households"].includes(part))) return ["family", "collective", "single_person", "institution"];
    if (path.some((part) => ["initial_unions", "unions"].includes(part))) return ["partnership", "marriage"];
    if (path.some((part) => part === "institutions")) return ["institution", "religion"];
    return null;
  }
  if (key === "unit" && !path.some((part) => part === "migration")) return null;
  if (Array.isArray(ENUMS[key]) && ENUMS[key].length) return ENUMS[key];
  if (typeof value === "string" && value.includes("_") && ["type", "mode", "kind", "scope", "unit", "overflow", "buyer_priority", "asset_continuity", "empty_group_disposition", "transport_cost_account", "required_contributor_gamete"].includes(key)) {
    return UNION_VARIANTS[key] ?? [value];
  }
  return null;
}

export function renderScenarioForm(container, scenario, onChange, suppliedSchema = null) {
  container.replaceChildren();
  if (!scenario || typeof scenario !== "object" || Array.isArray(scenario)) {
    container.append(createElement("p", "muted", "El escenario debe ser un objeto JSON."));
    return;
  }

  const schemaDocument = suppliedSchema;
  const rootSchema = schemaDocument ? schemaAt(schemaDocument, schemaDocument) : null;
  const rootVariants = rootSchema ? variantList(rootSchema, schemaDocument) : [];
  let chosenRoot = rootSchema;
  if (rootVariants.length) chosenRoot = rootVariants[activeVariant(scenario, rootVariants, schemaDocument)]?.schema ?? rootSchema;
  const properties = chosenRoot?.properties ?? {};
  const allKeys = new Set([...Object.keys(properties), ...Object.keys(scenario)]);

  function commit(nextScenario, rerender = false) {
    scenario = nextScenario;
    onChange?.(scenario);
    if (rerender) renderScenarioForm(container, scenario, onChange, suppliedSchema);
  }

  function rowLabel(key, path, required = false) {
    const title = createElement("span", "field-title", labelFor(key));
    title.title = path.join(" · ");
    const code = createElement("code", "field-key", key);
    const label = createElement("span", "field-heading");
    label.append(title, code);
    if (required) label.append(createElement("span", "required-tag", "Obligatorio"));
    return label;
  }

  function nameControl(control, key, path) {
    control.setAttribute("aria-label", `${labelFor(key)} · ${path.join(" · ")}`);
    return control;
  }

  function renderNode(host, key, value, path, nodeSchema, depth, pathRequired = false, assignOverride = null) {
    const schema = schemaAt(nodeSchema, schemaDocument);
    const labelText = labelFor(key);
    if (depth > 18) {
      host.append(createElement("p", "muted", `${labelText}: estructura demasiado profunda para mostrar.`));
      return;
    }

    const unionVariants = variantList(schema, schemaDocument);
    if (unionVariants.length) {
      const details = createElement("details", "editor-node union-node");
      details.open = depth < 2;
      const summary = createElement("summary", "editor-summary", labelText);
      summary.append(createElement("code", "field-key", key));
      details.append(summary);
      const contents = createElement("div", "editor-children");
      const variantSelect = createElement("select", "variant-select");
      nameControl(variantSelect, `Variante de ${labelText}`, path);
      const selectedIndex = activeVariant(value, unionVariants, schemaDocument);
      unionVariants.forEach(({ schema: variant }, index) => {
        const discriminator = Object.entries(variant.properties ?? {}).find(([, child]) => hasOwn(schemaAt(child, schemaDocument), "const"));
        const description = variant.title ?? (discriminator ? enumLabelFor(String(schemaAt(discriminator[1], schemaDocument).const)) : `Variante ${index + 1}`);
        const option = new Option(description, String(index));
        variantSelect.add(option);
      });
      variantSelect.value = String(selectedIndex);
      const variantLabel = createElement("label", "field-row");
      variantLabel.append(createElement("span", "field-title", "Variante"), variantSelect);
      contents.append(variantLabel);
      variantSelect.addEventListener("change", () => {
        const index = Number(variantSelect.value);
        const replacement = defaultFromSchema(unionVariants[index]?.schema ?? {}, key, schemaDocument);
        const next = assignOverride ? assignOverride(replacement) : setAt(scenario, path, replacement);
        commit(next, true);
      });
      const activeSchema = unionVariants[selectedIndex]?.schema ?? {};
      renderNode(contents, key, value, path, activeSchema, depth + 1, pathRequired, assignOverride);
      host.append(details);
      return;
    }

    if (Array.isArray(value)) {
      const details = createElement("details", "editor-node array-node");
      details.open = depth === 0 || path[path.length - 1] === "scheduled_events" || path[path.length - 1] === "effects";
      const summary = createElement("summary", "editor-summary", `${labelText} · ${value.length}`);
      summary.append(createElement("code", "field-key", key));
      details.append(summary);
      const items = createElement("div", "array-items");
      const itemSchema = schema.items ?? {};
      value.forEach((item, index) => {
        const itemBox = createElement("div", "array-item");
        const itemPath = [...path, index];
        const itemHead = createElement("div", "array-item-heading");
        itemHead.append(createElement("span", "array-index", `${labelText} ${index + 1}`));
        const moveUp = makeButton("↑", `Mover ${labelText} ${index + 1} hacia arriba`, () => {
          if (!index) return;
          [value[index - 1], value[index]] = [value[index], value[index - 1]];
          commit(scenario, true);
        });
        moveUp.disabled = index === 0;
        const moveDown = makeButton("↓", `Mover ${labelText} ${index + 1} hacia abajo`, () => {
          if (index >= value.length - 1) return;
          [value[index + 1], value[index]] = [value[index], value[index + 1]];
          commit(scenario, true);
        });
        moveDown.disabled = index >= value.length - 1;
        itemHead.append(moveUp, moveDown, makeButton("Quitar", `Quitar ${labelText} ${index + 1}`, () => {
          value.splice(index, 1);
          commit(scenario, true);
        }, "small-button danger"));
        itemBox.append(itemHead);
        renderNode(itemBox, `[${index + 1}]`, item, itemPath, itemSchema, depth + 1, (schema.minItems ?? 0) > index);
        items.append(itemBox);
      });
      const itemDefault = defaultArrayItem(schema, key, schemaDocument);
      const add = makeButton("Añadir elemento", `Añadir elemento a ${labelText}`, () => {
        value.push(copy(itemDefault));
        commit(scenario, true);
      });
      const footer = createElement("div", "array-controls");
      footer.append(add);
      if (schema.description) footer.append(createElement("span", "muted", schema.description));
      details.append(items, footer);
      host.append(details);
      return;
    }

    if (value && typeof value === "object") {
      const details = createElement("details", "editor-node object-node");
      details.open = depth === 0 || depth === 1;
      const summary = createElement("summary", "editor-summary", labelText);
      summary.append(createElement("code", "field-key", key));
      details.append(summary);
      const children = createElement("div", "editor-children");
      const childProperties = schema.properties ?? {};
      const displayed = new Set([...Object.keys(childProperties), ...Object.keys(value)]);
      for (const childKey of displayed) {
        const exists = hasOwn(value, childKey);
        const childSchema = (hasOwn(childProperties, childKey) ? childProperties[childKey] : schema.additionalProperties) ?? {};
        const isRequired = (schema.required ?? []).includes(childKey);
        if (!exists) {
          const optional = createElement("div", "optional-field");
          optional.append(rowLabel(childKey, [...path, childKey], isRequired));
          optional.append(makeButton(isRequired ? "Añadir campo requerido" : "Añadir campo opcional", `Añadir ${labelFor(childKey)}`, () => {
            const nextValue = addOptionalProperty(value, childKey, childSchema, schemaDocument);
            const next = assignOverride ? assignOverride(nextValue) : setAt(scenario, path, nextValue);
            commit(next, true);
          }));
          children.append(optional);
          continue;
        }
        const entry = createElement("div", "object-entry");
        const tools = createElement("div", "object-entry-tools");
        const keyInput = createElement("input", "key-input");
        keyInput.type = "text";
        keyInput.value = childKey;
        keyInput.setAttribute("aria-label", `Nombre de campo ${childKey}`);
        const declaredProperty = hasOwn(childProperties, childKey);
        const mayRename = !declaredProperty || !schema.properties;
        keyInput.readOnly = !mayRename;
        if (!mayRename) keyInput.title = "Este nombre forma parte de la estructura del esquema; edita su valor en el control de abajo.";
        keyInput.addEventListener("change", () => {
          if (!mayRename) { keyInput.value = childKey; return; }
          const newKey = keyInput.value.trim();
          if (!newKey || newKey === childKey || hasOwn(value, newKey)) {
            keyInput.value = childKey;
            return;
          }
          setOwn(value, newKey, value[childKey]);
          delete value[childKey];
          commit(scenario, true);
        });
        tools.append(createElement("span", "muted", "Campo"), keyInput);
        if (!(schema.required ?? []).includes(childKey)) tools.append(makeButton("Quitar campo", `Quitar el campo ${labelFor(childKey)}`, () => {
          delete value[childKey];
          commit(scenario, true);
        }, "small-button danger"));
        entry.append(tools);
        const childPath = [...path, childKey];
        renderNode(entry, childKey, value[childKey], childPath, childSchema, depth + 1, isRequired);
        children.append(entry);
      }
      if (schema.additionalProperties === true || (schema.additionalProperties && typeof schema.additionalProperties === "object") || (!schema.properties && schema.additionalProperties !== false)) {
        const addRow = createElement("div", "map-add-row");
        const newKey = createElement("input", "new-key-input");
        newKey.type = "text";
        newKey.placeholder = "Nombre de la nueva clave";
        newKey.setAttribute("aria-label", `Nombre de nueva clave para ${labelText}`);
        const add = makeButton("Añadir clave", `Añadir clave a ${labelText}`, () => {
          const keyName = newKey.value.trim();
          if (!keyName || hasOwn(value, keyName)) return;
          const childSchema = schema.additionalProperties && typeof schema.additionalProperties === "object" ? schema.additionalProperties : {};
          setOwn(value, keyName, defaultFromSchema(childSchema, keyName, schemaDocument));
          commit(scenario, true);
        });
        addRow.append(newKey, add);
        children.append(addRow);
      }
      details.append(children);
      host.append(details);
      return;
    }

    const field = createElement("div", "field-row");
    field.append(rowLabel(key, path, pathRequired));
    if (value === null) {
      const restore = makeButton("Introducir valor", `Asignar valor a ${labelText}`, () => {
        const replacement = nullableDefault(value, schema, key, schemaDocument);
        const next = assignOverride ? assignOverride(replacement) : setAt(scenario, path, replacement);
        commit(next, true);
      });
      field.append(restore, createElement("span", "null-note", "Ahora es null / desconocido"));
      host.append(field);
      return;
    }

    const enums = Array.isArray(schema.enum) ? schema.enum : inferredEnum(key, value, schema, path);
    if (hasOwn(schema, "const")) {
      const fixed = createElement("input", "editor-control");
      fixed.type = "text";
      fixed.value = String(schema.const);
      fixed.readOnly = true;
      fixed.setAttribute("aria-label", `${labelText} · ${path.join(" · ")}: valor fijo para esta variante`);
      field.append(fixed);
    } else if (enums?.length) {
      const select = createElement("select", "editor-control");
      nameControl(select, key, path);
      enums.forEach((item) => select.add(new Option(enumLabelFor(String(item)), String(item))));
      select.value = String(value);
      select.addEventListener("change", () => {
        const nextValue = enums.find((item) => String(item) === select.value) ?? select.value;
        const next = assignOverride ? assignOverride(nextValue) : setAt(scenario, path, nextValue);
        commit(next, key === "type" || key === "mode");
      });
      field.append(select);
    } else if (typeof value === "boolean") {
      const label = createElement("label", "toggle-field");
      const input = createElement("input");
      input.type = "checkbox";
      input.checked = value;
      nameControl(input, key, path);
      input.addEventListener("change", () => {
        const next = assignOverride ? assignOverride(parseEditorValue(input.checked, schema)) : setAt(scenario, path, parseEditorValue(input.checked, schema));
        commit(next);
      });
      label.append(input, createElement("span", "toggle-track-label", input.checked ? "Sí" : "No"));
      input.addEventListener("change", () => { label.lastElementChild.textContent = input.checked ? "Sí" : "No"; });
      field.append(label);
    } else if (typeof value === "number") {
      const input = createElement("input", "editor-control");
      nameControl(input, key, path);
      input.type = Number.isInteger(value) ? "number" : "number";
      input.step = Number.isInteger(value) ? "1" : "any";
      input.value = String(value);
      input.addEventListener("input", () => {
        if (input.value === "") return;
        const number = parseEditorValue(input.value, schema);
        if (number === null) return;
        const next = assignOverride ? assignOverride(number) : setAt(scenario, path, number);
        commit(next);
      });
      field.append(input);
    } else {
      const input = createElement("input", "editor-control");
      nameControl(input, key, path);
      input.type = "text";
      input.value = String(value ?? "");
      input.addEventListener("input", () => {
        const text = parseEditorValue(input.value, schema);
        const next = assignOverride ? assignOverride(text) : setAt(scenario, path, text);
        commit(next);
      });
      field.append(input);
    }
    if (schema.description) field.append(createElement("small", "field-help", schema.description));
    if (schemaAllowsNull(schema, schemaDocument)) {
      field.append(makeButton("Marcar como desconocido", `Guardar ${labelText} como null`, () => {
        const next = assignOverride ? assignOverride(null) : setAt(scenario, path, null);
        commit(next, true);
      }));
    }
    host.append(field);
  }

  for (const key of allKeys) {
    const exists = hasOwn(scenario, key);
    const propertySchema = (hasOwn(properties, key) ? properties[key] : undefined) ?? {};
    const required = (chosenRoot?.required ?? []).includes(key);
    if (!exists) {
      const optional = createElement("section", "root-optional-field");
      optional.append(rowLabel(key, [key], required));
      optional.append(makeButton(required ? "Añadir campo requerido" : "Añadir campo opcional", `Añadir ${labelFor(key)}`, () => {
        const next = addOptionalProperty(scenario, key, propertySchema, schemaDocument);
        commit(next, true);
      }));
      container.append(optional);
      continue;
    }
    const group = createElement("details", "scenario-group");
    group.open = ["settlements", "policies", "territory", "culture", "economy", "scheduled_events"].includes(key) || key === "schema_version" || key === "start_year";
    const summary = createElement("summary", "scenario-group-summary");
    summary.append(createElement("span", "group-title", labelFor(key)), createElement("code", "field-key", key));
    if (Array.isArray(scenario[key])) summary.append(createElement("span", "group-count", `${scenario[key].length} elementos`));
    group.append(summary);
    const body = createElement("div", "scenario-group-body");
    renderNode(body, key, scenario[key], [key], propertySchema, 0, required);
    group.append(body);
    container.append(group);
  }
}
