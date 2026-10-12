import { readFile } from "node:fs/promises";

const source = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const editor = await readFile(new URL("../web/scenario-editor.js", import.meta.url), "utf8");
const html = await readFile(new URL("../web/index.html", import.meta.url), "utf8");
// SVG elements use this namespace identifier; it is not a network request.
const localSource = source.replaceAll('"http://www.w3.org/2000/svg"', "");
const forbidden = ["https://", "http://", "//cdn.", "iframe"];
for (const marker of forbidden) {
  if (localSource.includes(marker) || editor.includes(marker) || html.includes(marker)) {
    throw new Error(`Web resources must remain local; found ${marker}`);
  }
}
for (const expected of [
  'const PRIMARY_API = "/api"', '"/health"', '"/worlds"', '"/scenarios/validate"',
  "/census", "/runs/", "/markets/settlements", "/markets/trades", "/events",
  "/dossier", "/tree", "/unions/", "/compare", "/markdown", "/import/archive",
]) {
  if (!source.includes(expected)) throw new Error(`Missing API route reference: ${expected}`);
}
if (!source.includes('from "/scenario-editor.js"')) throw new Error("The local schema-driven scenario editor is not imported.");
if (!source.includes("/scenarios/schema?version=")) throw new Error("The scenario editor must load a versioned local JSON Schema.");
for (const marker of ['id="load-e3"', 'id="economy-results"', 'id="load-e3-demo"']) {
  if (!html.includes(marker)) throw new Error(`Missing E3 interface marker: ${marker}`);
}
console.log("Web source uses local resources and references the expected E1–E4 API routes.");
