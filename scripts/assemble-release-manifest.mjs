import { createHash } from "node:crypto";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { basename, join, relative, resolve, sep } from "node:path";

const root = resolve(process.argv[2] ?? "release-assets");

async function filesUnder(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(entries.map(async (entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return filesUnder(path);
    return [path];
  }));
  return nested.flat();
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

const files = await filesUnder(root);
const archives = files.filter((path) => path.endsWith(".tar.gz")).sort();
if (archives.length === 0) throw new Error(`No package archives found in ${root}`);

const artifacts = [];
for (const archive of archives) {
  const packageManifestPath = archive.replace(/\.tar\.gz$/, ".manifest.json");
  const checksumPath = `${archive}.sha256`;
  const [archiveBytes, packageManifest, checksumFile] = await Promise.all([
    readFile(archive),
    readFile(packageManifestPath, "utf8").then(JSON.parse),
    readFile(checksumPath, "utf8"),
  ]);
  const digest = sha256(archiveBytes);
  if (!checksumFile.startsWith(`${digest}  ${basename(archive)}`)) {
    throw new Error(`Checksum mismatch for ${archive}`);
  }
  if (typeof packageManifest.source_dirty !== "boolean" || !/^sha256:[a-f0-9]{64}$/.test(packageManifest.source_fingerprint ?? "")) {
    throw new Error(`Missing source provenance in ${packageManifestPath}`);
  }
  artifacts.push({
    file: relative(root, archive).split(sep).join("/"),
    sha256: digest,
    target: packageManifest.target,
    release_version: packageManifest.release_version,
    application_version: packageManifest.application_version,
    commit: packageManifest.commit,
    source_dirty: packageManifest.source_dirty,
    source_fingerprint: packageManifest.source_fingerprint,
    runtime: packageManifest.runtime,
    manifest_file: relative(root, packageManifestPath).split(sep).join("/"),
  });
}

const releaseManifest = {
  manifest_version: 1,
  product: "Chargen",
  created_at_utc: new Date().toISOString(),
  artifacts,
};
const releaseManifestPath = join(root, "release-manifest.json");
await writeFile(releaseManifestPath, `${JSON.stringify(releaseManifest, null, 2)}\n`);

const checksummed = [
  ...archives,
  ...artifacts.map((artifact) => join(root, artifact.manifest_file)),
  releaseManifestPath,
].sort();
const checksumLines = await Promise.all(checksummed.map(async (path) => {
  return `${sha256(await readFile(path))}  ${relative(root, path).split(sep).join("/")}`;
}));
await writeFile(join(root, "SHA256SUMS.txt"), `${checksumLines.join("\n")}\n`);
