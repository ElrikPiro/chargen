import { enumLabelFor, renderScenarioForm } from "/scenario-editor.js";
import { amountText, decimalDifference, integerDifference } from "/economic-amounts.js";

const PRIMARY_API = "/api";
const byId = (id) => document.getElementById(id);
const savedContextKey = "chargen.context.v2";
const savedDraftKey = "chargen.scenario.draft.v2";
const savedJobKey = "chargen.active-job.v1";
const savedViewKey = "chargen.view.v1";
const cache = new Map();
const cacheTtlMs = 25_000;
const cacheMaxEntries = 56;
const cacheMaxEntryChars = 320_000;
const viewRequests = new Map();
const schemas = new Map();
let currentApiBase = PRIMARY_API;
let currentRestoreId = null;
let apiCompatible = null;
let restoreCatalogController = null;
let activeJobId = null;
let activePoll = null;
let activeEvents = null;
let pauseRequested = false;
let activeJobContext = null;
let currentScenario = null;
let currentSchema = null;
let currentPersonDossier = null;
let peopleLoadedEnd = 0;
let peopleTotal = null;
let peoplePageContext = null;
let settlementNameMap = new Map();

const runStatusLabels = {
  queued: "En cola",
  running: "En marcha",
  paused: "En pausa",
  completed: "Completada",
  cancelled: "Cancelada",
  failed: "Fallida",
};

const pageLimit = 100;

function contextSignature() {
  const context = selectedContext();
  return `${currentApiBase}|${context.worldId}|${context.branchId}|${context.year}`;
}

function selectedContext() {
  return {
    apiBase: currentApiBase,
    restoreId: currentRestoreId,
    worldId: byId("world-select").value,
    branchId: byId("branch-select").value,
    year: Number(byId("year-input").value),
  };
}

function scopedContextKey(base = currentApiBase) {
  return `${savedContextKey}:${base}`;
}

function readSavedContext(base = currentApiBase) {
  try {
    const scoped = JSON.parse(localStorage.getItem(scopedContextKey(base)) || "null");
    if (scoped) return scoped;
    const latest = JSON.parse(localStorage.getItem(savedContextKey) || "null");
    return !latest?.apiBase || latest.apiBase === base ? latest : null;
  } catch { return null; }
}

function abortViewRequests() {
  for (const requestInfo of viewRequests.values()) requestInfo.controller.abort();
  viewRequests.clear();
}

function beginViewRequest(viewName) {
  viewRequests.get(viewName)?.controller.abort();
  const controller = new AbortController();
  const signature = contextSignature();
  const token = Symbol(viewName);
  viewRequests.set(viewName, { controller, token });
  return {
    signal: controller.signal,
    isCurrent: () => viewRequests.get(viewName)?.token === token && contextSignature() === signature,
  };
}

function cacheKey(path, base = currentApiBase) {
  return `${base}|${contextSignature()}|${path}`;
}

function cacheRead(key) {
  const entry = cache.get(key);
  if (!entry) return null;
  if (Date.now() - entry.createdAt > cacheTtlMs) {
    cache.delete(key);
    return null;
  }
  cache.delete(key);
  cache.set(key, entry);
  return JSON.parse(entry.serialized);
}

function cacheWrite(key, value) {
  const serialized = JSON.stringify(value);
  if (serialized.length > cacheMaxEntryChars) return;
  cache.delete(key);
  cache.set(key, { serialized, createdAt: Date.now() });
  while (cache.size > cacheMaxEntries) cache.delete(cache.keys().next().value);
}

function invalidateCache(base = null) {
  if (!base) {
    cache.clear();
    return;
  }
  for (const key of cache.keys()) if (key.startsWith(`${base}|`)) cache.delete(key);
}

