import { readFile } from "node:fs/promises";

const source = await readFile(new URL("../web/app.js", import.meta.url), "utf8");
const html = await readFile(new URL("../web/index.html", import.meta.url), "utf8");
const forbidden = ["https://", "http://", "//cdn.", "iframe"];
for (const marker of forbidden) {
  if (source.includes(marker) || html.includes(marker)) {
    throw new Error(`Web resources must remain local; found ${marker}`);
  }
}
for (const expected of ["const API = \"/api\"", '"/health"', '"/worlds"', '"/scenarios/validate"', '/census', '/runs/', '/markets/settlements', '/markets/trades', '/events']) {
  if (!source.includes(expected)) throw new Error(`Missing API route reference: ${expected}`);
}
for (const marker of ['id="load-e3"', 'id="economy-results"', 'id="load-e3-demo"']) {
  if (!html.includes(marker)) throw new Error(`Missing E3 interface marker: ${marker}`);
}
console.log("Web source uses local resources and references the expected API routes.");
