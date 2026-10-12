const API = "/api";
const byId = (id) => document.getElementById(id);
const savedContextKey = "chargen.context.v1";
let activeJobId = null;
let activePoll = null;
let currentScenario = null;

const runStatusLabels = {
  queued: "En cola",
  running: "En marcha",
  paused: "En pausa",
  completed: "Completada",
  cancelled: "Cancelada",
  failed: "Fallida",
};

async function request(path, options = {}) {
  const response = await fetch(`${API}${path}`, {
    headers: { "content-type": "application/json", ...(options.headers ?? {}) },
    ...options,
  });
  const body = response.status === 204 ? null : await response.json();
  if (!response.ok) {
    const message = body?.message ?? `Solicitud fallida (${response.status})`;
    throw new Error(message);
  }
  return body;
}

function showMessage(message, error = false) {
  const region = byId("messages");
  region.replaceChildren();
  if (!message) return;
  const notice = document.createElement("p");
  notice.className = error ? "notice error" : "notice";
  notice.textContent = message;
  region.append(notice);
}

function selectedContext() {
  return {
    worldId: byId("world-select").value,
    branchId: byId("branch-select").value,
    year: Number(byId("year-input").value),
  };
}

function saveContext() {
  const context = selectedContext();
  if (context.worldId && context.branchId && Number.isInteger(context.year)) {
    localStorage.setItem(savedContextKey, JSON.stringify(context));
  }
}

function worldLabel(world) {
  return world.name || world.world_id || world.id || "Mundo";
}

async function refreshWorlds(restore = true, preferredContext = null) {
  const worlds = await request("/worlds");
  const select = byId("world-select");
  const previous = preferredContext ?? (restore ? JSON.parse(localStorage.getItem(savedContextKey) || "null") : null);
  select.replaceChildren(new Option("Selecciona o crea un mundo", ""));
  for (const world of worlds) {
    const id = world.world_id ?? world.id;
    if (id) select.add(new Option(worldLabel(world), id));
  }
  const wanted = previous?.worldId;
  if (wanted && [...select.options].some((option) => option.value === wanted)) select.value = wanted;
  await refreshBranches(Boolean(previous), previous);
}

async function refreshBranches(restore = true, preferredContext = null) {
  const worldId = byId("world-select").value;
  const select = byId("branch-select");
  const previous = preferredContext ?? (restore ? JSON.parse(localStorage.getItem(savedContextKey) || "null") : null);
  select.replaceChildren(new Option("Selecciona una rama", ""));
  if (!worldId) return;
  const branches = await request(`/worlds/${encodeURIComponent(worldId)}/branches`);
  for (const branch of branches) {
    const id = branch.branch_id ?? branch.id;
    if (id) {
      const head = branch.head_year == null ? "sin años" : `hasta ${branch.head_year}`;
      const parent = branch.parent_branch_id ? ` · hija de ${branch.parent_branch_id}` : "";
      select.add(new Option(`${id} (${head}${parent})`, id));
    }
  }
  const wanted = previous?.branchId;
  if (wanted && [...select.options].some((option) => option.value === wanted)) select.value = wanted;
  else if (!restore) select.value = [...select.options].find((option) => option.value === "main")?.value ?? select.options[1]?.value ?? "";
  if (previous && Number.isInteger(previous.year)) byId("year-input").value = String(previous.year);
  else if (previous && Number.isInteger(currentScenario?.start_year)) byId("year-input").value = String(currentScenario.start_year);
  saveContext();
}

async function parseScenario() {
  const scenario = JSON.parse(byId("scenario-json").value);
  return request("/scenarios/validate", { method: "POST", body: JSON.stringify({ scenario }) });
}

async function loadCensus() {
  const context = selectedContext();
  if (!context.worldId || !context.branchId || !Number.isInteger(context.year)) {
    throw new Error("Selecciona un mundo, una rama y un año entero.");
  }
  saveContext();
  const query = new URLSearchParams({ year: String(context.year), limit: "100" });
  const census = await request(`/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}/census?${query}`);
  renderCensus(census);
}

function renderCensus(value) {
  const host = byId("results");
  const rows = value?.settlements ?? value?.items ?? [];
  if (!Array.isArray(rows) || rows.length === 0) {
    host.innerHTML = `<p class="muted">No hay asentamientos en la vista de censo para este año.</p>`;
    return;
  }
  const table = document.createElement("table");
  const head = document.createElement("thead");
  head.innerHTML = "<tr><th>Asentamiento</th><th>Habitantes vivos</th></tr>";
  const body = document.createElement("tbody");
  for (const row of rows) {
    const tr = document.createElement("tr");
    const name = row.settlement_name ?? row.name ?? row.settlement_id ?? row.id ?? "Asentamiento";
    const count = row.population ?? row.living_population ?? row.count ?? "—";
    tr.insertCell().textContent = String(name);
    tr.insertCell().textContent = String(count);
    body.append(tr);
  }
  table.append(head, body);
  host.replaceChildren(table);
}

