import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const [directory, expectedTarget, expectedExecutable] = process.argv.slice(2);
if (![directory, expectedTarget, expectedExecutable].every(Boolean)) {
  throw new Error("Usage: check-package.mjs PACKAGE_DIR EXPECTED_TARGET EXPECTED_EXECUTABLE");
}

const packageDir = resolve(directory);
const manifest = JSON.parse(await readFile(resolve(packageDir, "MANIFEST.json"), "utf8"));
assert.equal(manifest.target, expectedTarget);
assert.equal(manifest.executable, expectedExecutable);
assert.equal(manifest.offline_runtime, true);
assert.equal(typeof manifest.source_dirty, "boolean");
assert.match(manifest.source_fingerprint, /^sha256:[a-f0-9]{64}$/);

if (expectedTarget.startsWith("windows-")) {
  assert.deepEqual(manifest.runtime, {
    platform: "windows",
    crt: "statically-linked-msvc",
    external_runtime_dependencies: ["Windows system DLLs"],
    verification: "PE imports checked for dynamic MSVC CRT",
  });
  const executable = await readFile(resolve(packageDir, expectedExecutable));
  const importedDlls = readPeImports(executable);
  const dynamicCrt = importedDlls.filter((name) =>
    /^(?:api-ms-win-crt-|ucrtbase\.dll$|vcruntime\d+.*\.dll$|msvcp\d+.*\.dll$|concrt\d+.*\.dll$|vccorlib\d+.*\.dll$)/i.test(name),
  );
  assert.deepEqual(dynamicCrt, [], `Unexpected dynamic MSVC CRT imports: ${dynamicCrt.join(", ")}`);
  console.log(`Windows package manifest and PE runtime verified (${importedDlls.length} imported DLLs).`);
} else {
  assert.equal(manifest.runtime.platform, expectedTarget.split("-")[0]);
  assert.equal(manifest.runtime.crt, "platform-default");
  assert.equal(manifest.runtime.verification, "not inspected");
  console.log("Package manifest verified.");
}

function readPeImports(image) {
  assert.equal(image.toString("ascii", 0, 2), "MZ", "Not a Windows PE executable");
  const peOffset = image.readUInt32LE(0x3c);
  assert.equal(image.toString("ascii", peOffset, peOffset + 4), "PE\0\0", "Invalid PE signature");
  assert.equal(image.readUInt16LE(peOffset + 4), 0x8664, "Expected x86_64 Windows executable");

  const sectionCount = image.readUInt16LE(peOffset + 6);
  const optionalHeaderSize = image.readUInt16LE(peOffset + 20);
  const optionalHeader = peOffset + 24;
  const magic = image.readUInt16LE(optionalHeader);
  assert.ok(magic === 0x20b || magic === 0x10b, "Unsupported PE optional-header format");
  const directoryOffset = optionalHeader + (magic === 0x20b ? 112 : 96);
  const importRva = image.readUInt32LE(directoryOffset + 8);
  const sectionsOffset = optionalHeader + optionalHeaderSize;
  const sections = [];
  for (let index = 0; index < sectionCount; index += 1) {
    const offset = sectionsOffset + index * 40;
    sections.push({
      virtualSize: image.readUInt32LE(offset + 8),
      virtualAddress: image.readUInt32LE(offset + 12),
      rawSize: image.readUInt32LE(offset + 16),
      rawAddress: image.readUInt32LE(offset + 20),
    });
  }
  const offsetForRva = (rva) => {
    const section = sections.find((item) => rva >= item.virtualAddress && rva < item.virtualAddress + Math.max(item.virtualSize, item.rawSize));
    assert.ok(section, `PE RVA 0x${rva.toString(16)} is outside all sections`);
    const offset = section.rawAddress + rva - section.virtualAddress;
    assert.ok(offset < image.length, `PE RVA 0x${rva.toString(16)} exceeds the executable`);
    return offset;
  };
  if (importRva === 0) return [];

  const importsOffset = offsetForRva(importRva);
  const imports = [];
  for (let offset = importsOffset; offset + 20 <= image.length; offset += 20) {
    const nameRva = image.readUInt32LE(offset + 12);
    if (nameRva === 0) break;
    let nameOffset = offsetForRva(nameRva);
    let endOffset = nameOffset;
    while (endOffset < image.length && image[endOffset] !== 0) endOffset += 1;
    assert.ok(endOffset < image.length, "Unterminated DLL name in PE import table");
    imports.push(image.toString("ascii", nameOffset, endOffset));
  }
  return imports;
}
