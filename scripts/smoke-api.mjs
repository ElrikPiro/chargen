import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { delimiter, resolve } from "node:path";
import { createInterface } from "node:readline";
import { setTimeout as delay } from "node:timers/promises";

const executable = resolve(process.argv[2] ?? "target/release/chargen-app");
const temporaryDirectory = await mkdtemp(`${tmpdir()}${delimiter === ";" ? "\\" : "/"}chargen-api-smoke-`);
const database = resolve(temporaryDirectory, "worlds.sqlite3");
const childEnvironment = { ...process.env };
for (const key of Object.keys(childEnvironment)) {
  if (key.toLowerCase() === "path") delete childEnvironment[key];
}
childEnvironment.PATH = temporaryDirectory;
let scenario;
let child;

async function startServer() {
  child = spawn(executable, ["serve", "--database", database, "--bind", "127.0.0.1:0", "--no-open"], {
    cwd: temporaryDirectory,
    env: childEnvironment,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  let stderr = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => { stderr = `${stderr}${chunk}`.slice(-8000); });
  const lines = createInterface({ input: child.stdout });
  const urlPromise = new Promise((resolveUrl, reject) => {
    const timeout = setTimeout(() => reject(new Error(`Server did not announce its address. ${stderr}`)), 20_000);
    child.once("error", (error) => { clearTimeout(timeout); reject(error); });
    child.once("exit", (code) => {
      clearTimeout(timeout);
      reject(new Error(`Server exited before startup (code ${code}). ${stderr}`));
    });
    lines.on("line", (line) => {
      const match = line.match(/^Chargen listening on (http:\/\/127\.0\.0\.1:\d+\/)$/);
      if (match) { clearTimeout(timeout); resolveUrl(match[1]); }
    });
  });
  const baseUrl = await urlPromise;
  for (let attempt = 0; attempt < 40; attempt += 1) {
    try {
      const response = await fetch(new URL("api/health", baseUrl));
      if (response.ok) return baseUrl;
    } catch {}
    await delay(50);
  }
  throw new Error(`Server announced an address but did not become available. ${stderr}`);
}

async function stopServer() {
  if (!child || child.exitCode !== null) return;
  const exited = new Promise((resolveExit) => child.once("exit", resolveExit));
  child.kill();
  await Promise.race([exited, delay(5000)]);
  if (child.exitCode === null) child.kill("SIGKILL");
  await exited;
  child = undefined;
}

async function json(baseUrl, route, options = {}, expectedStatus = 200) {
  const response = await fetch(new URL(route.replace(/^\//, ""), baseUrl), {
    headers: { "content-type": "application/json", ...(options.headers ?? {}) },
    ...options,
  });
  const body = response.status === 204 ? null : await response.json();
  assert.equal(response.status, expectedStatus, `${options.method ?? "GET"} ${route}: ${JSON.stringify(body)}`);
  return body;
}

async function assertLocalJavaScriptModules(baseUrl) {
  const baseOrigin = new URL(baseUrl).origin;
  const pending = ["/app.js"];
  const visited = new Set();
  while (pending.length > 0) {
    const route = pending.pop();
    if (visited.has(route)) continue;
    visited.add(route);
    const response = await fetch(new URL(route, baseUrl));
    assert.equal(response.status, 200, `JavaScript module ${route} must be served locally`);
    assert.match(response.headers.get("content-type") ?? "", /javascript/i, `${route} must use a JavaScript MIME type`);
    const source = await response.text();
    for (const match of source.matchAll(/(?:\bfrom\s*|\bimport\s*)["']([^"']+\.js)["']/g)) {
      const dependency = new URL(match[1], new URL(route, baseUrl));
      assert.equal(dependency.origin, baseOrigin, `JavaScript import ${match[1]} must remain local`);
      pending.push(`${dependency.pathname}${dependency.search}`);
    }
  }
}

async function assertDurableHistory(baseUrl) {
  const health = await json(baseUrl, "/api/health");
  assert.equal(health.offline, true);
  assert.equal(health.api_version, 1);
  const root = await fetch(new URL("", baseUrl)).then((response) => response.text());
  assert.match(root, /Chargen/);
  await assertLocalJavaScriptModules(baseUrl);
  assert.equal((await json(baseUrl, "/api/scenarios/validate", {
    method: "POST", body: JSON.stringify({ scenario }),
  })).valid, true);

  const worlds = await json(baseUrl, "/api/worlds");
  assert.ok(worlds.some((world) => world.world_id === String(scenario.world_id)));
  const branches = await json(baseUrl, `/api/worlds/${scenario.world_id}/branches`);
  assert.ok(branches.some((branch) => branch.branch_id === "main"));

  const census = await json(baseUrl, `/api/worlds/${scenario.world_id}/branches/main/census?year=0`);
  assert.equal(census.year, 0);
  assert.equal(census.total_population, 3);
  assert.equal(census.settlements[0]?.settlement_name, "Bajo del Roble");
  const people = await json(baseUrl, `/api/worlds/${scenario.world_id}/branches/main/people?year=0&limit=10`);
  assert.equal(people.people.length, 3);
  return census;
}

try {
  let baseUrl = await startServer();
  scenario = await json(baseUrl, "/api/demo-scenario");
  scenario.world_id = 880_013;
  scenario.policies.mortality.default_probability_ppm = 0;
  scenario.policies.unions.enabled = false;
  scenario.policies.births.enabled = false;
  scenario.policies.adoption.enabled = false;
  const created = await json(baseUrl, "/api/worlds", {
    method: "POST", body: JSON.stringify({ scenario }),
  });
  assert.equal(created.world_id, String(scenario.world_id));

  const accepted = await json(baseUrl, `/api/worlds/${scenario.world_id}/branches/main/runs`, {
    method: "POST", body: JSON.stringify({ through_year: 0 }),
  });
  assert.ok(accepted.job_id);
  let run = accepted;
  for (let attempt = 0; attempt < 200; attempt += 1) {
    run = await json(baseUrl, `/api/runs/${encodeURIComponent(accepted.job_id)}`);
    if (["completed", "cancelled", "failed"].includes(run.status)) break;
    await delay(25);
  }
  assert.equal(run.status, "completed", JSON.stringify(run));
  assert.equal(run.current_year, 0);
  const firstCensus = await assertDurableHistory(baseUrl);

  await stopServer();
  baseUrl = await startServer();
  const secondCensus = await assertDurableHistory(baseUrl);
  assert.deepEqual(secondCensus, firstCensus, "history changes after reopening the same database");
  console.log("API smoke passed: scenario validation, create, annual commit, census/people queries and restart recovery.");
} finally {
  await stopServer();
  await rm(temporaryDirectory, { recursive: true, force: true });
}