function setJobControls(status) {
  byId("run-status").textContent = runStatusLabels[status] ?? "Estado desconocido";
  byId("run-status").dataset.status = status;
  const isActive = ["queued", "running", "paused"].includes(status);
  byId("pause-run").disabled = !activeJobId || status !== "running";
  byId("resume-run").disabled = !activeJobId || status !== "paused";
  byId("cancel-run").disabled = !activeJobId || !isActive;
}

async function pollJob() {
  if (!activeJobId) return;
  const context = selectedContext();
  const job = await request(`/runs/${encodeURIComponent(activeJobId)}`);
  setJobControls(job.status);
  const done = Number(job.completed_years ?? 0);
  const total = Number(job.total_years ?? 1);
  byId("run-progress").max = Math.max(total, 1);
  byId("run-progress").value = Math.min(done, total);
  byId("progress-label").textContent = job.current_year == null
    ? `Estado: ${runStatusLabels[job.status] ?? "Desconocido"}.`
    : `Año ${job.current_year} confirmado · ${done} de ${total} pasos.`;
  if (["completed", "cancelled", "failed"].includes(job.status)) {
    clearInterval(activePoll);
    activePoll = null;
    if (job.status === "completed") await loadCensus().catch(() => {});
    activeJobId = null;
    setJobControls(job.status);
    return;
  }
  if (context.worldId && context.branchId) saveContext();
}

async function startRun() {
  const context = selectedContext();
  const throughYear = Number(byId("through-year").value);
  if (!context.worldId || !context.branchId) throw new Error("Selecciona un mundo y una rama.");
  if (!Number.isSafeInteger(throughYear)) throw new Error("El año de cierre debe ser un entero seguro.");
  const job = await request(`/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}/runs`, {
    method: "POST",
    body: JSON.stringify({ through_year: throughYear }),
  });
  activeJobId = job.job_id;
  byId("run-progress").value = 0;
  setJobControls(job.status ?? "queued");
  activePoll = setInterval(() => pollJob().catch((error) => showMessage(error.message, true)), 1000);
  await pollJob();
}

async function initialize() {
  try {
    const health = await request("/health");
    byId("connection").textContent = `Servicio local listo · ${health.version ?? "E1"}`;
    const demo = await request("/demo-scenario");
    currentScenario = demo;
    byId("scenario-json").value = JSON.stringify(demo, null, 2);
    await refreshWorlds();
  } catch (error) {
    byId("connection").textContent = "Servicio local no disponible";
    showMessage(error.message, true);
  }
}

byId("refresh-worlds").addEventListener("click", () => refreshWorlds(true).then(() => showMessage("Lista de mundos actualizada.")).catch((e) => showMessage(e.message, true)));
byId("world-select").addEventListener("change", () => refreshBranches(false).catch((e) => showMessage(e.message, true)));
byId("branch-select").addEventListener("change", saveContext);
byId("year-input").addEventListener("change", saveContext);
byId("load-census").addEventListener("click", () => loadCensus().then(() => showMessage("Censo consultado para el año seleccionado.")).catch((e) => showMessage(e.message, true)));
byId("validate-scenario").addEventListener("click", () => parseScenario().then((result) => {
  if (result.valid) showMessage("Escenario válido.");
  else showMessage(`El escenario tiene ${result.issues?.length ?? 0} errores de validación.`, true);
}).catch((e) => showMessage(e.message, true)));
byId("create-world").addEventListener("click", async () => {
  try {
    const scenario = JSON.parse(byId("scenario-json").value);
    const created = await request("/worlds", { method: "POST", body: JSON.stringify({ scenario }) });
    currentScenario = scenario;
    await refreshWorlds(false, {
      worldId: created.world_id,
      branchId: created.root_branch_id,
      year: created.head_year ?? scenario.start_year,
    });
    showMessage("Mundo guardado. Ya puedes iniciar una corrida.");
  } catch (error) { showMessage(error.message, true); }
});
byId("start-run").addEventListener("click", () => startRun().catch((e) => showMessage(e.message, true)));
byId("pause-run").addEventListener("click", () => request(`/runs/${encodeURIComponent(activeJobId)}/pause`, { method: "POST" }).then(pollJob).catch((e) => showMessage(e.message, true)));
byId("resume-run").addEventListener("click", () => request(`/runs/${encodeURIComponent(activeJobId)}/resume`, { method: "POST" }).then(pollJob).catch((e) => showMessage(e.message, true)));
byId("cancel-run").addEventListener("click", () => request(`/runs/${encodeURIComponent(activeJobId)}/cancel`, { method: "POST" }).then(pollJob).catch((e) => showMessage(e.message, true)));
initialize();