function apiPath(base, path) {
  if (/^https?:\/\//i.test(path)) return path;
  if (path.startsWith(`${base}/`) || path.startsWith("/api/")) return path;
  return `${base}${path.startsWith("/") ? path : `/${path}`}`;
}

async function request(path, options = {}, base = currentApiBase) {
  const route = String(path).split("?", 1)[0];
  if (apiCompatible === false && route !== "/health" && route !== "/restores") {
    const error = new Error("Las solicitudes están bloqueadas hasta que el servicio declare la API 1 compatible. El borrador local sigue disponible para editar y exportar.");
    error.code = "api_incompatible";
    throw error;
  }
  const method = String(options.method ?? "GET").toUpperCase();
  const responseType = options.responseType ?? "json";
  const canCache = method === "GET" && responseType === "json" && !options.noCache;
  const key = canCache ? cacheKey(path, base) : null;
  if (key) {
    const cached = cacheRead(key);
    if (cached !== null) return cached;
  }
  const fetchOptions = { ...options };
  delete fetchOptions.responseType;
  delete fetchOptions.noCache;
  const headers = new Headers(options.headers ?? {});
  if (typeof options.body === "string" && !headers.has("content-type")) headers.set("content-type", "application/json");
  fetchOptions.headers = headers;
  const response = await fetch(apiPath(base, path), fetchOptions);
  if (response.status === 204) return null;
  let body;
  if (responseType === "blob") body = await response.blob();
  else if (responseType === "arrayBuffer") body = await response.arrayBuffer();
  else if (responseType === "text" || !response.headers.get("content-type")?.includes("json")) body = await response.text();
  else body = await response.json();
  if (!response.ok) {
    const message = typeof body === "string" ? body || `Solicitud fallida (${response.status})` : body?.message ?? `Solicitud fallida (${response.status})`;
    const error = new Error(message);
    error.status = response.status;
    error.code = typeof body === "object" ? body?.code : undefined;
    error.details = typeof body === "object" ? body?.details : undefined;
    throw error;
  }
  if (key && responseType === "json") cacheWrite(key, body);
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

function describeError(error) {
  if (error?.name === "AbortError") return "";
  if (error?.code === "api_incompatible") return error.message;
  if (["version_mismatch", "incompatible_version", "api_version_mismatch"].includes(error?.code)) {
    return `La versión de la aplicación y del servicio no es compatible. ${error.message}`;
  }
  return error?.message ?? "La solicitud no se pudo completar.";
}

function displayConnection(message, status) {
  const element = byId("connection");
  element.textContent = message;
  element.dataset.status = status;
}

function saveContext() {
  const context = selectedContext();
  if (context.worldId && context.branchId && Number.isSafeInteger(context.year)) {
    try {
      localStorage.setItem(savedContextKey, JSON.stringify(context));
      localStorage.setItem(scopedContextKey(), JSON.stringify(context));
    } catch { /* Storage may be disabled. */ }
  }
  updateContextLabels();
  if (Number.isSafeInteger(context.year)) byId("branch-cut-year").value = String(context.year);
}

function clearHistoricalResults() {
  const empty = (id, text) => byId(id).replaceChildren(makeText("p", "muted", text));
  empty("results", "El contexto cambió. Consulta el censo para ver el cierre elegido.");
  empty("territory-results", "El contexto cambió. Consulta la vista histórica para cargar territorio, cultura e instituciones.");
  empty("economy-results", "El contexto cambió. Consulta mercados y eventos para cargar el nuevo cierre.");
  empty("household-results", "El contexto cambió. Consulta los hogares del año elegido.");
  empty("person-record", "El contexto cambió. Vuelve a cargar personas para abrir una ficha.");
  empty("person-events", "El contexto cambió. Vuelve a abrir una ficha para consultar su cronología.");
  empty("family-tree", "El contexto cambió. El árbol genealógico anterior se ha retirado.");
  empty("comparison-results", "El contexto cambió. Ejecuta una nueva comparación para consultar los cierres elegidos.");
  byId("settlement-select").replaceChildren(new Option("Todos", ""));
  byId("person-select").replaceChildren(new Option("Selecciona una persona", ""));
  byId("record-person-select").replaceChildren(new Option("Consultar personas para elegir", ""));
  byId("record-settlement-select").replaceChildren(new Option("Consultar la vista histórica para elegir", ""));
  byId("load-more-people").disabled = true;
  byId("people-page-status").textContent = "Carga la vista histórica para llenar el selector.";
  peopleLoadedEnd = 0;
  peopleTotal = null;
  peoplePageContext = null;
  settlementNameMap = new Map();
  currentPersonDossier = null;
  byId("export-person").disabled = true;
  byId("export-person").dataset.kind = "";
  byId("export-person").dataset.id = "";
}

function updateContextLabels() {
  const context = selectedContext();
  const world = byId("world-select").selectedOptions[0]?.textContent;
  const branch = context.branchId || "sin rama";
  const year = Number.isSafeInteger(context.year) ? `año ${context.year}` : "sin año";
  const source = currentRestoreId ? "copia restaurada" : "biblioteca activa";
  const value = `${world && context.worldId ? world : "sin mundo"} · rama ${branch} · ${year} · ${source}`;
  document.querySelectorAll(".context-stamp").forEach((node) => { node.textContent = value; });
  byId("return-primary").disabled = currentApiBase === PRIMARY_API;
}

function reportPollError(error) {
  if (error?.name === "AbortError") return;
  if (error instanceof TypeError) {
    displayConnection("Servicio local desconectado · intentando reconectar", "offline");
    byId("progress-label").textContent = "Conexión interrumpida; el trabajo y el último año confirmado se conservan. Intentando reconectar…";
    return;
  }
  showMessage(describeError(error), true);
}

function worldLabel(world) {
  return world.name || world.world_name || world.world_id || world.id || "Mundo";
}

function listFrom(value, keys = ["items", "worlds", "branches", "restores"]) {
  if (Array.isArray(value)) return value;
  for (const key of keys) if (Array.isArray(value?.[key])) return value[key];
  return [];
}

function setOptions(select, placeholder, rows, getValue, getLabel, preferred = null) {
  const previous = preferred ?? select.value;
  select.replaceChildren(new Option(placeholder, ""));
  for (const row of rows) {
    const value = getValue(row);
    if (value != null && value !== "") select.add(new Option(String(getLabel(row)), String(value)));
  }
  if ([...select.options].some((option) => option.value === String(previous))) select.value = String(previous);
}

async function refreshDataSources(preferredBase = null) {
  const select = byId("data-source");
  restoreCatalogController?.abort();
  const controller = new AbortController();
  restoreCatalogController = controller;
  let restores = [];
  let restoreLoadError = null;
  try {
    let page = await request("/restores?limit=100&offset=0", { noCache: true, signal: controller.signal }, PRIMARY_API);
    restores = listFrom(page);
    let offset = Number(page.offset ?? 0) + restores.length;
    while (page.has_more === true) {
      page = await request(`/restores?limit=100&offset=${offset}`, { noCache: true, signal: controller.signal }, PRIMARY_API);
      const next = listFrom(page);
      if (!next.length) break;
      restores.push(...next);
      offset = Number(page.offset ?? offset) + next.length;
    }
    if (page.has_more === true) throw new Error("El catálogo de copias quedó incompleto: el servicio indicó más páginas sin devolver elementos.");
  } catch (error) {
    if (error?.name === "AbortError" || controller.signal.aborted) return;
    restoreLoadError = error;
  }
  if (restoreCatalogController !== controller) return;
  const availableCount = restores.filter((restore) => restore.available !== false).length;
  const unavailable = restores.filter((restore) => restore.available === false);
  byId("restore-status").textContent = restoreLoadError
    ? `No se pudo completar el catálogo de copias. ${describeError(restoreLoadError)} Pulsa Reconectar para intentarlo de nuevo.${restores.length ? ` Se recibieron ${restores.length} entradas antes del error.` : ""}`
    : restores.length
      ? `${availableCount} copia${availableCount === 1 ? "" : "s"} disponible${availableCount === 1 ? "" : "s"} en este navegador.${unavailable.length ? ` ${unavailable.length} copia${unavailable.length === 1 ? "" : "s"} no disponible${unavailable.length === 1 ? "" : "s"}; ${unavailable[0].error?.message ?? "el servicio indicó una incompatibilidad"}.` : ""}`
      : "Las copias completas importadas quedan disponibles en el selector de fuente.";
  select.replaceChildren(new Option("Biblioteca activa", PRIMARY_API));
  for (const restore of restores) {
    const id = restore.restored_id ?? restore.restore_id ?? restore.id;
    if (!id) continue;
    const base = restore.api_base ?? `${PRIMARY_API}/restores/${encodeURIComponent(id)}`;
    const label = restore.name ?? restore.world_name ?? restore.worlds?.[0]?.name ?? restore.label ?? `Copia ${id}`;
    const available = restore.available !== false;
    const option = new Option(`${label} · ${available ? "restaurada" : "no disponible"}`, base);
    option.dataset.restoreId = String(id);
    option.disabled = !available;
    if (restore.error?.message) option.title = restore.error.message;
    select.add(option);
  }
  const wanted = preferredBase ?? currentApiBase;
  const wantedOption = [...select.options].find((option) => option.value === wanted);
  if (wantedOption && !wantedOption.disabled) select.value = wanted;
  else {
    select.value = PRIMARY_API;
    if (wanted !== PRIMARY_API) showMessage(`La copia ${wantedOption?.dataset.restoreId ?? "guardada"} no está disponible. Revisa el error del origen o importa otra copia.`, true);
  }
  currentApiBase = select.value;
  currentRestoreId = select.selectedOptions[0]?.dataset.restoreId ?? null;
  restoreCatalogController = null;
}

async function activateDataSource(apiBase, restoredId, label = null) {
  await refreshDataSources(apiBase);
  const select = byId("data-source");
  if (![...select.options].some((option) => option.value === apiBase)) {
    const option = new Option(`${label ?? `Copia ${restoredId ?? "restaurada"}`} · restaurada`, apiBase);
    option.dataset.restoreId = restoredId ?? "";
    select.add(option);
  }
  select.value = apiBase;
  currentApiBase = apiBase;
  currentRestoreId = restoredId ?? select.selectedOptions[0]?.dataset.restoreId ?? null;
}

function renderWorldLibrary(worlds) {
  const host = byId("world-library");
  host.replaceChildren();
  if (!worlds.length) {
    host.append(makeText("p", "muted", "No hay mundos en esta fuente todavía. Prepara un escenario o restaura una copia."));
    return;
  }
  for (const world of worlds) {
    const id = world.world_id ?? world.id;
    if (!id) continue;
    const card = document.createElement("article");
    card.className = "world-card";
    const head = document.createElement("div");
    head.className = "world-card-main";
    head.append(makeText("h4", "", worldLabel(world)));
    const metadata = [
      world.branch_count == null ? null : `${world.branch_count} ramas`,
      world.latest_year == null ? "sin cierres" : `hasta ${world.latest_year}`,
      world.world_id ? `ID ${world.world_id}` : null,
    ].filter(Boolean).join(" · ");
    head.append(makeText("p", "muted", metadata || "Mundo guardado"));
    const actions = document.createElement("div");
    actions.className = "actions";
    const open = document.createElement("button");
    open.type = "button";
    open.textContent = "Abrir mundo";
    open.addEventListener("click", async () => {
      try {
        clearHistoricalResults();
        byId("world-select").value = String(id);
        await refreshBranches(false);
        saveContext();
        setView("history");
      } catch (error) {
        showMessage(describeError(error), true);
      }
    });
    const exportButton = document.createElement("button");
    exportButton.type = "button";
    exportButton.className = "secondary";
    exportButton.textContent = "Descargar escenario";
    exportButton.addEventListener("click", () => downloadWorldScenario(id).catch((error) => showMessage(describeError(error), true)));
    const restoreButton = document.createElement("button");
    restoreButton.type = "button";
    restoreButton.className = "secondary";
    restoreButton.textContent = "Abrir copia aislada";
    restoreButton.addEventListener("click", () => restoreWorldCopy(id).catch((error) => showMessage(describeError(error), true)));
    actions.append(open, exportButton, restoreButton);
    card.append(head, actions);
    host.append(card);
  }
}

function makeText(tag, className, text) {
  const element = document.createElement(tag);
  if (className) element.className = className;
  element.textContent = text == null ? "" : String(text);
  return element;
}

async function refreshWorlds(restore = true, preferredContext = null) {
  const previousSignature = contextSignature();
  const worlds = listFrom(await request("/worlds", { noCache: true }), ["items", "worlds"]);
  renderWorldLibrary(worlds);
  const select = byId("world-select");
  const saved = preferredContext ?? (restore ? readSavedContext() : null);
  const previousWorldId = saved?.worldId ?? select.value;
  setOptions(select, "Selecciona o crea un mundo", worlds, (item) => item.world_id ?? item.id, worldLabel, previousWorldId);
  await refreshBranches(Boolean(saved), saved);
  if (previousSignature !== contextSignature()) {
    abortViewRequests();
    clearHistoricalResults();
  }
}

async function refreshBranches(restore = true, preferredContext = null) {
  const worldId = byId("world-select").value;
  const select = byId("branch-select");
  select.replaceChildren(new Option("Selecciona una rama", ""));
  const leftSelect = byId("compare-left-branch");
  const rightSelect = byId("compare-right-branch");
  const saved = preferredContext ?? (restore ? readSavedContext() : null);
  if (!worldId) {
    setOptions(leftSelect, "Usar rama actual", [], (row) => row.branch_id, (row) => row.branch_id);
    setOptions(rightSelect, "Selecciona una rama", [], (row) => row.branch_id, (row) => row.branch_id);
    saveContext();
    return;
  }
  const branches = listFrom(await request(`/worlds/${encodeURIComponent(worldId)}/branches`), ["items", "branches"]);
  for (const branch of branches) {
    const id = branch.branch_id ?? branch.id;
    if (!id) continue;
    const head = branch.head_year == null ? "sin años" : `hasta ${branch.head_year}`;
    const parent = branch.parent_branch_id ? ` · hija de ${branch.parent_branch_id}` : "";
    select.add(new Option(`${id} (${head}${parent})`, id));
  }
  setOptions(leftSelect, "Usar rama actual", branches, (row) => row.branch_id ?? row.id, (row) => row.branch_id ?? row.id, "");
  setOptions(rightSelect, "Selecciona una rama", branches, (row) => row.branch_id ?? row.id, (row) => row.branch_id ?? row.id, saved?.branchId ?? "");
  const wanted = saved?.branchId;
  if (wanted && [...select.options].some((option) => option.value === String(wanted))) select.value = String(wanted);
  else if (!restore) select.value = branches.find((branch) => (branch.branch_id ?? branch.id) === "main")?.branch_id ?? branches[0]?.branch_id ?? branches[0]?.id ?? "";
  if (saved && Number.isSafeInteger(saved.year)) byId("year-input").value = String(saved.year);
  else {
    const branch = branches.find((item) => (item.branch_id ?? item.id) === select.value);
    const nextYear = branch?.head_year ?? currentScenario?.start_year ?? 0;
    byId("year-input").value = String(nextYear);
  }
  byId("compare-left-year").value = byId("year-input").value;
  byId("compare-right-year").value = byId("year-input").value;
  byId("branch-cut-year").value = byId("year-input").value;
  saveContext();
}

function validateContext() {
  const context = selectedContext();
  if (!context.worldId || !context.branchId) throw new Error("Selecciona un mundo y una rama.");
  if (!Number.isSafeInteger(context.year)) throw new Error("Selecciona un año entero seguro.");
  return context;
}

function asOfQuery(context, extra = {}) {
  return new URLSearchParams({ year: String(context.year), limit: String(pageLimit), ...extra });
}

function resolvePageTotal(rawTotal, loadedEnd, pageCount, pageSize) {
  if (rawTotal !== undefined && rawTotal !== null && Number.isFinite(Number(rawTotal))) return Number(rawTotal);
  return pageCount < pageSize ? loadedEnd : null;
}

function pageCountLabel(loadedEnd, total, itemLabel) {
  if (total == null) return `Mostrando ${loadedEnd} ${itemLabel}; total pendiente.`;
  if (total === 0) return `No hay ${itemLabel}.`;
  return `Mostrando ${Math.min(loadedEnd, total)} de ${total} ${itemLabel}${loadedEnd >= total ? " · fin de lista" : ""}.`;
}

function addPageControl(section, page, itemLabel, nextPage, appendRows) {
  if (!page || typeof nextPage !== "function") return;
  const rawTotal = page.total ?? page.item_total ?? page.total_items ?? null;
  let offset = Number(page.offset ?? 0);
  let visible = Number(page.progressCount ?? page.visibleCount ?? page.items?.length ?? page.rows?.length ?? 0);
  let loadedEnd = offset + visible;
  const pageSize = Number(page.limit ?? pageLimit);
  let total = resolvePageTotal(rawTotal, loadedEnd, visible, pageSize);
  const summary = makeText("p", "muted page-count", pageCountLabel(loadedEnd, total, itemLabel));
  section.append(summary);
  const hasMore = total == null ? visible >= pageSize : loadedEnd < total;
  if (!hasMore || visible === 0) return;
  const button = document.createElement("button");
  button.type = "button";
  button.className = "secondary load-more";
  button.textContent = `Cargar siguientes ${page.limit ?? pageLimit}`;
  button.addEventListener("click", async () => {
    button.disabled = true;
    button.textContent = "Cargando…";
    try {
      const nextOffset = loadedEnd;
      const next = await nextPage(nextOffset);
      const rows = next.rows ?? next.items ?? next.settlements ?? next.people ?? next.households ?? [];
      appendRows(rows);
      offset = Number(next.offset ?? nextOffset);
      visible = Number(next.progressCount ?? rows.length);
      loadedEnd = offset + visible;
      const nextRawTotal = next.total ?? next.item_total ?? next.total_items ?? total;
      const nextLimit = Number(next.limit ?? page.limit ?? pageLimit);
      const nextTotal = resolvePageTotal(nextRawTotal, loadedEnd, rows.length, nextLimit);
      total = nextTotal;
      summary.textContent = pageCountLabel(loadedEnd, nextTotal, itemLabel);
      const nextHasMore = nextTotal == null ? rows.length >= nextLimit : loadedEnd < nextTotal;
      if (!nextHasMore || rows.length === 0) button.remove();
      else {
        button.disabled = false;
        button.textContent = `Cargar siguientes ${next.limit ?? page.limit ?? pageLimit}`;
      }
    } catch (error) {
      button.disabled = false;
      button.textContent = "Reintentar carga";
      if (error.name !== "AbortError") showMessage(describeError(error), true);
    }
  });
  section.append(button);
}

function appendTable(host, title, columns, rows, page = null, nextPage = null, itemLabel = "resultados") {
  const section = document.createElement("section");
  section.className = "table-section";
  section.append(makeText("h3", "", title));
  const scroll = document.createElement("div");
  scroll.className = "table-scroll";
  const table = document.createElement("table");
  const head = document.createElement("thead");
  const headerRow = document.createElement("tr");
  for (const column of columns) headerRow.append(makeText("th", "", column.label));
  head.append(headerRow);
  const body = document.createElement("tbody");
  const appendRows = (items) => {
    for (const row of items) {
      const tr = document.createElement("tr");
      for (const column of columns) {
        const cell = document.createElement("td");
        const value = column.render ? column.render(row) : column.value(row);
        if (value instanceof Node) cell.append(value);
        else cell.textContent = value == null ? "—" : typeof value === "object" ? JSON.stringify(value) : String(value);
        tr.append(cell);
      }
      body.append(tr);
    }
  };
  appendRows(rows ?? []);
  table.append(head, body);
  scroll.append(table);
  section.append(scroll);
  if (!(rows ?? []).length) section.append(makeText("p", "muted", "Sin datos para este año."));
  addPageControl(section, page, itemLabel, nextPage, appendRows);
  host.append(section);
  return section;
}

function renderCensus(value, nextPage = null, append = false) {
  const host = byId("results");
  const rows = value?.settlements ?? value?.items ?? [];
  if (!append) {
    host.replaceChildren();
    host.append(makeText("p", "muted", `Censo · rama ${value?.branch_id ?? selectedContext().branchId ?? "desconocida"} · año ${value?.year ?? selectedContext().year}.`));
  }
  const target = append ? host.querySelector(".table-section") : null;
  const columns = [
    { label: "Asentamiento", value: (row) => row.settlement_name ?? row.name ?? row.settlement_id ?? row.id },
    { label: "Habitantes vivos", value: (row) => row.population ?? row.living_population ?? row.count },
  ];
  if (append && target) {
    const body = target.querySelector("tbody");
    for (const row of rows) {
      const tr = document.createElement("tr");
      columns.forEach((column) => tr.insertCell().textContent = String(column.value(row) ?? "—"));
      body.append(tr);
    }
  } else {
    appendTable(host, "Población por asentamiento", columns, rows, {
      total: value?.total ?? value?.total_settlements ?? value?.item_total, offset: value?.offset ?? 0, visibleCount: rows.length, limit: value?.limit ?? pageLimit,
    }, nextPage, "asentamientos");
  }
}

async function loadCensus(offset = 0, append = false) {
  const context = validateContext();
  saveContext();
  const view = beginViewRequest("census");
  const base = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}`;
  const query = asOfQuery(context, { offset: String(offset) });
  const census = await request(`${base}/census?${query}`, { signal: view.signal });
  if (!view.isCurrent()) return;
  const next = async (nextOffset) => {
    const more = await request(`${base}/census?${asOfQuery(context, { offset: String(nextOffset) })}`, { signal: view.signal });
    return { ...more, rows: more.settlements ?? more.items ?? [] };
  };
  renderCensus(census, next, append);
}

function pageData(page, rows, itemLabel) {
  return { total: page?.total ?? page?.item_total ?? page?.total_items ?? null, offset: page?.offset ?? 0, limit: page?.limit ?? pageLimit, visibleCount: rows.length, itemLabel };
}

function tableNext(pathForOffset, mapper = (page) => page.items ?? [], signal = undefined) {
  return async (offset) => {
    const page = await request(pathForOffset(offset), { signal });
    return { ...page, rows: mapper(page) };
  };
}

function setSelectRows(select, placeholder, rows, value, label) {
  setOptions(select, placeholder, rows, value, label, select.value);
}

async function loadE2() {
  const context = validateContext();
  saveContext();
  const view = beginViewRequest("territory");
  const base = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}`;
  const offsetQuery = { offset: "0" };
  const settlementId = byId("settlement-select").value;
  const personId = byId("person-select").value;
  const filter = settlementId ? { settlement_id: settlementId } : {};
  const query = (extra = {}) => asOfQuery(context, { ...offsetQuery, ...filter, ...extra });
  const [settlements, routes, migration, culture, institutions, organizations, people, census] = await Promise.all([
    request(`${base}/settlements?${query()}`, { signal: view.signal }),
    request(`${base}/routes?${query()}`, { signal: view.signal }),
    request(`${base}/migration?${query()}`, { signal: view.signal }),
    request(`${base}/culture?${query()}`, { signal: view.signal }),
    request(`${base}/institutions?${query()}`, { signal: view.signal }),
    request(`${base}/organizations?${query()}`, { signal: view.signal }),
    request(`${base}/people?${query()}`, { signal: view.signal }),
    request(`${base}/census?${query()}`, { signal: view.signal }),
  ]);
  if (!view.isCurrent()) return;
  const settlementRows = settlements.items ?? [];
  const names = new Map(settlementRows.map((item) => [String(item.settlement_id), item.settlement_name]));
  settlementNameMap = names;
  const peopleRows = people.people ?? people.items ?? [];
  const personData = peopleRows.map((item) => item.person ?? item);
  setSelectRows(byId("settlement-select"), "Todos los asentamientos", settlementRows, (item) => item.settlement_id, (item) => item.settlement_name);
  setSelectRows(byId("person-select"), "Selecciona una persona", personData, (item) => item.id ?? item.person_id, (item) => `Persona ${item.id ?? item.person_id}`);
  setSelectRows(byId("record-person-select"), "Selecciona una persona", personData, (item) => item.id ?? item.person_id, (item) => `Persona ${item.id ?? item.person_id}`);
  setOptions(byId("record-settlement-select"), "Selecciona un asentamiento", settlementRows, (item) => item.settlement_id, (item) => item.settlement_name);
  peopleLoadedEnd = Number(people.offset ?? 0) + peopleRows.length;
  const peopleLimit = Number(people.limit ?? pageLimit);
  peopleTotal = resolvePageTotal(people.total ?? people.total_people, peopleLoadedEnd, peopleRows.length, peopleLimit);
  peoplePageContext = contextSignature();
  byId("people-page-status").textContent = peopleTotal == null
    ? `Cargadas ${peopleLoadedEnd} personas; total pendiente.`
    : `Cargadas ${Math.min(peopleLoadedEnd, peopleTotal)} de ${peopleTotal} personas${peopleLoadedEnd >= peopleTotal ? " · fin de lista." : "."}`;
  byId("load-more-people").disabled = peopleTotal == null ? peopleRows.length < peopleLimit : peopleLoadedEnd >= peopleTotal;
  const host = byId("territory-results");
  host.replaceChildren();
  const selectedSettlement = settlementId ? names.get(String(settlementId)) ?? settlementId : "todos los asentamientos";
  host.append(makeText("p", "muted", `Vista territorial · rama ${context.branchId} · año ${context.year} · ${selectedSettlement}.`));

  const nextCensus = async (offset) => {
    const more = await request(`${base}/census?${query({ offset: String(offset) })}`, { signal: view.signal });
    return { ...more, rows: more.settlements ?? more.items ?? [] };
  };
  renderCensus(census, nextCensus);
  appendTable(host, "Rutas abiertas al cierre", [
    { label: "Origen", value: (row) => names.get(String(row.from)) ?? row.from },
    { label: "Destino", value: (row) => names.get(String(row.to)) ?? row.to },
    { label: "Coste", value: (row) => row.cost },
    { label: "Capacidad anual", value: (row) => row.capacity_per_year ?? "Sin límite" },
  ], routes.items ?? [], pageData(routes, routes.items ?? [], "rutas"), tableNext((offset) => `${base}/routes?${query({ offset: String(offset) })}`, (page) => page.items ?? [], view.signal), "rutas");
  appendTable(host, "Balance anual por asentamiento", [
    { label: "Asentamiento", value: (row) => names.get(String(row.settlement_id)) ?? row.settlement_id },
    { label: "Antes", value: (row) => row.before }, { label: "Nacimientos", value: (row) => row.births },
    { label: "Defunciones", value: (row) => row.deaths }, { label: "Llegadas", value: (row) => row.incoming },
    { label: "Salidas", value: (row) => row.outgoing }, { label: "Después", value: (row) => row.after },
  ], migration.items ?? [], pageData(migration, migration.items ?? [], "asentamientos"), tableNext((offset) => `${base}/migration?${query({ offset: String(offset) })}`, (page) => page.items ?? [], view.signal), "asentamientos");
  const mapCulture = (page) => (page.items ?? []).flatMap((settlement) => (settlement.traits ?? []).map((trait) => ({
    settlement: settlement.settlement_name, trait: String(trait.key), value: trait.current_value, members: trait.member_count, weight: trait.total_weight,
  })));
  appendTable(host, "Valores culturales agregados", [
    { label: "Asentamiento", value: (row) => row.settlement }, { label: "Dimensión", value: (row) => row.trait },
    { label: "Valor medio", value: (row) => row.value }, { label: "Miembros", value: (row) => row.members }, { label: "Peso total", value: (row) => row.weight },
  ], mapCulture(culture), pageData(culture, culture.items ?? [], "asentamientos con valores culturales"), tableNext((offset) => `${base}/culture?${query({ offset: String(offset) })}`, mapCulture, view.signal), "asentamientos con valores culturales");
  appendTable(host, "Instituciones y religiones", [
    { label: "Nombre", value: (row) => row.definition?.name }, { label: "Tipo", value: (row) => row.definition?.kind === "religion" ? "Religión" : "Institución" },
    { label: "Afiliaciones activas", value: (row) => row.active_affiliation_count },
  ], institutions.items ?? [], pageData(institutions, institutions.items ?? [], "instituciones o religiones"), tableNext((offset) => `${base}/institutions?${query({ offset: String(offset) })}`, (page) => page.items ?? [], view.signal), "instituciones o religiones");
  appendTable(host, "Organizaciones", [
    { label: "Nombre", value: (row) => row.organization?.name }, { label: "Asentamiento", value: (row) => names.get(String(row.organization?.settlement_id)) ?? row.organization?.settlement_id },
    { label: "Fundadores", value: (row) => row.organization?.founders?.length ?? 0 },
    { label: "Valores culturales", value: (row) => (row.organization?.culture_values ?? []).map((item) => `${String(item.key)}=${item.current_value ?? "—"}`).join(", ") || "—" },
    { label: "Miembros activos", value: (row) => row.active_member_count }, { label: "Estado", value: (row) => row.active_at_close ? "Activa" : "Disuelta" },
  ], organizations.items ?? [], pageData(organizations, organizations.items ?? [], "organizaciones"), tableNext((offset) => `${base}/organizations?${query({ offset: String(offset) })}`, (page) => page.items ?? [], view.signal), "organizaciones");
  if (personId) {
    const [residence, affiliations] = await Promise.all([
      request(`${base}/people/${encodeURIComponent(personId)}/residence?${query()}`, { signal: view.signal }),
      request(`${base}/people/${encodeURIComponent(personId)}/affiliations?${query()}`, { signal: view.signal }),
    ]);
    if (!view.isCurrent()) return;
    appendTable(host, `Residencia y profesiones de persona ${personId}`, [
      { label: "Tipo", value: (row) => row.type }, { label: "Lugar o profesión", value: (row) => row.name },
      { label: "Desde", value: (row) => row.start }, { label: "Hasta (exclusivo)", value: (row) => row.end ?? "Vigente" },
    ], [
      ...(residence.residences ?? []).map((item) => ({ type: "Residencia", name: names.get(String(item.settlement_id)) ?? item.settlement_id, start: item.validity.start, end: item.validity.end_exclusive })),
      ...(residence.professions ?? []).map((item) => ({ type: "Profesión", name: item.profession_id, start: item.validity.start, end: item.validity.end_exclusive })),
    ], null, null, "intervalos");
    appendTable(host, "Afiliaciones históricas", [
      { label: "Tipo", value: (row) => row.type }, { label: "Institución u organización", value: (row) => row.name },
      { label: "Lugar de anclaje", value: (row) => names.get(String(row.settlement_id)) ?? row.settlement_id },
      { label: "Desde", value: (row) => row.start }, { label: "Hasta (exclusivo)", value: (row) => row.end ?? "Vigente" },
    ], [
      ...(affiliations.institutions ?? []).map((item) => ({ type: "Institución", name: item.institution_id, settlement_id: item.settlement_id, start: item.validity.start, end: item.validity.end_exclusive })),
      ...(affiliations.organizations ?? []).map((item) => ({ type: "Organización", name: item.organization_name, settlement_id: item.settlement_id, start: item.validity.start, end: item.validity.end_exclusive })),
    ], null, null, "afiliaciones");
  }
  byId("load-e2").setAttribute("aria-label", `Consultar vista histórica de ${context.branchId}, año ${context.year}`);
}

async function loadMorePeople() {
  const context = validateContext();
  if (peoplePageContext !== contextSignature()) {
    await loadE2();
    showMessage("El selector se actualizó para el contexto nuevo. Vuelve a cargar más personas si hace falta.");
    return;
  }
  const button = byId("load-more-people");
  if (button.disabled) return;
  button.disabled = true;
  button.textContent = "Cargando…";
  const view = beginViewRequest("people-page");
  try {
    const base = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}`;
    const page = await request(`${base}/people?${asOfQuery(context, { offset: String(peopleLoadedEnd) })}`, { signal: view.signal });
    if (!view.isCurrent()) return;
    const rows = (page.people ?? page.items ?? []).map((item) => item.person ?? item);
    for (const person of rows) {
      const id = person.id ?? person.person_id;
      if (!id) continue;
      for (const select of [byId("person-select"), byId("record-person-select")]) {
        if (![...select.options].some((option) => option.value === String(id))) select.add(new Option(`Persona ${id}`, String(id)));
      }
    }
    const limit = Number(page.limit ?? pageLimit);
    peopleLoadedEnd = Number(page.offset ?? peopleLoadedEnd) + rows.length;
    peopleTotal = resolvePageTotal(page.total ?? page.total_people ?? peopleTotal, peopleLoadedEnd, rows.length, limit);
    byId("people-page-status").textContent = peopleTotal == null
      ? `Cargadas ${peopleLoadedEnd} personas; total pendiente.`
      : `Cargadas ${Math.min(peopleLoadedEnd, peopleTotal)} de ${peopleTotal} personas${peopleLoadedEnd >= peopleTotal ? " · fin de lista." : "."}`;
    button.disabled = peopleTotal == null ? rows.length < limit : peopleLoadedEnd >= peopleTotal;
  } catch (error) {
    button.disabled = false;
    throw error;
  } finally {
    button.textContent = "Cargar más personas";
  }
}

async function loadRecordPeople() {
  const context = validateContext();
  saveContext();
  const view = beginViewRequest("people-page");
  const base = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}`;
  const page = await request(`${base}/people?${asOfQuery(context, { offset: "0" })}`, { signal: view.signal });
  if (!view.isCurrent()) return;
  const rows = (page.people ?? page.items ?? []).map((item) => item.person ?? item);
  const selectRows = (select) => setSelectRows(select, "Selecciona una persona", rows,
    (item) => item.id ?? item.person_id,
    (item) => `Persona ${item.id ?? item.person_id}`);
  selectRows(byId("record-person-select"));
  selectRows(byId("person-select"));
  peopleLoadedEnd = Number(page.offset ?? 0) + rows.length;
  const limit = Number(page.limit ?? pageLimit);
  peopleTotal = resolvePageTotal(page.total ?? page.total_people, peopleLoadedEnd, rows.length, limit);
  peoplePageContext = contextSignature();
  byId("people-page-status").textContent = peopleTotal == null
    ? `Cargadas ${peopleLoadedEnd} personas; total pendiente.`
    : `Cargadas ${Math.min(peopleLoadedEnd, peopleTotal)} de ${peopleTotal} personas${peopleLoadedEnd >= peopleTotal ? " · fin de lista." : "."}`;
  byId("load-more-people").disabled = peopleTotal == null ? rows.length < limit : peopleLoadedEnd >= peopleTotal;
  showMessage(`Selector de personas actualizado para ${context.branchId}, año ${context.year}.`);
}

function describeEvent(event) {
  const kind = event?.kind ?? {};
  const data = kind.data ?? {};
  if (kind.type === "scheduled_event_occurred") return `Inicio: ${data.name ?? "evento"}`;
  if (kind.type === "event_effect_realized") return `${data.event_name ?? "Evento"} · efecto ${Number(data.effect_index ?? 0) + 1} · ${data.effect?.type ? enumLabelFor(String(data.effect.type)) : "resultado"}`;
  if (kind.type === "economic_year_closed") return "Cierre económico";
  return String(kind.type ?? "Suceso").replaceAll("_", " ");
}

async function loadE3() {
  const context = validateContext();
  saveContext();
  const view = beginViewRequest("economy");
  const base = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}`;
  const settlementId = byId("settlement-select").value;
  const extra = settlementId ? { settlement_id: settlementId } : {};
  const query = (offset = 0) => asOfQuery(context, { offset: String(offset), ...extra });
  const [marketPage, settlementMarkets, trades, events, settlements] = await Promise.all([
    request(`${base}/markets?${query()}`, { signal: view.signal }), request(`${base}/markets/settlements?${query()}`, { signal: view.signal }),
    request(`${base}/markets/trades?${query()}`, { signal: view.signal }), request(`${base}/events?${query()}`, { signal: view.signal }),
    request(`${base}/settlements?${query()}`, { signal: view.signal }),
  ]);
  if (!view.isCurrent()) return;
  const names = new Map((settlements.items ?? []).map((row) => [String(row.settlement_id), row.settlement_name]));
  const host = byId("economy-results");
  host.replaceChildren();
  const selectedName = settlementId ? names.get(String(settlementId)) ?? settlementId : "todos los asentamientos";
  host.append(makeText("p", "muted", `Mercados · rama ${context.branchId} · año ${context.year} · ${selectedName}. Los sucesos son resultados realizados hasta este cierre.`));
  const mapGroups = (page) => (page.groups ?? []).flatMap((group) => {
    const goods = Object.entries(group.goods ?? {});
    if (!goods.length) return [{ settlement_id: group.group_id.settlement_id, profession: group.group_id.profession_id ?? "Consumidores", members: group.member_count_after, money: group.closing_money, good: "—", stock: "—" }];
    return goods.map(([good, ledger]) => ({ settlement_id: group.group_id.settlement_id, profession: group.group_id.profession_id ?? "Consumidores", members: group.member_count_after, money: group.closing_money, good, stock: ledger.closing_stock, produced: ledger.produced, unmet: ledger.unmet_physical_need }));
  });
  appendTable(host, marketPage.enabled ? "Saldos económicos por grupo" : "Economía", [
    { label: "Asentamiento", value: (row) => names.get(String(row.settlement_id)) ?? row.settlement_id }, { label: "Profesión", value: (row) => row.profession },
    { label: "Miembros", value: (row) => row.members }, { label: "Dinero al cierre", value: (row) => row.money }, { label: "Bien", value: (row) => row.good },
    { label: "Existencias", value: (row) => row.stock }, { label: "Producción", value: (row) => row.produced }, { label: "Necesidad sin cubrir", value: (row) => row.unmet },
  ], mapGroups(marketPage), pageData(marketPage, marketPage.groups ?? [], "grupos económicos"), tableNext((offset) => `${base}/markets?${query(offset)}`, mapGroups, view.signal), "grupos económicos");
  if (marketPage.money) appendTable(host, "Balance monetario mundial", [
    { label: "Concepto", value: (row) => row.name }, { label: "Importe", value: (row) => row.value },
  ], Object.entries(marketPage.money).map(([name, value]) => ({ name: name.replaceAll("_", " "), value })));
  appendTable(host, "Existencias de tesorería", [
    { label: "Bien", value: (row) => row.good }, { label: "Apertura", value: (row) => row.opening_stock }, { label: "Transferencias recibidas", value: (row) => row.transfers_in },
    { label: "Transferencias enviadas", value: (row) => row.transfers_out }, { label: "Destrucción", value: (row) => row.event_destroyed }, { label: "Cierre", value: (row) => row.closing_stock },
  ], Object.entries(marketPage.treasury_goods ?? {}).map(([good, ledger]) => ({ good, ...ledger })));
  const mapSettlementGoods = (page) => (page.settlements ?? []).flatMap((settlement) => (settlement.goods ?? []).map((good) => ({ settlement_id: settlement.settlement_id, ...good })));
  appendTable(host, "Necesidad, demanda y stock por bien", [
    { label: "Asentamiento", value: (row) => names.get(String(row.settlement_id)) ?? row.settlement_id }, { label: "Bien", value: (row) => row.good_id },
    { label: "Precio inicial", value: (row) => row.opening_price }, { label: "Precio siguiente", value: (row) => row.next_price }, { label: "Necesidad física", value: (row) => row.physical_need },
    { label: "Necesidad sin cubrir", value: (row) => row.unmet_physical_need }, { label: "Demanda solvente", value: (row) => row.solvent_demand },
    { label: "Insumos pendientes", value: (row) => row.unmet_input_demand }, { label: "Oferta sin vender", value: (row) => row.unsold_offer }, { label: "Importaciones", value: (row) => row.imports },
    { label: "Exportaciones", value: (row) => row.exports }, { label: "Existencias finales", value: (row) => row.ending_stock },
  ], mapSettlementGoods(settlementMarkets), pageData(settlementMarkets, settlementMarkets.settlements ?? [], "asentamientos"), tableNext((offset) => `${base}/markets/settlements?${query(offset)}`, mapSettlementGoods, view.signal), "asentamientos");
  appendTable(host, "Intercambios realizados", [
    { label: "Vendedor", value: (row) => `${names.get(String(row.source_group?.settlement_id)) ?? row.source_group?.settlement_id} · ${row.source_group?.profession_id ?? "Consumidores"}` },
    { label: "Comprador", value: (row) => `${names.get(String(row.destination_group?.settlement_id)) ?? row.destination_group?.settlement_id} · ${row.destination_group?.profession_id ?? "Consumidores"}` },
    { label: "Bien", value: (row) => row.good_id }, { label: "Cantidad", value: (row) => row.quantity }, { label: "Pago", value: (row) => row.buyer_payment },
    { label: "Transporte", value: (row) => row.transport_cost }, { label: "Motivo", value: (row) => row.purpose === "production_input" ? "Insumos" : "Necesidad" },
  ], trades.trades ?? [], pageData(trades, trades.trades ?? [], "intercambios"), tableNext((offset) => `${base}/markets/trades?${query(offset)}`, (page) => page.trades ?? [], view.signal), "intercambios");
  appendTable(host, "Eventos y efectos realizados", [
    { label: "Año", value: (row) => row.year }, { label: "Orden", value: (row) => row.order }, { label: "Resultado", value: describeEvent },
  ], events.events ?? [], pageData(events, events.events ?? [], "eventos"), tableNext((offset) => `${base}/events?${query(offset)}`, (page) => page.events ?? [], view.signal), "eventos");
}

function setJobControls(status) {
  byId("run-status").textContent = runStatusLabels[status] ?? "Estado desconocido";
  byId("run-status").dataset.status = status;
  const active = ["queued", "running", "paused"].includes(status);
  byId("pause-run").disabled = !activeJobId || status !== "running" || pauseRequested;
  byId("resume-run").disabled = !activeJobId || status !== "paused";
  byId("cancel-run").disabled = !activeJobId || !active;
  byId("start-run").disabled = active;
}

async function pollJob() {
  if (!activeJobId) return;
  const job = await request(`/runs/${encodeURIComponent(activeJobId)}`, { noCache: true });
  setJobControls(job.status);
  const done = Number(job.completed_years ?? 0);
  const total = Number(job.total_years ?? 1);
  byId("run-progress").max = Math.max(total, 1);
  byId("run-progress").value = Math.min(done, total);
  byId("progress-label").textContent = pauseRequested && job.status === "running"
    ? "Pausa solicitada; el trabajo confirmará la pausa al terminar el año actual."
    : job.current_year == null
      ? `Estado: ${runStatusLabels[job.status] ?? "Desconocido"}. ${job.message ?? ""}`
      : `Año ${job.current_year} confirmado · ${done} de ${total} pasos. ${job.message ?? ""}`;
  if (["paused", "completed", "cancelled", "failed"].includes(job.status)) pauseRequested = false;
  if (["completed", "cancelled", "failed"].includes(job.status)) {
    clearInterval(activePoll);
    activePoll = null;
    activeEvents?.close();
    activeEvents = null;
    activeJobId = null;
    try { localStorage.removeItem(savedJobKey); } catch { /* Ignore storage errors. */ }
    setJobControls(job.status);
    if (job.status === "completed") {
      invalidateCache();
      await loadCensus().catch(() => {});
    }
    return;
  }
  try { localStorage.setItem(savedJobKey, JSON.stringify({ jobId: activeJobId, apiBase: currentApiBase })); } catch { /* Ignore storage errors. */ }
}

function listenForRunEvents(jobId) {
  if (typeof EventSource !== "function") return;
  const eventsUrl = apiPath(currentApiBase, `/runs/${encodeURIComponent(jobId)}/progress`);
  try {
    activeEvents = new EventSource(eventsUrl);
    activeEvents.addEventListener("progress", (event) => {
      try {
        const progress = JSON.parse(event.data);
        const done = Number(progress.completed_years ?? 0);
        const total = Number(progress.total_years ?? 1);
        byId("run-progress").max = Math.max(total, 1);
        byId("run-progress").value = Math.min(done, total);
        byId("progress-label").textContent = progress.message ?? `Año ${progress.current_year ?? "—"} · ${done} de ${total} pasos.`;
        if (progress.status) setJobControls(progress.status);
      } catch { /* Polling remains the recovery path if an event is malformed. */ }
    });
    activeEvents.onerror = () => { activeEvents?.close(); activeEvents = null; };
  } catch { activeEvents = null; }
}

async function startRun() {
  const context = validateContext();
  const throughYear = Number(byId("through-year").value);
  if (!Number.isSafeInteger(throughYear)) throw new Error("El año final debe ser un entero seguro.");
  const job = await request(`/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}/runs`, {
    method: "POST", body: JSON.stringify({ through_year: throughYear }),
  });
  invalidateCache();
  activeJobId = job.job_id;
  pauseRequested = false;
  try { localStorage.setItem(savedJobKey, JSON.stringify({ jobId: activeJobId, apiBase: currentApiBase })); } catch { /* Ignore storage errors. */ }
  byId("run-progress").value = 0;
  setJobControls(job.status ?? "queued");
  listenForRunEvents(activeJobId);
  clearInterval(activePoll);
  activePoll = setInterval(() => pollJob().catch((error) => {
    reportPollError(error);
  }), 1200);
  await pollJob();
}

function renderValidation(result) {
  const host = byId("validation-results");
  host.replaceChildren();
  if (result.valid) {
    host.append(makeText("p", "notice", "Escenario válido. Puedes guardarlo como un mundo."));
    return;
  }
  const issues = result.issues ?? [];
  host.append(makeText("p", "notice error", `El escenario tiene ${issues.length} problemas. Revisa las rutas indicadas.`));
  if (!issues.length) return;
  const list = document.createElement("ul");
  list.className = "issue-list";
  for (const issue of issues) {
    const item = document.createElement("li");
    item.append(makeText("code", "", issue.path || "escenario"), makeText("span", "", `${issue.code ? `${issue.code}: ` : ""}${issue.message ?? "Valor no válido"}`));
    list.append(item);
  }
  host.append(list);
}

async function parseScenario() {
  if (!currentScenario) throw new Error("Carga o abre un escenario primero.");
  const validation = await request("/scenarios/validate", { method: "POST", body: JSON.stringify({ scenario: currentScenario }), noCache: true });
  renderValidation(validation);
  return validation;
}

async function loadScenarioSchema(version) {
  if (schemas.has(version)) return schemas.get(version);
  try {
    const response = await request(`/scenarios/schema?version=${encodeURIComponent(version)}`, { noCache: true });
    const schema = response.schema ?? response.json_schema ?? response;
    schemas.set(version, schema);
    return schema;
  } catch {
    return null;
  }
}

function updateEditorFromChange(changed) {
  const oldVersion = currentScenario?.schema_version;
  currentScenario = changed;
  byId("scenario-json").value = JSON.stringify(currentScenario, null, 2);
  byId("scenario-version").textContent = `Esquema ${currentScenario.schema_version ?? "?"} · reglas ${currentScenario.rules_version ?? "?"}`;
  byId("validation-results").replaceChildren();
  persistScenarioDraft();
  renderAgenda();
  const version = currentScenario.schema_version ?? 1;
  if (version !== oldVersion) {
    loadScenarioSchema(version).then((schema) => {
      if (currentScenario?.schema_version !== version) return;
      currentSchema = schema;
      renderScenarioForm(byId("scenario-form"), currentScenario, updateEditorFromChange, currentSchema);
    });
  }
}

function persistScenarioDraft() {
  if (!currentScenario) return;
  try {
    localStorage.setItem(savedDraftKey, JSON.stringify({ saved_at: new Date().toISOString(), scenario: currentScenario }));
    byId("draft-status").textContent = `Borrador guardado en este navegador · ${new Date().toLocaleTimeString("es-ES", { hour: "2-digit", minute: "2-digit" })}.`;
  } catch {
    byId("draft-status").textContent = "El almacenamiento del navegador no está disponible; descarga una copia JSON para guardar este diseño.";
  }
}

function renderAgenda() {
  const host = byId("agenda-preview");
  host.replaceChildren();
  const events = currentScenario?.scheduled_events ?? [];
  if (!events.length) {
    host.append(makeText("p", "muted", "La agenda no tiene eventos programados."));
    return;
  }
  host.append(makeText("p", "muted", "Estos eventos son futuros del escenario. No son sucesos históricos hasta que una corrida los confirme."));
  appendTable(host, "Agenda futura", [
    { label: "Nombre", value: (event) => event.name ?? event.id }, { label: "Inicio", value: (event) => event.start_year },
    { label: "Duración", value: (event) => event.duration_years ?? "Un año" }, { label: "Prioridad", value: (event) => event.priority },
    { label: "Efectos", value: (event) => (event.effects ?? []).map((effect) => enumLabelFor(String(effect.type ?? "efecto"))).join(", ") },
  ], events, null, null, "eventos futuros");
}

async function setScenario(scenario, saveDraft = true) {
  if (!scenario || typeof scenario !== "object" || Array.isArray(scenario)) throw new Error("El escenario debe ser un objeto JSON.");
  currentScenario = scenario;
  currentSchema = await loadScenarioSchema(scenario.schema_version ?? 1);
  byId("scenario-json").value = JSON.stringify(currentScenario, null, 2);
  byId("scenario-version").textContent = `Esquema ${scenario.schema_version ?? "?"} · reglas ${scenario.rules_version ?? "?"}`;
  renderScenarioForm(byId("scenario-form"), currentScenario, updateEditorFromChange, currentSchema);
  renderAgenda();
  if (saveDraft) persistScenarioDraft();
}

async function loadDemo(path) {
  const scenario = await request(path, { noCache: true });
  await setScenario(scenario);
  byId("through-year").value = String((scenario.start_year ?? 0) + 3);
  setView("scenario");
  showMessage("Escenario de demostración cargado. Revisa sus parámetros y valida antes de guardarlo.");
}

async function createWorld() {
  if (!currentScenario) throw new Error("Carga o importa un escenario primero.");
  const validation = await parseScenario();
  if (!validation.valid) throw new Error("Corrige los errores de validación antes de guardar el mundo.");
  const created = await request("/worlds", { method: "POST", body: JSON.stringify({ scenario: currentScenario }) });
  invalidateCache();
  clearHistoricalResults();
  await refreshWorlds(false, {
    apiBase: currentApiBase,
    worldId: created.world_id,
    branchId: created.root_branch_id,
    year: created.head_year ?? currentScenario.start_year,
  });
  setView("simulation");
  showMessage("Mundo guardado. Puedes iniciar una corrida o consultar su estado inicial.");
}

async function loadWorldScenario() {
  const context = validateContext();
  const cutYear = Number(byId("branch-cut-year").value);
  const query = Number.isSafeInteger(cutYear) ? `?year=${encodeURIComponent(cutYear)}` : "";
  const response = await request(`/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}/scenario${query}`, { noCache: true });
  await setScenario(response.scenario ?? response);
  byId("new-branch-id").value = `${context.branchId}-alternativa`;
  setView("scenario");
  showMessage(`Escenario editable cargado para el corte ${cutYear}. Los eventos aún no ejecutados se muestran como agenda futura.`);
}

async function createBranch() {
  const context = validateContext();
  if (!currentScenario) throw new Error("Abre el escenario del mundo y edítalo antes de crear una rama.");
  const branchId = byId("new-branch-id").value.trim();
  const cutYear = Number(byId("branch-cut-year").value);
  if (!branchId) throw new Error("Escribe un nombre para la rama.");
  if (!Number.isSafeInteger(cutYear)) throw new Error("El corte de la rama debe ser un año entero.");
  const validation = await parseScenario();
  if (!validation.valid) throw new Error("Corrige los errores de validación antes de crear la rama.");
  const created = await request(`/worlds/${encodeURIComponent(context.worldId)}/branches`, {
    method: "POST",
    body: JSON.stringify({ parent_branch_id: context.branchId, branch_id: branchId, cut_year: cutYear, scenario: currentScenario }),
  });
  invalidateCache();
  clearHistoricalResults();
  await refreshBranches(false, { ...context, branchId: created.branch_id ?? branchId, year: cutYear });
  setView("branches");
  showMessage(`Rama ${created.branch_id ?? branchId} creada desde el cierre ${cutYear}.`);
}

function flattenFields(value, prefix = "", depth = 0, rows = []) {
  if (depth > 7) {
    rows.push({ field: prefix || "estructura", value: "Detalle omitido: supera siete niveles." });
    return rows;
  }
  if (rows.length >= 200) {
    if (rows.length === 200) rows.push({ field: "diagnóstico", value: "Detalle adicional omitido después de 200 campos; consulta la ficha visual y el Markdown exportado." });
    return rows;
  }
  if (value === null || value === undefined || typeof value !== "object") {
    rows.push({ field: prefix || "valor", value: value == null ? "Desconocido" : String(value) });
    return rows;
  }
  if (Array.isArray(value)) {
    if (!value.length) rows.push({ field: prefix, value: "Sin datos" });
    value.slice(0, 100).forEach((item, index) => flattenFields(item, `${prefix}[${index + 1}]`, depth + 1, rows));
    if (value.length > 100) rows.push({ field: prefix, value: `Se muestran 100 de ${value.length} elementos en diagnóstico.` });
    return rows;
  }
  const entries = Object.entries(value);
  if (!entries.length) rows.push({ field: prefix, value: "Sin datos" });
  for (const [key, item] of entries) flattenFields(item, prefix ? `${prefix}.${key}` : key, depth + 1, rows);
  return rows;
}

function statusLabel(status, aliveFallback = null) {
  const value = typeof status === "string" ? status : status?.status;
  if (value === "alive" || aliveFallback === true) return "Viva al cierre";
  if (value === "deceased" || aliveFallback === false) {
    const year = status?.death_year;
    return year == null ? "Fallecida al cierre" : `Fallecida desde ${year}`;
  }
  if (value === "not_known_at_year") return "No constaba todavía en este cierre";
  return "Estado desconocido en este cierre";
}

function householdKindLabel(kind) {
  const value = typeof kind === "string" ? kind : kind && typeof kind === "object" ? Object.keys(kind)[0] : "";
  const labels = { family: "Familiar", collective: "Colectivo", single_person: "Una persona", institution: "Institucional", other: "Otro" };
  if (value === "other" && typeof kind.other === "string") return `Otro: ${kind.other}`;
  return labels[value] ?? String(value || "Desconocido").replaceAll("_", " ");
}

function familyLinkLabel(kind) {
  return ({ biological_parent: "Vínculo biológico", adoptive_parent: "Vínculo adoptivo", caregiver: "Vínculo de cuidado" })[kind]
    ?? String(kind ?? "Relación no especificada").replaceAll("_", " ");
}

function renderDiagnosticDetails(host, dossier) {
  const details = document.createElement("details");
  details.className = "diagnostic-details";
  details.append(makeText("summary", "", "Datos de diagnóstico (campos técnicos)"));
  details.append(makeText("p", "muted", "Datos originales del servicio; sirven para inspección técnica y conservan los nombres de campo del modelo."));
  const pre = makeText("pre", "diagnostic-json", JSON.stringify(dossier, null, 2));
  pre.tabIndex = 0;
  pre.setAttribute("aria-label", "Datos técnicos de la ficha en formato JSON de solo lectura");
  details.append(pre);
  host.append(details);
}

function personRosterRow(row) {
  return {
    id: row.person_id ?? row.person?.id,
    nombre: row.presentation?.name ?? row.person?.name ?? `Persona ${row.person_id ?? row.person?.id ?? "—"}`,
    estado: statusLabel(row.status, row.alive_at_close),
    edad: row.age_at_close ?? "—",
    "edad al fallecer": row.age_at_death ?? "—",
    asentamiento: settlementNameMap.get(String(row.settlement_id ?? row.person?.settlement_id)) ?? `Asentamiento ${row.settlement_id ?? row.person?.settlement_id ?? "—"}`,
    hogar: row.household_id ?? row.person?.household_id ?? "—",
  };
}

function renderRoster(host, title, rows, page, nextPage, label) {
  const columns = [
    { label: "Persona", value: (row) => row.nombre }, { label: "ID", value: (row) => row.id },
    { label: "Estado al cierre", value: (row) => row.estado }, { label: "Edad al cierre", value: (row) => row.edad },
    { label: "Edad al fallecer", value: (row) => row["edad al fallecer"] },
    { label: "Asentamiento", value: (row) => row.asentamiento }, { label: "Hogar", value: (row) => row.hogar },
  ];
  appendTable(host, title, columns, rows.map(personRosterRow), {
    total: page.total, offset: page.offset ?? 0, limit: page.limit ?? pageLimit, progressCount: rows.length,
  }, async (offset) => {
    const more = await nextPage(offset);
    const moreRows = more.people ?? more.members ?? [];
    return { ...more, progressCount: moreRows.length, rows: moreRows.map(personRosterRow) };
  }, label);
}

function renderUnionParticipants(union, loadMembers) {
  const participants = [...(union.participants ?? [])];
  const total = union.participants_total ?? participants.length;
  const details = document.createElement("details");
  details.className = "union-participants";
  details.append(makeText("summary", "", `Participantes · ${participants.length}${union.participants_truncated ? ` de ${total}` : ""}`));
  const list = document.createElement("ul");
  const status = makeText("p", "muted", participants.length >= total ? `${participants.length} participantes.` : `Se muestran ${participants.length} de ${total}.`);
  const addParticipants = (items) => {
    for (const participant of items) {
      const item = document.createElement("li");
      const id = participant.person_id ?? participant.person?.id ?? participant.id;
      const name = participant.presentation?.name ?? `Persona ${id ?? "—"}`;
      const button = document.createElement("button");
      button.type = "button"; button.className = "text-button"; button.textContent = name;
      button.setAttribute("aria-label", `Abrir ficha de ${name}`);
      button.disabled = id == null;
      button.addEventListener("click", () => openPersonById(id, name));
      item.append(button); list.append(item);
    }
  };
  addParticipants(participants);
  details.append(list, status);
  if (union.participants_truncated && typeof loadMembers === "function" && union.union_id != null) {
    let offset = participants.length;
    const button = document.createElement("button"); button.type = "button"; button.className = "secondary small-button load-more";
    button.textContent = `Cargar participantes · ${offset} de ${total}`;
    button.addEventListener("click", async () => {
      button.disabled = true; button.textContent = "Cargando…";
      try {
        const page = await loadMembers(union.union_id, offset);
        const more = page.participants ?? [];
        addParticipants(more);
        offset = Number(page.offset ?? offset) + more.length;
        const pageTotal = Number(page.total ?? total);
        status.textContent = `Se muestran ${Math.min(offset, pageTotal)} de ${pageTotal} participantes.`;
        if (more.length === 0 || offset >= pageTotal) button.remove();
        else { button.disabled = false; button.textContent = `Cargar siguientes participantes · ${offset} de ${pageTotal}`; }
      } catch (error) {
        button.disabled = false; button.textContent = "Reintentar carga de participantes";
        if (error.name !== "AbortError") showMessage(describeError(error), true);
      }
    });
    details.append(button);
  }
  return details;
}

function renderPersonDossier(host, title, dossier, unionsNext = null, unionMembersNext = null) {
  host.replaceChildren(makeText("h3", "", title));
  const record = dossier.record;
  const person = record?.person;
  const known = Boolean(record);
  const name = dossier.presentation?.name ?? `Persona ${dossier.person_id}`;
  const state = statusLabel(dossier.status, record?.alive_at_close);
  const currentProfession = dossier.professions?.find((row) => row.profession_id === dossier.profession && row.active_at_close)
    ?? dossier.professions?.find((row) => row.profession_id === dossier.profession);
  if (!known && dossier.status?.status === "not_known_at_year") {
    host.append(makeText("p", "notice", `${name} no constaba en el cierre de ${dossier.year}. Esta vista no revela si aparece en cierres posteriores.`));
  }
  const settlementId = dossier.settlement_id ?? person?.settlement_id;
  const householdId = dossier.household_id ?? person?.household_id;
  const facts = [
    { label: "Nombre", value: name }, { label: "Estado al cierre", value: state },
    { label: "Nacimiento", value: dossier.birth_year ?? person?.birth_year ?? "No conocido a este cierre" },
    { label: "Edad al cierre", value: dossier.age_at_close ?? "No aplicable" },
    { label: "Fallecimiento", value: dossier.death_year ?? person?.death_year ?? "No consta antes de este cierre" },
    { label: "Edad al fallecer", value: dossier.age_at_death ?? "No aplicable" },
    { label: "Residencia", value: dossier.settlement_name ?? (settlementId == null ? "No consta" : settlementNameMap.get(String(settlementId)) ?? `Asentamiento ${settlementId}`) },
    { label: "Hogar", value: householdId ?? "No consta" },
    { label: "Profesión al cierre", value: dossier.profession_name ?? currentProfession?.profession_name ?? dossier.profession ?? person?.profession ?? "No consta" },
  ];
  appendTable(host, "Datos conocidos al cierre", [
    { label: "Dato", value: (row) => row.label }, { label: "Valor", value: (row) => row.value },
  ], facts);
  const traits = dossier.traits ?? [];
  appendTable(host, "Rasgos registrados", [
    { label: "Rasgo", value: (row) => row.key }, { label: "Valor", value: (row) => row.value },
    { label: "Descripción del catálogo", value: (row) => row.description ?? "Sin descripción configurada" },
  ], traits);
  const descriptors = dossier.presentation?.descriptive_traits ?? [];
  host.append(makeText("h4", "", "Descriptores de presentación"));
  host.append(makeText("p", "muted", descriptors.length
    ? `${descriptors.join(" · ")} · descripción ornamental derivada del perfil, no un hecho histórico adicional.`
    : "No hay descriptores ornamentales para esta persona."));
  const residenceRows = dossier.residences ?? [];
  const professionRows = dossier.professions ?? [];
  appendTable(host, "Residencias y trayectoria profesional", [
    { label: "Tipo", value: (row) => row.type }, { label: "Lugar o profesión", value: (row) => row.name },
    { label: "Desde", value: (row) => row.start }, { label: "Hasta (exclusivo)", value: (row) => row.end ?? "Vigente en este cierre" },
  ], [
    ...residenceRows.map((row) => ({ type: "Residencia", name: row.settlement_name ?? settlementNameMap.get(String(row.settlement_id)) ?? `Asentamiento ${row.settlement_id}`, start: row.validity?.start, end: row.validity?.end_exclusive })),
    ...professionRows.map((row) => ({ type: "Profesión", name: row.profession_name ?? row.profession_id, start: row.validity?.start, end: row.validity?.end_exclusive })),
  ]);
  const affiliationRows = [
    ...(dossier.institution_affiliations ?? dossier.institutions ?? []).map((row) => ({
      type: "Institución", name: row.institution_name ?? row.name ?? row.institution_id,
      place: row.settlement_name ?? settlementNameMap.get(String(row.settlement_id)) ?? `Asentamiento ${row.settlement_id}`,
      start: row.validity?.start, end: row.validity?.end_exclusive,
      state: row.active_at_close,
    })),
    ...(dossier.organization_affiliations ?? dossier.organizations ?? []).map((row) => ({
      type: row.founder ? "Organización · fundación" : "Organización", name: row.organization_name ?? row.name ?? row.organization_id,
      place: row.settlement_name ?? settlementNameMap.get(String(row.settlement_id)) ?? `Asentamiento ${row.settlement_id}`,
      start: row.validity?.start, end: row.validity?.end_exclusive,
      state: row.active_at_close,
    })),
  ];
  appendTable(host, "Afiliaciones institucionales y organizaciones", [
    { label: "Tipo", value: (row) => row.type }, { label: "Nombre", value: (row) => row.name },
    { label: "Lugar", value: (row) => row.place }, { label: "Desde", value: (row) => row.start },
    { label: "Hasta (exclusivo)", value: (row) => row.end ?? "Vigente en este cierre" },
    { label: "Estado", value: (row) => row.state == null ? "No especificado" : row.state ? "Activa al cierre" : "Terminada antes de este cierre" },
  ], affiliationRows);
  const unionRows = dossier.unions ?? [];
  const unionPage = dossier.unions ? {
    total: dossier.unions_total ?? unionRows.length, offset: 0, limit: 100, truncated: dossier.unions_truncated, unions: unionRows,
  } : null;
  const unionKindLabel = (kind) => {
    if (typeof kind === "string") return enumLabelFor(kind);
    if (kind && typeof kind === "object" && typeof kind.other === "string") return `Otro tipo de unión: ${kind.other}`;
    if (kind && typeof kind === "object") return enumLabelFor(String(Object.keys(kind)[0] ?? "union"));
    return "Unión";
  };
  const toUnionRow = (row) => ({
    kind: unionKindLabel(row.kind),
    participants: renderUnionParticipants(row, unionMembersNext),
    start: row.validity?.start ?? row.start_year ?? row.start,
    end: row.validity?.end_exclusive ?? row.end_exclusive ?? row.end_year ?? "Vigente en este cierre",
    state: row.active_at_close === true ? "Activa al cierre" : row.active_at_close === false ? "Terminada antes de este cierre" : "Estado no especificado",
  });
  appendTable(host, "Uniones y participantes conocidos", [
    { label: "Tipo", value: (row) => row.kind }, { label: "Participantes", value: (row) => row.participants },
    { label: "Desde", value: (row) => row.start }, { label: "Hasta (exclusivo)", value: (row) => row.end },
    { label: "Estado", value: (row) => row.state },
  ], unionRows.map(toUnionRow), unionPage && typeof unionsNext === "function" ? { ...unionPage, visibleCount: unionRows.length } : null,
  unionsNext ? async (offset) => { const more = await unionsNext(offset); return { ...more, rows: (more.unions ?? []).map(toUnionRow) }; } : null, "uniones");
  if (dossier.unions_truncated && !unionsNext) host.append(makeText("p", "muted", `Se muestran ${unionRows.length} de ${dossier.unions_total ?? "más"} uniones. Vuelve a cargar si el servicio dispone de continuación.`));
  const family = dossier.family_links ?? [];
  appendTable(host, "Vínculos familiares directos", [
    { label: "Papel", value: (row) => row.direction === "parent" ? "Progenitor o persona cuidadora" : "Hijo/a o persona a cargo" },
    { label: "Persona relacionada", value: (row) => row.relative?.presentation?.name ?? `Persona ${row.relative?.person_id ?? "—"}` },
    { label: "Relación", value: (row) => familyLinkLabel(row.link?.kind) },
    { label: "Desde", value: (row) => row.link?.validity?.start ?? "—" },
    { label: "Hasta", value: (row) => row.link?.validity?.end_exclusive ?? "Vigente al cierre" },
    { label: "Estado al cierre", value: (row) => statusLabel(row.relative?.status) },
    { label: "Ficha", render: (row) => {
      const id = row.relative?.person_id ?? row.relative?.person?.id ?? row.relative?.id;
      const name = row.relative?.presentation?.name ?? `Persona ${id ?? "—"}`;
      const button = document.createElement("button"); button.type = "button"; button.className = "secondary small-button";
      button.textContent = "Abrir"; button.disabled = id == null;
      button.addEventListener("click", () => openPersonById(id, name));
      return button;
    } },
  ], family);
  if (dossier.family_links_truncated) {
    const note = makeText("p", "muted", `Se muestran ${family.length} de ${dossier.family_links_total} vínculos directos. El árbol tabular amplía las relaciones y permite cargarlas por páginas.`);
    const jump = document.createElement("a"); jump.href = "#family-tree"; jump.textContent = "Ir al árbol tabular";
    note.append(" ", jump); host.append(note);
  }
  const genomeRows = Array.isArray(dossier.genome)
    ? dossier.genome.map((locus) => ({ locus: locus.locus_id, first: locus.alleles?.[0], second: locus.alleles?.[1], expression: locus.expressed_value }))
    : Object.entries(person?.genome?.loci ?? {}).map(([locus, alleles]) => ({ locus, first: alleles?.[0], second: alleles?.[1], expression: null }));
  if (genomeRows.length) {
    appendTable(host, "Variantes configuradas en el genoma", [
      { label: "Locus", value: (row) => row.locus },
      { label: "Alelo 1", value: (row) => row.first?.variant }, { label: "Linaje conocido 1", value: (row) => row.first?.lineage ?? "No especificado" },
      { label: "Alelo 2", value: (row) => row.second?.variant }, { label: "Linaje conocido 2", value: (row) => row.second?.lineage ?? "No especificado" },
      { label: "Valor expresado configurado", value: (row) => row.expression ?? "No configurado" },
    ], genomeRows);
    host.append(makeText("p", "muted", "Estos son valores configurados y heredados en el modelo; la ficha no atribuye diagnósticos ni rasgos no registrados."));
  }
  renderDiagnosticDetails(host, dossier);
}

function renderHouseholdDossier(host, title, dossier, nextPage) {
  host.replaceChildren(makeText("h3", "", title));
  appendTable(host, "Hogar al cierre", [
    { label: "Dato", value: (row) => row.label }, { label: "Valor", value: (row) => row.value },
  ], [
    { label: "Identificador", value: dossier.household_id }, { label: "Tipo", value: householdKindLabel(dossier.kind) },
    { label: "Asentamiento", value: settlementNameMap.get(String(dossier.settlement_id)) ?? `Asentamiento ${dossier.settlement_id}` },
    { label: "Estado", value: dossier.active_at_close ? "Activo al cierre" : "Cerrado" },
    { label: "Año de cierre", value: dossier.closed_year ?? "No consta" },
  ]);
  const members = dossier.members ?? [];
  renderRoster(host, "Integrantes del hogar", members, dossier, nextPage, "integrantes");
  renderDiagnosticDetails(host, dossier);
}

function renderSettlementDossier(host, title, dossier, nextPage) {
  host.replaceChildren(makeText("h3", "", title));
  const settlement = dossier.settlement ?? {};
  const balance = settlement.balance ?? {};
  appendTable(host, "Asentamiento al cierre", [
    { label: "Dato", value: (row) => row.label }, { label: "Valor", value: (row) => row.value },
  ], [
    ["Nombre", settlement.settlement_name], ["Población", settlement.population], ["Capacidad", settlement.capacity ?? "Sin capacidad configurada"],
    ["Oportunidad", settlement.opportunity_score ?? "No calculada"], ["Nacimientos", balance.births], ["Defunciones", balance.deaths],
    ["Llegadas", balance.incoming], ["Salidas", balance.outgoing], ["Población final del balance", balance.after], ["Hogares", dossier.household_count],
  ].map(([label, value]) => ({ label, value: value ?? "No registrado" })));
  const traits = dossier.culture?.traits ?? [];
  appendTable(host, "Cultura agregada", [
    { label: "Dimensión", value: (row) => row.key }, { label: "Valor actual", value: (row) => row.current_value ?? "Sin valor" },
    { label: "Miembros", value: (row) => row.member_count }, { label: "Peso acumulado", value: (row) => row.total_weight },
    { label: "Intervalo observado", value: (row) => `${row.minimum ?? "—"} a ${row.maximum ?? "—"}` },
  ], traits);
  renderRoster(host, "Habitantes vivos al cierre", dossier.people ?? [], dossier, nextPage, "habitantes");
  renderDiagnosticDetails(host, dossier);
}

function historicalEventText(event) {
  if (event.text_es || event.description_es) return event.text_es ?? event.description_es;
  const kind = event.kind?.type ?? "";
  const data = event.kind?.data ?? {};
  const labels = {
    person_born: "Nació.", person_died: "Falleció.", household_changed: "Cambió de hogar.", residents_moved: "Se trasladó de asentamiento.",
    profession_changed: "Cambió de profesión.", family_link_established: "Se estableció un vínculo familiar.", family_link_ended: "Terminó un vínculo familiar.",
    union_started: "Inició una unión.", institution_affiliation_started: "Comenzó una afiliación institucional.", institution_affiliation_ended: "Terminó una afiliación institucional.",
    organization_founded: "Participó en la fundación de una organización.", organization_membership_started: "Se incorporó a una organización.", organization_membership_ended: "Dejó una organización.",
  };
  if (kind === "scheduled_event_occurred") return `Ocurrió el evento programado «${data.name ?? "sin nombre"}».`;
  if (kind === "event_effect_realized") return `Se registraron efectos del evento «${data.event_name ?? "sin nombre"}».`;
  return labels[kind] ?? `Se registró un hecho relacionado (${kind.replaceAll("_", " ") || "sin descripción"}).`;
}

function renderPersonEvents(host, page, nextPage) {
  host.replaceChildren();
  const events = page.events ?? [];
  appendTable(host, "Cronología de hechos registrados", [
    { label: "Año", value: (row) => row.year }, { label: "Hecho", value: (row) => row.description },
    { label: "Orden del año", value: (row) => row.order },
  ], events.map((event) => ({ ...event, description: historicalEventText(event) })), {
    total: page.total, offset: page.offset ?? 0, limit: page.limit ?? pageLimit, progressCount: events.length,
  }, async (offset) => {
    const more = await nextPage(offset);
    const moreEvents = more.events ?? [];
    return { ...more, progressCount: moreEvents.length, rows: moreEvents.map((event) => ({ ...event, description: historicalEventText(event) })) };
  }, "hechos de la cronología");
}

function getPersonRows(page) {
  return page?.people ?? page?.items ?? [];
}

async function loadHouseholds(offset = 0, append = false) {
  const context = validateContext();
  saveContext();
  const view = beginViewRequest("households");
  const path = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}/households?${asOfQuery(context, { offset: String(offset) })}`;
  const page = await request(path, { signal: view.signal });
  if (!view.isCurrent()) return;
  const rows = page.items ?? page.households ?? [];
  const host = byId("household-results");
  if (!append) {
    host.replaceChildren();
    host.append(makeText("p", "muted", `Hogares · rama ${context.branchId} · año ${context.year}.`));
    const idColumn = { label: "Hogar", value: (row) => row.household_id ?? row.id ?? row.household?.id };
    const columns = [
      idColumn, { label: "Tipo", value: (row) => householdKindLabel(row.kind ?? row.household?.kind) },
      { label: "Asentamiento", value: (row) => row.settlement_name ?? row.settlement_id ?? row.household?.settlement_id },
      { label: "Integrantes", value: (row) => row.member_count ?? row.members?.length ?? row.household?.members?.length },
      { label: "Ficha", render: (row) => {
        const button = document.createElement("button"); button.type = "button"; button.className = "secondary small-button"; button.textContent = "Abrir";
        button.addEventListener("click", () => loadHouseholdDossier(row.household_id ?? row.id ?? row.household?.id).catch((error) => showMessage(describeError(error), true)));
        return button;
      } },
    ];
    appendTable(host, "Hogares visibles", columns, rows, { ...page, visibleCount: rows.length }, async (nextOffset) => {
      const more = await request(`/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}/households?${asOfQuery(context, { offset: String(nextOffset) })}`, { signal: view.signal });
      return { ...more, rows: more.items ?? more.households ?? [] };
    }, "hogares");
  } else {
    const table = host.querySelector("tbody");
    for (const row of rows) {
      const tr = table.insertRow();
      [row.household_id ?? row.id ?? row.household?.id, householdKindLabel(row.kind ?? row.household?.kind), row.settlement_name ?? row.settlement_id ?? row.household?.settlement_id, row.member_count ?? row.members?.length ?? row.household?.members?.length].forEach((item) => tr.insertCell().textContent = String(item ?? "—"));
      const button = document.createElement("button"); button.type = "button"; button.className = "secondary small-button"; button.textContent = "Abrir";
      button.addEventListener("click", () => loadHouseholdDossier(row.household_id ?? row.id ?? row.household?.id).catch((error) => showMessage(describeError(error), true)));
      tr.insertCell().append(button);
    }
  }
}

