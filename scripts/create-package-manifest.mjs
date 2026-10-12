import { createHash } from "node:crypto";
import { lstat, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const [directory, releaseVersion, applicationVersion, target, commit, executable, statusFile, sourceFileList] = process.argv.slice(2);
if (![directory, releaseVersion, applicationVersion, target, commit, executable, statusFile, sourceFileList].every(Boolean)) {
  throw new Error("Usage: create-package-manifest.mjs DIR RELEASE_VERSION APP_VERSION TARGET COMMIT EXECUTABLE STATUS_FILE SOURCE_FILE_LIST");
}

const repository = resolve(process.cwd());
const status = await readFile(resolve(statusFile));
const sourceFiles = (await readFile(resolve(sourceFileList)))
  .toString("utf8")
  .split("\0")
  .filter(Boolean)
  .sort();
const sourceHash = createHash("sha256");
sourceHash.update("git-status\0").update(status).update("\0");
for (const path of sourceFiles) {
  try {
    const metadata = await lstat(resolve(repository, path));
    if (!metadata.isFile() && !metadata.isSymbolicLink()) continue;
    sourceHash
      .update(path)
      .update("\0")
      .update(await readFile(resolve(repository, path)))
      .update("\0");
  } catch (error) {
    if (error.code === "ENOENT") sourceHash.update("missing\0").update(path).update("\0");
    else throw error;
  }
}
const windowsTarget = target.startsWith("windows-");
const manifest = {
  manifest_version: 1,
  product: "Chargen",
  release_version: releaseVersion,
  application_version: applicationVersion,
  target,
  commit,
  source_dirty: status.length > 0,
  source_fingerprint: `sha256:${sourceHash.digest("hex")}`,
  executable,
  archive_format: "tar.gz",
  offline_runtime: true,
  runtime: {
    platform: windowsTarget ? "windows" : target.split("-")[0],
    crt: windowsTarget ? "statically-linked-msvc" : "platform-default",
    external_runtime_dependencies: windowsTarget ? ["Windows system DLLs"] : ["platform system libraries"],
    verification: windowsTarget ? "PE imports checked for dynamic MSVC CRT" : "not inspected",
  },
  contents: [executable, "LICENSE", "README.md", "docs/"],
};
await writeFile(resolve(directory, "MANIFEST.json"), `${JSON.stringify(manifest, null, 2)}\n`);
