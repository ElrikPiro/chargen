import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [linuxPath, windowsPath, profile = "benchmark"] = process.argv.slice(2);
assert.ok(linuxPath && windowsPath, "usage: node compare-benchmark-digests.mjs LINUX.txt WINDOWS.txt [PROFILE]");
const [linux, windows] = await Promise.all([
  readFile(linuxPath, "utf8").then((value) => value.trim()),
  readFile(windowsPath, "utf8").then((value) => value.trim()),
]);
assert.match(linux, /^[0-9a-f]{16,64}$/i, "Linux digest file is invalid");
assert.equal(windows, linux, `${profile} deterministic output differs across platforms: Linux=${linux}, Windows=${windows}`);
console.log(`Cross-platform ${profile} deterministic digest matches: ${linux}`);