async function loadHouseholdDossier(householdId) {
  const context = validateContext();
  if (!householdId) throw new Error("El hogar no tiene un identificador disponible.");
  const view = beginViewRequest("household-dossier");
  const path = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}/households/${encodeURIComponent(householdId)}`;
  const response = await request(`${path}?${asOfQuery(context, { offset: "0" })}`, { signal: view.signal });
  if (!view.isCurrent()) return;
  const dossier = response.dossier ?? response;
  const host = byId("person-record");
  renderHouseholdDossier(host, `Ficha del hogar ${householdId} · año ${context.year}`, dossier, async (offset) => {
    const more = await request(`${path}?${asOfQuery(context, { offset: String(offset) })}`, { signal: view.signal });
    return more.dossier ?? more;
  });
  byId("person-events").replaceChildren(makeText("p", "muted", "La cronología individual se consulta al abrir una ficha de persona."));
  byId("family-tree").replaceChildren(makeText("p", "muted", "La ficha de hogar muestra sus integrantes y vínculos conocidos."));
  currentPersonDossier = { type: "household", id: householdId, dossier };
  byId("export-person").disabled = false;
  byId("export-person").textContent = "Exportar hogar Markdown";
  byId("export-person").dataset.kind = "household";
  byId("export-person").dataset.id = String(householdId);
}

function treeRows(page) {
  return page?.items ?? page?.nodes ?? page?.relations ?? page?.relationships ?? [];
}

function mapTreeRow(row) {
  const record = row.person ?? row.relative ?? row.related_person ?? row;
  const person = record.person ?? record;
  const relation = row.link?.kind ?? row.relation ?? row.kind ?? row.relationship ?? row.type ?? "Relación";
  return {
    persona: row.presentation?.name ?? person.name ?? person.display_name ?? person.person_id ?? person.id ?? row.person_id ?? row.relative_id,
    relación: familyLinkLabel(relation),
    profundidad: row.distance ?? row.depth ?? row.generation,
    nacimiento: person.birth_year ?? record.birth_year ?? row.birth_year,
    fallecimiento: person.death_year ?? record.death_year ?? row.death_year,
    edad: record.age_at_close ?? row.age_at_close,
    estado: record.alive_at_close === false ? "Fallecida" : record.alive_at_close === true ? "Viva" : null,
  };
}

function renderGenealogyDiagram(host, personId, dossier, ancestors, descendants) {
  const subjectId = ancestors.subject?.person?.id ?? descendants.subject?.person?.id ?? personId;
  const relatives = [
    ...(ancestors.relations ?? []).map((row) => ({ row, direction: "ancestors" })),
    ...(descendants.relations ?? []).map((row) => ({ row, direction: "descendants" })),
  ];
  const nodes = [{
    id: String(subjectId), name: dossier.presentation?.name ?? `Persona ${personId}`, direction: "subject", distance: 0, kind: "subject", source: null,
  }];
  const seen = new Set([String(subjectId)]);
  for (const { row, direction } of relatives) {
    const id = String(row.relative?.person?.id ?? row.relative?.id ?? "");
    if (!id || seen.has(id) || nodes.length >= 40) continue;
    seen.add(id);
    nodes.push({
      id, name: row.presentation?.name ?? `Persona ${id}`, direction, distance: Number(row.distance) || 1,
      kind: row.link?.kind ?? "caregiver", source: row,
    });
  }
  const levels = [...new Set(nodes.slice(1).map((node) => node.distance))].sort((a, b) => a - b);
  const positions = new Map([[String(subjectId), { x: 480, y: 48 }]]);
  let cursorY = 125;
  for (const distance of levels) {
    const group = nodes.slice(1).filter((node) => node.distance === distance);
    const left = group.filter((node) => node.direction === "ancestors");
    const right = group.filter((node) => node.direction === "descendants");
    const rowsAtLevel = Math.max(1, Math.ceil(Math.max(left.length, right.length) / 4));
    for (const [direction, list] of [["ancestors", left], ["descendants", right]]) {
      list.forEach((node, index) => {
        const sideIndex = index % 4;
        const depthOffset = Math.min(3, distance) * 130;
        const baseX = direction === "ancestors" ? 480 - depthOffset : 480 + depthOffset;
        const x = Math.max(72, Math.min(888, baseX + (sideIndex - 1.5) * 47));
        const y = cursorY + Math.floor(index / 4) * 82;
        positions.set(node.id, { x, y });
      });
    }
    cursorY += rowsAtLevel * 82 + 28;
  }
  const height = Math.max(190, cursorY + 22);
  const svgNS = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(svgNS, "svg");
  svg.classList.add("genealogy-svg");
  svg.setAttribute("viewBox", `0 0 960 ${height}`);
  svg.setAttribute("role", "group");
  const titleId = `genealogy-title-${personId}`;
  const descId = `genealogy-desc-${personId}`;
  svg.setAttribute("aria-labelledby", `${titleId} ${descId}`);
  const title = document.createElementNS(svgNS, "title"); title.id = titleId; title.textContent = `Diagrama genealógico de ${dossier.presentation?.name ?? `persona ${personId}`}`;
  const desc = document.createElementNS(svgNS, "desc"); desc.id = descId; desc.textContent = "Vínculos registrados a la persona central, organizados por distancia. Cada enlace corresponde a un FamilyLink devuelto por la consulta.";
  svg.append(title, desc);
  const colorForKind = (kind) => ({ biological_parent: "#3275b8", adoptive_parent: "#7953a7", caregiver: "#b96525" }[kind] ?? "#60716a");
  const nodeIds = new Set(nodes.map((node) => node.id));
  const diagramLinks = relatives.map(({ row }) => row.link).filter((link) => link
    && nodeIds.has(String(link.parent)) && nodeIds.has(String(link.child))).slice(0, 39);
  const drawnLinks = new Set();
  for (const link of diagramLinks) {
    const fromId = String(link.parent);
    const toId = String(link.child);
    const from = positions.get(fromId);
    const to = positions.get(toId);
    if (!from || !to) continue;
    const linkKey = String(link.id ?? `${fromId}-${toId}-${link.kind}`);
    if (drawnLinks.has(linkKey)) continue;
    drawnLinks.add(linkKey);
    const line = document.createElementNS(svgNS, "line");
    line.setAttribute("x1", String(from.x)); line.setAttribute("y1", String(from.y));
    line.setAttribute("x2", String(to.x)); line.setAttribute("y2", String(to.y));
    line.setAttribute("stroke", colorForKind(link.kind)); line.setAttribute("stroke-width", "2.5");
    line.setAttribute("aria-label", familyLinkLabel(link.kind));
    svg.append(line);
  }
  for (const node of nodes) {
    const position = positions.get(node.id);
    if (!position) continue;
    const group = document.createElementNS(svgNS, "g");
    group.classList.add("genealogy-node", node.kind === "subject" ? "subject" : "relative");
    group.setAttribute("transform", `translate(${position.x} ${position.y})`);
    const relationName = node.kind === "subject" ? "Persona seleccionada" : familyLinkLabel(node.kind);
    group.setAttribute("aria-label", `${node.name}, ${relationName}, distancia ${node.distance}. Pulsa Enter para abrir su ficha.`);
    group.setAttribute("role", node.kind === "subject" ? "img" : "button");
    if (node.kind !== "subject") { group.setAttribute("tabindex", "0"); group.style.cursor = "pointer"; }
    const nodeTitle = document.createElementNS(svgNS, "title"); nodeTitle.textContent = `${node.name} · ${relationName} · distancia ${node.distance}`;
    const rect = document.createElementNS(svgNS, "rect");
    rect.setAttribute("x", node.kind === "subject" ? "-80" : "-66"); rect.setAttribute("y", "-25");
    rect.setAttribute("width", node.kind === "subject" ? "160" : "132"); rect.setAttribute("height", "50"); rect.setAttribute("rx", "10");
    rect.setAttribute("stroke", node.kind === "subject" ? "#173a30" : colorForKind(node.kind)); rect.setAttribute("stroke-width", "2.5");
    const name = document.createElementNS(svgNS, "text"); name.setAttribute("y", "-2"); name.classList.add("genealogy-node-name");
    name.textContent = node.name.length > 20 ? `${node.name.slice(0, 19)}…` : node.name;
    const detail = document.createElementNS(svgNS, "text"); detail.setAttribute("y", "15"); detail.classList.add("genealogy-node-detail");
    detail.textContent = node.kind === "subject" ? "persona central" : `distancia ${node.distance}`;
    group.append(nodeTitle, rect, name, detail);
    if (node.kind !== "subject") {
      const open = () => openPersonById(node.id, node.name);
      group.addEventListener("click", open);
      group.addEventListener("keydown", (event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); open(); } });
    }
    svg.append(group);
  }
  const diagram = document.createElement("figure"); diagram.className = "genealogy-diagram";
  const scroll = document.createElement("div"); scroll.className = "genealogy-scroll"; scroll.setAttribute("role", "region"); scroll.setAttribute("aria-label", "Diagrama genealógico; desplázate horizontalmente para ver todos los niveles"); scroll.tabIndex = 0;
  scroll.append(svg);
  diagram.append(scroll, makeText("figcaption", "muted", `Diagrama local de las páginas cargadas ahora · ${nodes.length} nodos y ${drawnLinks.size} vínculos como máximo en el dibujo. La tabla paginada muestra todas las relaciones devueltas y continúa más allá de este dibujo.`));
  const legend = document.createElement("ul"); legend.className = "genealogy-legend"; legend.setAttribute("aria-label", "Leyenda de tipos de vínculo");
  for (const [kind, label] of [["biological_parent", "Biológico"], ["adoptive_parent", "Adoptivo"], ["caregiver", "Cuidado"]]) {
    const item = document.createElement("li");
    const swatch = document.createElement("span"); swatch.className = "legend-swatch"; swatch.style.backgroundColor = colorForKind(kind); swatch.setAttribute("aria-hidden", "true");
    item.append(swatch, document.createTextNode(label)); legend.append(item);
  }
  diagram.append(legend);
  host.append(diagram);
  if (relatives.length > drawnLinks.size) host.append(makeText("p", "muted", `El diagrama no dibuja ${relatives.length - drawnLinks.size} relaciones de las páginas actuales por su límite de 39 vínculos y 40 nodos. La tabla permite consultarlas todas.`));
}

function openPersonById(personId, name = null) {
  if (personId == null) return;
  const id = String(personId);
  const select = byId("record-person-select");
  if (![...select.options].some((option) => option.value === id)) select.add(new Option(name ?? `Persona ${id}`, id));
  select.value = id;
  loadPersonRecord().catch((error) => showMessage(describeError(error), true));
}

function renderTreeDirection(host, title, page, nextPage) {
  const rows = treeRows(page);
  if (page?.truncated || page?.maximum_depth != null) {
    const note = page?.truncated ? `Consulta truncada por el límite del servicio${page.maximum_depth != null ? ` (profundidad máxima ${page.maximum_depth})` : ""}; carga más relaciones o reduce la profundidad.`
      : page.maximum_depth != null ? `Profundidad máxima disponible en este cierre: ${page.maximum_depth}.` : "";
    if (note) host.append(makeText("p", "muted", `${title}: ${note}`));
  }
  if (!rows.length) {
    host.append(makeText("p", "muted", `${title}: sin relaciones conocidas a este cierre.`));
    return;
  }
  const flattened = rows.map((row) => ({ ...mapTreeRow(row), camino: Array.isArray(row.path) ? row.path.join(" → ") : row.path }));
  appendTable(host, title, [
    { label: "Persona", value: (row) => row.persona }, { label: "Relación", value: (row) => row.relación },
    { label: "Profundidad", value: (row) => row.profundidad }, { label: "Nacimiento", value: (row) => row.nacimiento },
    { label: "Fallecimiento", value: (row) => row.fallecimiento ?? "Desconocido" }, { label: "Estado al cierre", value: (row) => row.estado }, { label: "Camino", value: (row) => row.camino },
    { label: "Edad al cierre", value: (row) => row.edad ?? "No aplicable" },
  ], flattened, { ...page, visibleCount: rows.length }, async (offset) => {
    const more = await nextPage(offset);
    return { ...more, rows: treeRows(more).map((row) => ({ ...mapTreeRow(row), camino: Array.isArray(row.path) ? row.path.join(" → ") : row.path })) };
  }, "relaciones genealógicas");
}

async function loadPersonRecord() {
  const context = validateContext();
  const personId = byId("record-person-select").value;
  if (!personId) throw new Error("Consulta la vista histórica para cargar personas y elige una.");
  const depth = Math.min(12, Math.max(1, Number(byId("tree-depth").value) || 5));
  const view = beginViewRequest("person-dossier");
  const base = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}`;
  const dossierQuery = new URLSearchParams({ year: String(context.year) });
  const dossierResponse = await request(`${base}/people/${encodeURIComponent(personId)}/dossier?${dossierQuery}`, { signal: view.signal });
  if (!view.isCurrent()) return;
  const dossier = dossierResponse.dossier ?? dossierResponse;
  currentPersonDossier = { type: "person", id: personId, dossier };
  renderPersonDossier(
    byId("person-record"), `Ficha de persona ${personId} · año ${context.year}`, dossier,
    (offset) => request(`${base}/people/${encodeURIComponent(personId)}/unions?${asOfQuery(context, { offset: String(offset) })}`, { signal: view.signal }),
    (unionId, offset) => request(`${base}/unions/${encodeURIComponent(unionId)}/members?${asOfQuery(context, { offset: String(offset) })}`, { signal: view.signal }),
  );
  const timelinePage = { events: dossier.timeline ?? [], total: dossier.timeline_total ?? dossier.timeline?.length ?? 0, offset: 0, limit: 100 };
  renderPersonEvents(byId("person-events"), timelinePage, (offset) => request(`${base}/people/${encodeURIComponent(personId)}/events?${asOfQuery(context, { offset: String(offset) })}`, { signal: view.signal }));
  const treeHost = byId("family-tree");
  treeHost.replaceChildren();
  treeHost.append(makeText("h3", "", "Árbol genealógico: diagrama y tabla"));
  treeHost.append(makeText("p", "muted", `Persona ${personId} · rama ${context.branchId} · año ${context.year} · profundidad ${depth}. La tabla distingue vínculos y sirve como alternativa accesible al gráfico.`));
  if (!dossier.record) {
    treeHost.append(makeText("p", "muted", "No hay relaciones genealógicas conocidas para esta persona en el cierre seleccionado."));
    byId("export-person").disabled = true;
    byId("export-person").dataset.kind = "";
    byId("export-person").dataset.id = "";
    return;
  }
  treeHost.append(makeText("p", "muted", "Cargando relaciones genealógicas…"));
  try {
    const [ancestors, descendants] = await Promise.all([
      request(`${base}/people/${encodeURIComponent(personId)}/tree?${new URLSearchParams({ year: String(context.year), direction: "ancestors", depth: String(depth), kinds: "biological_parent,adoptive_parent,caregiver", limit: "50", offset: "0" })}`, { signal: view.signal }),
      request(`${base}/people/${encodeURIComponent(personId)}/tree?${new URLSearchParams({ year: String(context.year), direction: "descendants", depth: String(depth), kinds: "biological_parent,adoptive_parent,caregiver", limit: "50", offset: "0" })}`, { signal: view.signal }),
    ]);
    if (!view.isCurrent()) return;
    treeHost.replaceChildren();
    treeHost.append(makeText("h3", "", "Árbol genealógico: diagrama y tabla"));
    treeHost.append(makeText("p", "muted", `Persona ${personId} · rama ${context.branchId} · año ${context.year} · profundidad ${depth}. La tabla distingue vínculos y sirve como alternativa accesible al gráfico.`));
    renderGenealogyDiagram(treeHost, personId, dossier, ancestors, descendants);
    renderTreeDirection(treeHost, "Antepasados y cuidadores", ancestors, async (offset) => request(`${base}/people/${encodeURIComponent(personId)}/tree?${new URLSearchParams({ year: String(context.year), direction: "ancestors", depth: String(depth), kinds: "biological_parent,adoptive_parent,caregiver", limit: "50", offset: String(offset) })}`, { signal: view.signal }));
    renderTreeDirection(treeHost, "Descendientes y personas a cargo", descendants, async (offset) => request(`${base}/people/${encodeURIComponent(personId)}/tree?${new URLSearchParams({ year: String(context.year), direction: "descendants", depth: String(depth), kinds: "biological_parent,adoptive_parent,caregiver", limit: "50", offset: String(offset) })}`, { signal: view.signal }));
  } catch (error) {
    if (error.name === "AbortError" || !view.isCurrent()) return;
    treeHost.replaceChildren(makeText("h3", "", "Árbol genealógico: diagrama y tabla"));
    treeHost.append(makeText("p", "notice error", `No se pudo cargar el árbol para ${context.branchId}, año ${context.year}: ${describeError(error)}`));
  }
  byId("export-person").disabled = false;
  byId("export-person").textContent = "Exportar ficha Markdown";
  byId("export-person").dataset.kind = "person";
  byId("export-person").dataset.id = String(personId);
}

