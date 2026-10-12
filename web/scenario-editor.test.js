import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  addOptionalProperty,
  defaultArrayItem,
  defaultForKey,
  defaultFromSchema,
  enumLabelFor,
  inferredEnum,
  nullableDefault,
  parseEditorValue,
  resolveSchema,
  schemaAllowsNull,
} from "./scenario-editor.js";

test("schema string fields create editable strings even when names look numeric", () => {
  assert.equal(defaultForKey("id", { type: "string" }), "");
  assert.equal(parseEditorValue("profession: artisan", { type: "string" }), "profession: artisan");
});

test("free-form units and economic decimal strings stay editable text", () => {
  const unitPath = ["economy", "goods", "grain", "unit"];
  assert.equal(inferredEnum("unit", "ración", { type: "string" }, unitPath), null);
  assert.equal(parseEditorValue("1.250", { type: "string" }), "1.250");
});

test("an omitted optional field and an empty array can be created from schema", () => {
  const root = {
    properties: {
      professions: {
        type: "array",
        items: { type: "object", required: ["id"], properties: { id: { type: "string" } } },
      },
      economy: { type: "object", required: ["goods"], properties: { goods: { type: "object", additionalProperties: { type: "string" } } } },
    },
  };
  const scenario = addOptionalProperty({}, "professions", root.properties.professions, root);
  assert.deepEqual(scenario.professions, []);
  assert.deepEqual(defaultArrayItem(root.properties.professions, "profession", root), { id: "" });
  assert.deepEqual(addOptionalProperty(scenario, "economy", root.properties.economy, root), { professions: [], economy: { goods: {} } });
});

test("map keys such as __proto__ remain own data through JSON round-trips", () => {
  const catalog = addOptionalProperty({}, "__proto__", { type: "string" }, {});
  assert.equal(Object.getPrototypeOf(catalog), Object.prototype);
  assert.equal(Object.hasOwn(catalog, "__proto__"), true);
  assert.equal(catalog.__proto__, "");
  const roundTrip = JSON.parse(JSON.stringify(catalog));
  assert.equal(Object.getPrototypeOf(roundTrip), Object.prototype);
  assert.equal(Object.hasOwn(roundTrip, "__proto__"), true);
  assert.equal(roundTrip.__proto__, "");
});

test("oneOf defaults include the selected discriminant and required fields", () => {
  const union = {
    oneOf: [
      {
        type: "object",
        title: "Muerte",
        required: ["type", "data"],
        properties: {
          type: { const: "deaths" },
          data: { type: "object", required: ["count"], properties: { count: { type: "integer" } } },
        },
      },
      {
        type: "object",
        title: "Pérdida de producción",
        required: ["type", "data"],
        properties: {
          type: { const: "production_loss" },
          data: { type: "object", required: ["loss_ppm"], properties: { loss_ppm: { type: "integer" } } },
        },
      },
    ],
  };
  assert.deepEqual(defaultFromSchema(union, "effect", {}), { type: "deaths", data: { count: 0 } });
});

test("nullable fields can switch between a value and unknown", () => {
  const nullableYear = { type: ["integer", "null"] };
  assert.equal(schemaAllowsNull(nullableYear), true);
  assert.equal(nullableDefault(null, nullableYear, "maximum_age"), 0);
  assert.equal(nullableDefault(42, nullableYear, "maximum_age"), null);
  assert.equal(defaultFromSchema({ type: "null" }, "unknown", {}), null);
  assert.equal(parseEditorValue("1.25", nullableYear), 1.25);
  assert.equal(parseEditorValue("", nullableYear), null);
  assert.equal(defaultFromSchema({ type: ["string", "null"], const: "fixed" }, "id", {}), "fixed");
});

test("frequent family and simulation enums have Spanish display labels", () => {
  assert.equal(enumLabelFor("partnership"), "Pareja");
  assert.equal(enumLabelFor("marriage"), "Matrimonio");
  assert.equal(enumLabelFor("deaths"), "Muertes");
});

test("real schema documents keep $defs available for policies, settlement defaults and event variants", () => {
  for (const version of [1, 2, 3]) {
    const schemaDocument = JSON.parse(readFileSync(new URL(`../schemas/scenario-v${version}.schema.json`, import.meta.url), "utf8"));
    const scenarioRoot = resolveSchema(schemaDocument, schemaDocument);
    assert.ok(scenarioRoot.required.includes("policies"), `v${version} root required policies`);
    assert.ok(scenarioRoot.properties.policies, `v${version} has policies`);

    const policiesSchema = resolveSchema(scenarioRoot.properties.policies, schemaDocument);
    assert.ok(policiesSchema.required.includes("mortality"), `v${version} mortality is required`);
    const mortality = addOptionalProperty({}, "mortality", policiesSchema.properties.mortality, schemaDocument).mortality;
    assert.equal(mortality.default_probability_ppm, 0, `v${version} numeric mortality default`);
    assert.deepEqual(mortality.age_bands, [], `v${version} age-band array default`);

    const newSettlement = defaultArrayItem(scenarioRoot.properties.settlements, "settlement", schemaDocument);
    assert.deepEqual(newSettlement, { id: 0, logical_id: "", name: "", culture_values: {} }, `v${version} settlement default`);
  }

  const schemaDocument = JSON.parse(readFileSync(new URL("../schemas/scenario-v3.schema.json", import.meta.url), "utf8"));
  const scenarioRoot = resolveSchema(schemaDocument, schemaDocument);
  const eventArray = resolveSchema(scenarioRoot.properties.scheduled_events, schemaDocument);
  const event = resolveSchema(eventArray.items, schemaDocument);
  const effectArray = resolveSchema(event.properties.effects, schemaDocument);
  const effectSchema = resolveSchema(effectArray.items, schemaDocument);
  assert.ok(effectSchema.oneOf?.length >= 6, "v3 effects resolve all oneOf variants from $defs");
  const newEffect = defaultArrayItem(effectArray, "effect", schemaDocument);
  assert.equal(newEffect.type, "deaths");
  assert.ok(Object.hasOwn(newEffect.data, "selection"));
});
