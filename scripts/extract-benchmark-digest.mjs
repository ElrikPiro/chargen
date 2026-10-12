import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";

const [reportPath, digestPath] = process.argv.slice(2);
assert.ok(reportPath && digestPath, "usage: node extract-benchmark-digest.mjs REPORT.json DIGEST.txt");
const report = JSON.parse(await readFile(reportPath, "utf8"));
assert.equal(typeof report.deterministic_digest, "string", "benchmark report is missing deterministic_digest");
assert.match(report.deterministic_digest, /^[0-9a-f]{16,64}$/i, "deterministic_digest must be a fixed-width hexadecimal fingerprint");
await writeFile(digestPath, `${report.deterministic_digest}\n`, "utf8");