async function loadSettlementDossier(settlementId = byId("record-settlement-select").value) {
  const context = validateContext();
  if (!settlementId) throw new Error("Selecciona un asentamiento.");
  const view = beginViewRequest("settlement-dossier");
  const path = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}/settlements/${encodeURIComponent(settlementId)}/dossier`;
  const response = await request(`${path}?${asOfQuery(context, { offset: "0" })}`, { signal: view.signal });
  if (!view.isCurrent()) return;
  const dossier = response.dossier ?? response;
  renderSettlementDossier(byId("person-record"), `Ficha del asentamiento ${settlementId} · año ${context.year}`, dossier, async (offset) => {
    const more = await request(`${path}?${asOfQuery(context, { offset: String(offset) })}`, { signal: view.signal });
    return more.dossier ?? more;
  });
  byId("person-events").replaceChildren(makeText("p", "muted", "Selecciona una persona para consultar su cronología individual."));
  byId("family-tree").replaceChildren(makeText("p", "muted", "La ficha muestra sus habitantes, economía y relaciones territoriales conocidas."));
  currentPersonDossier = { type: "settlement", id: settlementId, dossier: response.dossier ?? response };
  byId("export-person").disabled = false;
  byId("export-person").textContent = "Exportar asentamiento Markdown";
  byId("export-person").dataset.kind = "settlement";
  byId("export-person").dataset.id = String(settlementId);
}

async function exportRecordMarkdown() {
  const button = byId("export-person");
  const kind = button.dataset.kind ?? currentPersonDossier?.type;
  const id = button.dataset.id ?? currentPersonDossier?.id;
  if (!kind || !id) throw new Error("Abre una ficha primero.");
  const context = validateContext();
  const endpoint = kind === "person" ? "people" : kind === "household" ? "households" : "settlements";
  const response = await request(`/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}/${endpoint}/${encodeURIComponent(id)}/markdown?${asOfQuery(context)}`, { responseType: "text" });
  download(new Blob([response], { type: "text/markdown;charset=utf-8" }), `${kind}-${id}-${context.year}.md`);
}

async function compareBranches() {
  const context = validateContext();
  const leftBranch = byId("compare-left-branch").value || context.branchId;
  const rightBranch = byId("compare-right-branch").value;
  const leftYear = Number(byId("compare-left-year").value);
  const rightYear = Number(byId("compare-right-year").value);
  if (!rightBranch) throw new Error("Selecciona una rama derecha para comparar.");
  if (!Number.isSafeInteger(leftYear) || !Number.isSafeInteger(rightYear)) throw new Error("Los dos años deben ser enteros.");
  const view = beginViewRequest("compare");
  const query = new URLSearchParams({ left_branch_id: leftBranch, left_year: String(leftYear), right_branch_id: rightBranch, right_year: String(rightYear), limit: String(pageLimit), offset: "0" });
  const path = `/worlds/${encodeURIComponent(context.worldId)}/compare?${query}`;
  const comparison = await request(path, { signal: view.signal });
  if (!view.isCurrent()) return;
  const host = byId("comparison-results");
  host.replaceChildren();
  const leftContext = comparison.left ?? { branch_id: leftBranch, year: leftYear };
  const rightContext = comparison.right ?? { branch_id: rightBranch, year: rightYear };
  host.append(makeText("p", "muted", `Comparación · rama ${leftContext.branch_id} / año ${leftContext.year} / revisión ${leftContext.revision_id ?? "—"} frente a rama ${rightContext.branch_id} / año ${rightContext.year} / revisión ${rightContext.revision_id ?? "—"}.`));
  host.append(makeText("p", "muted", `Economía: izquierda ${leftContext.economy_enabled ? "activa" : "desactivada"} · derecha ${rightContext.economy_enabled ? "activa" : "desactivada"}.`));
  const moneyFields = [
    ["opening_group_money", "Dinero inicial de grupos"], ["opening_treasury_money", "Dinero inicial del tesoro"],
    ["closing_group_money", "Dinero final de grupos"], ["closing_treasury_money", "Dinero final del tesoro"],
    ["external_transport_outflow", "Coste externo de transporte"], ["explicit_emission", "Emisión explícita"], ["explicit_retirement", "Retirada explícita"],
  ];
  if (leftContext.money || rightContext.money) appendTable(host, "Dinero agregado", [
    { label: "Concepto", value: (row) => row.label }, { label: "Izquierda", value: (row) => row.left },
    { label: "Derecha", value: (row) => row.right }, { label: "Cambio derecha − izquierda", value: (row) => row.delta },
  ], moneyFields.map(([key, label]) => {
    const left = leftContext.money?.[key] ?? null;
    const right = rightContext.money?.[key] ?? null;
    return { label, left: amountText(left), right: amountText(right), delta: decimalDifference(right, left) ?? "No calculable con precisión" };
  }));
  if (!leftContext.economy_enabled && !rightContext.economy_enabled) host.append(makeText("p", "muted", "La economía está desactivada en ambos cierres; no se han registrado mercados ni balances monetarios."));
  const treasuryById = (value) => value && typeof value === "object" ? value : {};
  const leftTreasury = treasuryById(leftContext.treasury_goods);
  const rightTreasury = treasuryById(rightContext.treasury_goods);
  const treasuryRows = [...new Set([...Object.keys(leftTreasury), ...Object.keys(rightTreasury)])].map((good) => ({ good, left: leftTreasury[good] ?? {}, right: rightTreasury[good] ?? {} }));
  if (treasuryRows.length) appendTable(host, "Existencias del tesoro por bien", [
    { label: "Bien", value: (row) => row.good },
    { label: "Existencia inicial izq. / der.", value: (row) => `${row.left.opening_stock ?? "—"} / ${row.right.opening_stock ?? "—"}` },
    { label: "Producido izq. / der.", value: (row) => `${row.left.produced ?? "—"} / ${row.right.produced ?? "—"}` },
    { label: "Transferencias netas izq. / der.", value: (row) => `${decimalDifference(row.left.transfers_in ?? "0", row.left.transfers_out ?? "0") ?? "—"} / ${decimalDifference(row.right.transfers_in ?? "0", row.right.transfers_out ?? "0") ?? "—"}` },
    { label: "Pérdidas izq. / der.", value: (row) => `${row.left.storage_loss ?? "—"} / ${row.right.storage_loss ?? "—"}` },
    { label: "Existencia final izq. / der.", value: (row) => `${row.left.closing_stock ?? "—"} / ${row.right.closing_stock ?? "—"}` },
  ], treasuryRows);
  const settlementRows = comparison.items ?? comparison.settlements ?? comparison.census?.items ?? comparison.census ?? [];
  const mapPopulation = (rows) => rows.map((item) => {
    const left = item.left?.settlement ?? item.left_census ?? item.left ?? {};
    const right = item.right?.settlement ?? item.right_census ?? item.right ?? {};
    const leftPopulation = left.population ?? left.living_population ?? item.left_population ?? item.left?.balance?.after;
    const rightPopulation = right.population ?? right.living_population ?? item.right_population ?? item.right?.balance?.after;
    const delta = integerDifference(rightPopulation, leftPopulation) ?? (typeof item.population_delta === "string" ? item.population_delta : Number.isSafeInteger(item.population_delta) ? String(item.population_delta) : "—");
    return {
      settlement: left.settlement_name ?? right.settlement_name ?? item.settlement_name ?? item.settlement_id,
      left_population: leftPopulation,
      right_population: rightPopulation,
      population_delta: delta,
      left_births: item.left?.balance?.births ?? item.left_balance?.births,
      right_births: item.right?.balance?.births ?? item.right_balance?.births,
      left_deaths: item.left?.balance?.deaths ?? item.left_balance?.deaths,
      right_deaths: item.right?.balance?.deaths ?? item.right_balance?.deaths,
      left_migration: `${item.left?.balance?.incoming ?? "—"} / ${item.left?.balance?.outgoing ?? "—"}`,
      right_migration: `${item.right?.balance?.incoming ?? "—"} / ${item.right?.balance?.outgoing ?? "—"}`,
      source: item,
    };
  });
  if (Array.isArray(settlementRows)) appendTable(host, "Censo y balance por asentamiento", [
    { label: "Asentamiento", value: (row) => row.settlement }, { label: "Población izquierda", value: (row) => row.left_population },
    { label: "Población derecha", value: (row) => row.right_population }, { label: "Cambio de población", value: (row) => row.population_delta },
    { label: "Nacimientos izq. / der.", value: (row) => `${row.left_births ?? "—"} / ${row.right_births ?? "—"}` },
    { label: "Defunciones izq. / der.", value: (row) => `${row.left_deaths ?? "—"} / ${row.right_deaths ?? "—"}` },
    { label: "Entradas / salidas izq.", value: (row) => row.left_migration },
    { label: "Entradas / salidas der.", value: (row) => row.right_migration },
  ], mapPopulation(settlementRows), { total: comparison.total ?? settlementRows.length, offset: comparison.offset ?? 0, limit: comparison.limit ?? pageLimit, visibleCount: settlementRows.length }, async (offset) => {
    const nextQuery = new URLSearchParams({ left_branch_id: leftBranch, left_year: String(leftYear), right_branch_id: rightBranch, right_year: String(rightYear), limit: String(pageLimit), offset: String(offset) });
    const more = await request(`/worlds/${encodeURIComponent(context.worldId)}/compare?${nextQuery}`, { signal: view.signal });
    return { ...more, rows: mapPopulation(more.items ?? more.settlements ?? more.census?.items ?? []) };
  }, "asentamientos");
  const mapEconomicRows = (items) => items.flatMap((item) => {
    const leftGoods = item.left?.economy?.goods ?? [];
    const rightGoods = item.right?.economy?.goods ?? [];
    const leftById = new Map(leftGoods.map((good) => [String(good.good_id ?? good.name), good]));
    const rightById = new Map(rightGoods.map((good) => [String(good.good_id ?? good.name), good]));
    return [...new Set([...leftById.keys(), ...rightById.keys()])].map((goodId) => {
      const left = leftById.get(goodId) ?? {};
      const right = rightById.get(goodId) ?? {};
      return {
        settlement: item.settlement_id ?? item.left?.settlement?.settlement_name ?? item.right?.settlement?.settlement_name,
        good: goodId,
        left_price: left.opening_price,
        right_price: right.opening_price,
        left_next_price: left.next_price,
        right_next_price: right.next_price,
        left_need: left.physical_need,
        right_need: right.physical_need,
        left_unmet: left.unmet_physical_need,
        right_unmet: right.unmet_physical_need,
        left_stock: left.ending_stock,
        right_stock: right.ending_stock,
        left_traded: `${left.local_traded ?? "—"} (imp. ${left.imports ?? "—"} / exp. ${left.exports ?? "—"})`,
        right_traded: `${right.local_traded ?? "—"} (imp. ${right.imports ?? "—"} / exp. ${right.exports ?? "—"})`,
      };
    });
  });
  const economicRows = mapEconomicRows(settlementRows);
  if (economicRows.length) appendTable(host, "Economía comparada por bien", [
    { label: "Asentamiento", value: (row) => row.settlement }, { label: "Bien", value: (row) => row.good },
    { label: "Precio inicial izq. / der.", value: (row) => `${row.left_price ?? "—"} / ${row.right_price ?? "—"}` },
    { label: "Precio siguiente izq. / der.", value: (row) => `${row.left_next_price ?? "—"} / ${row.right_next_price ?? "—"}` },
    { label: "Necesidad izq. / der.", value: (row) => `${row.left_need ?? "—"} / ${row.right_need ?? "—"}` },
    { label: "Sin cubrir izq. / der.", value: (row) => `${row.left_unmet ?? "—"} / ${row.right_unmet ?? "—"}` },
    { label: "Comercio izq.", value: (row) => row.left_traded }, { label: "Comercio der.", value: (row) => row.right_traded },
    { label: "Stock izq. / der.", value: (row) => `${row.left_stock ?? "—"} / ${row.right_stock ?? "—"}` },
  ], economicRows, { total: comparison.total ?? settlementRows.length, offset: comparison.offset ?? 0, limit: comparison.limit ?? pageLimit, progressCount: settlementRows.length }, async (offset) => {
    const nextQuery = new URLSearchParams({ left_branch_id: leftBranch, left_year: String(leftYear), right_branch_id: rightBranch, right_year: String(rightYear), limit: String(pageLimit), offset: String(offset) });
    const more = await request(`/worlds/${encodeURIComponent(context.worldId)}/compare?${nextQuery}`, { signal: view.signal });
    return { ...more, progressCount: (more.items ?? []).length, rows: mapEconomicRows(more.items ?? []) };
  }, "bienes por asentamiento");
  const mapCultureRows = (items) => items.flatMap((item) => {
    const leftTraits = item.left?.culture?.traits ?? [];
    const rightTraits = item.right?.culture?.traits ?? [];
    const leftByKey = new Map(leftTraits.map((trait) => [String(trait.key), trait]));
    const rightByKey = new Map(rightTraits.map((trait) => [String(trait.key), trait]));
    return [...new Set([...leftByKey.keys(), ...rightByKey.keys()])].map((key) => ({
      settlement: item.settlement_id ?? item.left?.settlement?.settlement_name ?? item.right?.settlement?.settlement_name,
      dimension: key, left_value: leftByKey.get(key)?.current_value, right_value: rightByKey.get(key)?.current_value,
    }));
  });
  const cultureRows = mapCultureRows(settlementRows);
  if (cultureRows.length) appendTable(host, "Cultura comparada", [
    { label: "Asentamiento", value: (row) => row.settlement }, { label: "Dimensión", value: (row) => row.dimension },
    { label: "Valor izquierdo", value: (row) => row.left_value }, { label: "Valor derecho", value: (row) => row.right_value },
  ], cultureRows, { total: comparison.total ?? settlementRows.length, offset: comparison.offset ?? 0, limit: comparison.limit ?? pageLimit, progressCount: settlementRows.length }, async (offset) => {
    const nextQuery = new URLSearchParams({ left_branch_id: leftBranch, left_year: String(leftYear), right_branch_id: rightBranch, right_year: String(rightYear), limit: String(pageLimit), offset: String(offset) });
    const more = await request(`/worlds/${encodeURIComponent(context.worldId)}/compare?${nextQuery}`, { signal: view.signal });
    return { ...more, progressCount: (more.items ?? []).length, rows: mapCultureRows(more.items ?? []) };
  }, "dimensiones culturales");
  const metadata = Object.fromEntries(Object.entries(comparison).filter(([key, value]) => !["settlements", "items", "census", "economy", "markets", "balances", "culture", "left", "right"].includes(key) && (typeof value !== "object" || value === null)));
  if (Object.keys(metadata).length) appendTable(host, "Resumen de comparación", [{ label: "Dato", value: (row) => row.key }, { label: "Valor", value: (row) => row.value }], Object.entries(metadata).map(([key, value]) => ({ key: key.replaceAll("_", " "), value })));
}

function download(blob, filename) {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

async function exportCurrentScenario() {
  if (!currentScenario) throw new Error("Carga o abre un escenario primero.");
  download(new Blob([JSON.stringify(currentScenario, null, 2)], { type: "application/json;charset=utf-8" }), `chargen-escenario-${currentScenario.world_id ?? "nuevo"}.json`);
}

async function downloadWorldScenario(worldId = selectedContext().worldId) {
  if (!worldId) throw new Error("Selecciona un mundo primero.");
  let branchId = selectedContext().worldId === worldId ? selectedContext().branchId : null;
  if (!branchId) {
    const branches = listFrom(await request(`/worlds/${encodeURIComponent(worldId)}/branches`), ["branches", "items"]);
    branchId = branches.find((branch) => (branch.branch_id ?? branch.id) === "main")?.branch_id ?? branches[0]?.branch_id ?? branches[0]?.id ?? "main";
  }
  const response = await request(`/worlds/${encodeURIComponent(worldId)}/branches/${encodeURIComponent(branchId)}/scenario`, { noCache: true });
  const scenario = response.scenario ?? response;
  download(new Blob([JSON.stringify(scenario, null, 2)], { type: "application/json;charset=utf-8" }), `chargen-${worldId}-escenario.json`);
}

async function importScenarioFile(file) {
  if (!file) return;
  const scenario = JSON.parse(await file.text());
  await setScenario(scenario);
  setView("scenario");
  showMessage(`Escenario ${file.name} cargado en el editor. Valídalo antes de guardarlo.`);
}

async function exportWorldArchive() {
  const context = validateContext();
  const exportInfo = await request(`/worlds/${encodeURIComponent(context.worldId)}/export`, { method: "POST", body: JSON.stringify({}) });
  if (!exportInfo.download_url) throw new Error("El servicio no devolvió una dirección de descarga para la copia.");
  const blob = await request(exportInfo.download_url, { responseType: "blob", noCache: true });
  const filename = exportInfo.filename ?? `chargen-${context.worldId}-copia.sqlite`;
  download(blob, filename);
  showMessage("Copia completa descargada. Puedes importarla más tarde desde esta biblioteca.");
}

async function restoreWorldCopy(worldId) {
  if (!worldId) throw new Error("Selecciona un mundo primero.");
  const exportInfo = await request(`/worlds/${encodeURIComponent(worldId)}/export`, { method: "POST", body: JSON.stringify({}) });
  if (!exportInfo.export_id) throw new Error("El servicio no devolvió un identificador de copia.");
  const restored = await request("/restore", { method: "POST", body: JSON.stringify({ export_id: exportInfo.export_id }), noCache: true });
  invalidateCache();
  const id = restored.restored_id ?? restored.restore_id;
  const apiBase = restored.api_base ?? (id ? `${PRIMARY_API}/restores/${encodeURIComponent(id)}` : null);
  if (!apiBase) throw new Error("El servicio no indicó dónde abrir la copia aislada.");
  clearHistoricalResults();
  await activateDataSource(apiBase, id, worldId);
  await probeConnection();
  await refreshWorlds(false);
  setView("library");
  showMessage(`Copia aislada de ${worldId} abierta como ${id ?? "fuente restaurada"}.`);
}

async function importWorldArchive(file) {
  if (!file) return;
  const bytes = await file.arrayBuffer();
  const imported = await request("/import/archive", {
    method: "POST",
    headers: { "content-type": "application/octet-stream", "x-file-name": encodeURIComponent(file.name) },
    body: bytes,
    noCache: true,
  });
  invalidateCache();
  const id = imported.restored_id ?? imported.restore_id ?? imported.id;
  const preferredBase = imported.api_base ?? (id ? `${PRIMARY_API}/restores/${encodeURIComponent(id)}` : PRIMARY_API);
  clearHistoricalResults();
  await activateDataSource(preferredBase, id, file.name);
  await refreshWorlds(false);
  setView("library");
  showMessage(`Copia ${id ?? "restaurada"} importada y disponible como fuente independiente.`);
}

async function probeConnection() {
  try {
    const health = await request("/health", { noCache: true }, currentApiBase);
    const apiVersion = health.api_version;
    if (apiVersion !== 1) {
      const previousCompatible = apiCompatible;
      apiCompatible = false;
      if (previousCompatible !== false) {
        abortViewRequests();
        invalidateCache();
        clearHistoricalResults();
        clearInterval(activePoll); activePoll = null;
        activeEvents?.close(); activeEvents = null;
      }
      const declaredVersion = apiVersion == null ? "no declarada" : String(apiVersion);
      displayConnection(`Servicio incompatible · API ${declaredVersion}; la interfaz requiere API 1`, "incompatible");
      byId("world-library").replaceChildren(makeText("p", "notice error", "Las consultas y operaciones del servicio están bloqueadas por incompatibilidad de versión. Puedes seguir editando el borrador local y exportarlo como JSON."));
      if (previousCompatible !== false) showMessage(`El servicio declara API ${declaredVersion}; esta interfaz requiere API 1. Las operaciones remotas están bloqueadas. El borrador local sigue disponible para editar y exportar.`, true);
      return false;
    }
    apiCompatible = true;
    displayConnection(`Servicio local listo · ${health.version ?? "Chargen"}${currentRestoreId ? " · copia restaurada" : ""}`, "online");
    return true;
  } catch (error) {
    displayConnection("Servicio local no disponible · borrador y contexto conservados", "offline");
    if (apiCompatible !== false) apiCompatible = null;
    return false;
  }
}

function setView(viewName, moveFocus = false) {
  const known = [...document.querySelectorAll("[data-view-panel]")].some((panel) => panel.dataset.viewPanel === viewName);
  if (!known) viewName = "library";
  for (const tab of document.querySelectorAll("[role=tab][data-view]")) {
    const selected = tab.dataset.view === viewName;
    tab.setAttribute("aria-selected", String(selected));
    tab.tabIndex = selected ? 0 : -1;
    if (selected && moveFocus) tab.focus();
  }
  for (const panel of document.querySelectorAll("[data-view-panel]")) panel.hidden = panel.dataset.viewPanel !== viewName;
  try { localStorage.setItem(savedViewKey, viewName); } catch { /* Ignore storage errors. */ }
  updateContextLabels();
}

function registerListeners() {
  for (const tab of document.querySelectorAll("[role=tab][data-view]")) {
    tab.addEventListener("click", () => setView(tab.dataset.view));
    tab.addEventListener("keydown", (event) => {
      const tabs = [...document.querySelectorAll("[role=tab][data-view]")];
      const index = tabs.indexOf(tab);
      let next = null;
      if (event.key === "ArrowRight") next = tabs[(index + 1) % tabs.length];
      if (event.key === "ArrowLeft") next = tabs[(index + tabs.length - 1) % tabs.length];
      if (event.key === "Home") next = tabs[0];
      if (event.key === "End") next = tabs[tabs.length - 1];
      if (next) { event.preventDefault(); setView(next.dataset.view, true); }
    });
  }
  document.querySelectorAll(".navigate-button").forEach((button) => button.addEventListener("click", () => setView(button.dataset.go)));
  const reconnect = async () => {
    await refreshDataSources(currentApiBase);
    const online = await probeConnection();
    if (!online) return;
    try {
      await refreshWorlds(true);
      await restoreActiveJob();
      showMessage("Conexión recuperada; biblioteca y trabajo activo actualizados.");
    } catch (error) { showMessage(describeError(error), true); }
  };
  byId("retry-connection").addEventListener("click", () => reconnect());
  byId("return-primary").addEventListener("click", () => {
    byId("data-source").value = PRIMARY_API;
    byId("data-source").dispatchEvent(new Event("change"));
  });
  byId("refresh-worlds").addEventListener("click", () => refreshWorlds(true).then(() => showMessage("Biblioteca actualizada.")).catch((error) => showMessage(describeError(error), true)));
  byId("data-source").addEventListener("change", async () => {
    saveContext();
    clearHistoricalResults();
    currentApiBase = byId("data-source").value || PRIMARY_API;
    currentRestoreId = byId("data-source").selectedOptions[0]?.dataset.restoreId ?? null;
    const preferredContext = readSavedContext(currentApiBase);
    abortViewRequests();
    invalidateCache();
    await probeConnection();
    await refreshWorlds(false, preferredContext).catch((error) => showMessage(describeError(error), true));
    setView("library");
    showMessage(currentRestoreId ? "Fuente restaurada seleccionada." : "Biblioteca activa seleccionada.");
  });
  byId("world-select").addEventListener("change", async () => {
    abortViewRequests(); invalidateCache(); clearHistoricalResults();
    try { await refreshBranches(false); } catch (error) { showMessage(describeError(error), true); }
  });
  byId("branch-select").addEventListener("change", () => { abortViewRequests(); clearHistoricalResults(); saveContext(); });
  byId("year-input").addEventListener("input", () => { abortViewRequests(); clearHistoricalResults(); saveContext(); });
  byId("load-census").addEventListener("click", () => loadCensus().then(() => showMessage("Censo consultado para el cierre seleccionado.")).catch((error) => showMessage(describeError(error), true)));
  byId("load-e2").addEventListener("click", () => loadE2().then(() => showMessage("Territorio, cultura e instituciones consultados.")).catch((error) => showMessage(describeError(error), true)));
  byId("load-e3").addEventListener("click", () => loadE3().then(() => showMessage("Mercados y sucesos realizados consultados.")).catch((error) => showMessage(describeError(error), true)));
  byId("load-e2-demo").addEventListener("click", () => loadDemo("/demo-scenario-e2").catch((error) => showMessage(describeError(error), true)));
  byId("load-e3-demo").addEventListener("click", () => loadDemo("/demo-scenario-e3").catch((error) => showMessage(describeError(error), true)));
  byId("validate-scenario").addEventListener("click", () => parseScenario().then((result) => showMessage(result.valid ? "Escenario válido." : `Se encontraron ${result.issues?.length ?? 0} problemas de validación.`, !result.valid)).catch((error) => showMessage(describeError(error), true)));
  byId("create-world").addEventListener("click", () => createWorld().catch((error) => showMessage(describeError(error), true)));
  byId("new-scenario").addEventListener("click", () => setView("scenario"));
  byId("apply-json").addEventListener("click", () => {
    try { setScenario(JSON.parse(byId("scenario-json").value)).then(() => showMessage("JSON aplicado al editor estructurado.")); }
    catch (error) { showMessage(`JSON no válido: ${error.message}`, true); }
  });
  byId("scenario-json").addEventListener("input", () => {
    try { localStorage.setItem(`${savedDraftKey}.raw`, byId("scenario-json").value); } catch { /* Ignore storage errors. */ }
  });
  byId("start-run").addEventListener("click", () => startRun().catch((error) => showMessage(describeError(error), true)));
  byId("pause-run").addEventListener("click", async () => {
    try {
      pauseRequested = true;
      byId("pause-run").disabled = true;
      byId("progress-label").textContent = "Pausa solicitada; espera la confirmación del cierre anual actual.";
      await request(`/runs/${encodeURIComponent(activeJobId)}/pause`, { method: "POST" });
      await pollJob();
    } catch (error) { pauseRequested = false; showMessage(describeError(error), true); }
  });
  byId("resume-run").addEventListener("click", () => request(`/runs/${encodeURIComponent(activeJobId)}/resume`, { method: "POST" }).then(pollJob).catch((error) => showMessage(describeError(error), true)));
  byId("cancel-run").addEventListener("click", () => request(`/runs/${encodeURIComponent(activeJobId)}/cancel`, { method: "POST" }).then(pollJob).catch((error) => showMessage(describeError(error), true)));
  byId("load-households").addEventListener("click", () => loadHouseholds().catch((error) => showMessage(describeError(error), true)));
  byId("load-record-people").addEventListener("click", () => loadRecordPeople().catch((error) => showMessage(describeError(error), true)));
  byId("load-more-people").addEventListener("click", () => loadMorePeople().catch((error) => showMessage(describeError(error), true)));
  byId("load-person").addEventListener("click", () => loadPersonRecord().catch((error) => showMessage(describeError(error), true)));
  byId("load-settlement-record").addEventListener("click", () => loadSettlementDossier().catch((error) => showMessage(describeError(error), true)));
  byId("export-person").addEventListener("click", () => exportRecordMarkdown().catch((error) => showMessage(describeError(error), true)));
  byId("compare-branches").addEventListener("click", () => compareBranches().catch((error) => showMessage(describeError(error), true)));
  byId("load-world-scenario").addEventListener("click", () => loadWorldScenario().catch((error) => showMessage(describeError(error), true)));
  byId("create-branch").addEventListener("click", () => createBranch().catch((error) => showMessage(describeError(error), true)));
  byId("export-scenario").addEventListener("click", () => exportCurrentScenario().catch((error) => showMessage(describeError(error), true)));
  byId("import-scenario").addEventListener("change", (event) => importScenarioFile(event.target.files?.[0]).catch((error) => showMessage(describeError(error), true)).finally(() => { event.target.value = ""; }));
  byId("export-backup").addEventListener("click", () => exportWorldArchive().catch((error) => showMessage(describeError(error), true)));
  byId("import-backup").addEventListener("change", (event) => importWorldArchive(event.target.files?.[0]).catch((error) => showMessage(describeError(error), true)).finally(() => { event.target.value = ""; }));
  window.addEventListener("online", () => reconnect());
  window.addEventListener("offline", () => displayConnection("Sin conexión · borrador y contexto conservados", "offline"));
}

async function restoreActiveJob() {
  let saved;
  try { saved = JSON.parse(localStorage.getItem(savedJobKey) || "null"); } catch {
    try { saved = localStorage.getItem(savedJobKey); } catch { return; }
  }
  const savedId = typeof saved === "string" ? saved : saved?.jobId;
  if (!savedId) return;
  if (activeJobId === savedId && activePoll) return;
  try {
    if (saved?.apiBase && saved.apiBase !== currentApiBase) {
      const option = [...byId("data-source").options].find((item) => item.value === saved.apiBase);
      clearHistoricalResults();
      await activateDataSource(saved.apiBase, option?.dataset.restoreId ?? null, "Trabajo activo");
      await refreshWorlds(true, readSavedContext(saved.apiBase));
    }
    activeJobId = savedId;
    const job = await request(`/runs/${encodeURIComponent(savedId)}`, { noCache: true });
    if (!["queued", "running", "paused"].includes(job.status)) {
      localStorage.removeItem(savedJobKey);
      activeJobId = null;
      return;
    }
    setJobControls(job.status);
    listenForRunEvents(savedId);
    activePoll = setInterval(() => pollJob().catch(reportPollError), 1200);
    await pollJob();
    showMessage("Se recuperó el trabajo que estaba activo antes de cerrar el navegador.");
  } catch (error) {
    activeJobId = null;
    if (error?.status === 404 || error?.status === 410) {
      try { localStorage.removeItem(savedJobKey); } catch { /* Ignore storage errors. */ }
      showMessage("El trabajo guardado ya no está disponible en esta fuente. La selección de mundo y rama se conserva.", true);
    } else {
      byId("progress-label").textContent = "No se pudo recuperar el trabajo en este momento. El identificador sigue guardado; pulsa Reconectar para intentarlo de nuevo.";
      showMessage(`No se pudo recuperar el trabajo guardado. ${describeError(error)}`, true);
    }
  }
}

async function initialize() {
  registerListeners();
  let savedView = "library";
  let savedContext = null;
  try { savedView = localStorage.getItem(savedViewKey) || "library"; } catch { /* Ignore storage errors. */ }
  try { savedContext = JSON.parse(localStorage.getItem(savedContextKey) || "null"); } catch { /* Ignore storage errors. */ }
  setView(savedView);
  await refreshDataSources(savedContext?.apiBase ?? PRIMARY_API);
  const online = await probeConnection();
  if (online) {
    try {
      const demo = await request("/demo-scenario", { noCache: true });
      const draft = JSON.parse(localStorage.getItem(savedDraftKey) || "null");
      await setScenario(draft?.scenario ?? demo, false);
      if (draft?.scenario) byId("draft-status").textContent = `Borrador recuperado de ${new Date(draft.saved_at).toLocaleString("es-ES")}.`;
      await refreshWorlds(true, readSavedContext(currentApiBase));
      await restoreActiveJob();
    } catch (error) {
      displayConnection("Servicio local conectado, pero la inicialización requiere reconexión", "error");
      showMessage(describeError(error), true);
    }
  } else {
    try {
      const draft = JSON.parse(localStorage.getItem(savedDraftKey) || "null");
      if (draft?.scenario) await setScenario(draft.scenario, false);
      const rawDraft = localStorage.getItem(`${savedDraftKey}.raw`);
      if (rawDraft && !draft?.scenario) byId("scenario-json").value = rawDraft;
      byId("world-library").replaceChildren(makeText("p", "muted", "El servicio local está desconectado. El borrador y el contexto guardado siguen disponibles en este navegador."));
    } catch { /* The UI remains available even when browser storage is unavailable. */ }
  }
  setInterval(async () => {
    const wasCompatible = apiCompatible;
    const connected = await probeConnection();
    if (connected && wasCompatible !== true) await reconnect();
    else if (connected && activeJobId) await pollJob().catch(() => {});
  }, 15_000);
}

initialize().catch((error) => {
  displayConnection("Servicio local no disponible · borrador conservado", "offline");
  showMessage(describeError(error), true);
});
