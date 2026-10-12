import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";

const file = process.argv[2];
if (!file) throw new Error("Usage: write-checksum.mjs FILE");
const absolutePath = resolve(file);
const digest = createHash("sha256").update(await readFile(absolutePath)).digest("hex");
await writeFile(`${absolutePath}.sha256`, `${digest}  ${basename(absolutePath)}\n`);
