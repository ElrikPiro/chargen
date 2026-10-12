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

function setOptions(select, placeholder, rows, getValue, getLabel) {
  const previous = select.value;
  select.replaceChildren(new Option(placeholder, ""));
  for (const row of rows) select.add(new Option(getLabel(row), String(getValue(row))));
  if ([...select.options].some((option) => option.value === previous)) select.value = previous;
}

function appendPageNotice(section, page) {
  for (const item of Array.isArray(page) ? page : [page]) {
    if (!item || item.total <= item.offset + item.visibleCount) continue;
    const shownThrough = item.offset + item.visibleCount;
    const notice = document.createElement("p");
    notice.className = "muted";
    notice.textContent = `Mostrando ${item.offset + 1}–${shownThrough} de ${item.total} ${item.itemLabel} en este cierre.`;
    section.append(notice);
  }
}

function appendTable(host, title, columns, rows, page = null) {
  const section = document.createElement("section");
  section.className = "e2-table-section";
  const heading = document.createElement("h3");
  heading.textContent = title;
  section.append(heading);
  if (!rows.length) {
    const empty = document.createElement("p");
    empty.className = "muted";
    empty.textContent = "Sin datos en este año.";
    section.append(empty);
    appendPageNotice(section, page);
    host.append(section);
    return;
  }
  const table = document.createElement("table");
  const head = document.createElement("thead");
  const headerRow = document.createElement("tr");
  for (const column of columns) {
    const cell = document.createElement("th");
    cell.textContent = column.label;
    headerRow.append(cell);
  }
  head.append(headerRow);
  const body = document.createElement("tbody");
  for (const row of rows) {
    const tr = document.createElement("tr");
    for (const column of columns) {
      const cell = document.createElement("td");
      const value = column.value(row);
      cell.textContent = value == null ? "—" : String(value);
      tr.append(cell);
    }
    body.append(tr);
  }
  table.append(head, body);
  section.append(table);
  appendPageNotice(section, page);
  host.append(section);
}

function asOfQuery(context, extra = {}) {
  return new URLSearchParams({ year: String(context.year), limit: "100", ...extra });
}

async function loadE2() {
  const context = selectedContext();
  if (!context.worldId || !context.branchId || !Number.isInteger(context.year)) {
    throw new Error("Selecciona un mundo, una rama y un año entero.");
  }
  saveContext();
  const base = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}`;
  const settlementId = byId("settlement-select").value;
  const personId = byId("person-select").value;
  const filter = settlementId ? { settlement_id: settlementId } : {};
  const [settlements, routes, migration, culture, institutions, organizations, people, census] = await Promise.all([
    request(`${base}/settlements?${asOfQuery(context)}`),
    request(`${base}/routes?${asOfQuery(context)}`),
    request(`${base}/migration?${asOfQuery(context, filter)}`),
    request(`${base}/culture?${asOfQuery(context, filter)}`),
    request(`${base}/institutions?${asOfQuery(context, filter)}`),
    request(`${base}/organizations?${asOfQuery(context, filter)}`),
    request(`${base}/people?${asOfQuery(context, filter)}`),
    request(`${base}/census?${asOfQuery(context, filter)}`),
  ]);
  const names = new Map((settlements.items ?? []).map((item) => [String(item.settlement_id), item.settlement_name]));
  setOptions(byId("settlement-select"), "Todos los asentamientos", settlements.items ?? [], (item) => item.settlement_id, (item) => item.settlement_name);
  setOptions(byId("person-select"), "Selecciona una persona", people.people ?? [], (item) => item.person.id, (item) => `Persona ${item.person.id}`);
  const host = byId("territory-results");
  host.replaceChildren();
  const contextLabel = document.createElement("p");
  contextLabel.className = "muted";
  const selectedSettlement = settlementId ? names.get(String(settlementId)) ?? settlementId : "todos los asentamientos";
  contextLabel.textContent = `Vista E2 · rama ${context.branchId} · año ${context.year} · ${selectedSettlement}.`;
  host.append(contextLabel);
  renderCensus(census);
  if ((people.people ?? []).length >= 100) {
    const notice = document.createElement("p");
    notice.className = "muted";
    notice.textContent = settlementId
      ? "El selector muestra hasta 100 personas de este asentamiento."
      : "El selector muestra hasta 100 personas; selecciona un asentamiento para acotar la lista.";
    host.append(notice);
  }
  appendTable(host, "Rutas abiertas al cierre", [
    { label: "Origen", value: (row) => names.get(String(row.from)) ?? row.from },
    { label: "Destino", value: (row) => names.get(String(row.to)) ?? row.to },
    { label: "Coste", value: (row) => row.cost },
    { label: "Capacidad anual", value: (row) => row.capacity_per_year ?? "Sin límite" },
  ], routes.items ?? [], { total: routes.total, offset: routes.offset, visibleCount: routes.items.length, itemLabel: "rutas" });
  appendTable(host, "Balance anual por asentamiento", [
    { label: "Asentamiento", value: (row) => names.get(String(row.settlement_id)) ?? row.settlement_id },
    { label: "Antes", value: (row) => row.before },
    { label: "Nacimientos", value: (row) => row.births },
    { label: "Defunciones", value: (row) => row.deaths },
    { label: "Llegadas", value: (row) => row.incoming },
    { label: "Salidas", value: (row) => row.outgoing },
    { label: "Después", value: (row) => row.after },
  ], migration.items ?? [], { total: migration.total, offset: migration.offset, visibleCount: migration.items.length, itemLabel: "asentamientos" });
  const cultureRows = (culture.items ?? []).flatMap((settlement) => settlement.traits.map((trait) => ({
    settlement: settlement.settlement_name,
    trait: String(trait.key),
    value: trait.current_value,
    members: trait.member_count,
    weight: trait.total_weight,
  })));
  appendTable(host, "Valores culturales agregados", [
    { label: "Asentamiento", value: (row) => row.settlement },
    { label: "Dimensión", value: (row) => row.trait },
    { label: "Valor medio", value: (row) => row.value },
    { label: "Miembros", value: (row) => row.members },
    { label: "Peso total", value: (row) => row.weight },
  ], cultureRows, { total: culture.total, offset: culture.offset, visibleCount: culture.items.length, itemLabel: "asentamientos con valores culturales" });
  appendTable(host, "Instituciones y religiones", [
    { label: "Nombre", value: (row) => row.definition.name },
    { label: "Tipo", value: (row) => row.definition.kind === "religion" ? "Religión" : "Institución" },
    { label: "Afiliaciones activas", value: (row) => row.active_affiliation_count },
  ], institutions.items ?? [], { total: institutions.total, offset: institutions.offset, visibleCount: institutions.items.length, itemLabel: "instituciones o religiones" });
  appendTable(host, "Organizaciones", [
    { label: "Nombre", value: (row) => row.organization.name },
    { label: "Asentamiento", value: (row) => names.get(String(row.organization.settlement_id)) ?? row.organization.settlement_id },
    { label: "Fundadores", value: (row) => row.organization.founders.length },
    { label: "Valores culturales", value: (row) => row.organization.culture_values.map((item) => `${String(item.key)}=${item.current_value ?? "—"}`).join(", ") || "—" },
    { label: "Miembros activos", value: (row) => row.active_member_count },
    { label: "Estado", value: (row) => row.active_at_close ? "Activa" : "Disuelta" },
  ], organizations.items ?? [], { total: organizations.total, offset: organizations.offset, visibleCount: organizations.items.length, itemLabel: "organizaciones" });
  if (personId) {
    const [residence, affiliations] = await Promise.all([
      request(`${base}/people/${encodeURIComponent(personId)}/residence?${asOfQuery(context)}`),
      request(`${base}/people/${encodeURIComponent(personId)}/affiliations?${asOfQuery(context)}`),
    ]);
    appendTable(host, `Residencia y profesiones de persona ${personId}`, [
      { label: "Tipo", value: (row) => row.type },
      { label: "Lugar o profesión", value: (row) => row.name },
      { label: "Desde", value: (row) => row.start },
      { label: "Hasta (exclusivo)", value: (row) => row.end ?? "Vigente" },
    ], [
      ...(residence.residences ?? []).map((item) => ({ type: "Residencia", name: names.get(String(item.settlement_id)) ?? item.settlement_id, start: item.validity.start, end: item.validity.end_exclusive })),
      ...(residence.professions ?? []).map((item) => ({ type: "Profesión", name: item.profession_id, start: item.validity.start, end: item.validity.end_exclusive })),
    ], [
      { total: residence.residence_total, offset: residence.offset, visibleCount: residence.residences.length, itemLabel: "residencias" },
      { total: residence.profession_total, offset: residence.offset, visibleCount: residence.professions.length, itemLabel: "profesiones" },
    ]);
    appendTable(host, "Afiliaciones históricas", [
      { label: "Tipo", value: (row) => row.type },
      { label: "Institución u organización", value: (row) => row.name },
      { label: "Lugar de anclaje", value: (row) => names.get(String(row.settlement_id)) ?? row.settlement_id },
      { label: "Desde", value: (row) => row.start },
      { label: "Hasta (exclusivo)", value: (row) => row.end ?? "Vigente" },
    ], [
      ...(affiliations.institutions ?? []).map((item) => ({ type: "Institución", name: item.institution_id, settlement_id: item.settlement_id, start: item.validity.start, end: item.validity.end_exclusive })),
      ...(affiliations.organizations ?? []).map((item) => ({ type: "Organización", name: item.organization_name, settlement_id: item.settlement_id, start: item.validity.start, end: item.validity.end_exclusive })),
    ], [
      { total: affiliations.institution_total, offset: affiliations.offset, visibleCount: affiliations.institutions.length, itemLabel: "afiliaciones institucionales" },
      { total: affiliations.organization_total, offset: affiliations.offset, visibleCount: affiliations.organizations.length, itemLabel: "afiliaciones organizacionales" },
    ]);
  }
}

function describeEvent(event) {
  const kind = event?.kind ?? {};
  const data = kind.data ?? {};
  if (kind.type === "scheduled_event_occurred") return `Inicio: ${data.name ?? "evento"}`;
  if (kind.type === "event_effect_realized") return `${data.event_name ?? "Evento"} · efecto ${Number(data.effect_index ?? 0) + 1} · ${data.effect?.type ?? "resultado"}`;
  if (kind.type === "economic_year_closed") return "Cierre económico";
  return String(kind.type ?? "Suceso").replaceAll("_", " ");
}

async function loadE3() {
  const context = selectedContext();
  if (!context.worldId || !context.branchId || !Number.isInteger(context.year)) {
    throw new Error("Selecciona un mundo, una rama y un año entero.");
  }
  saveContext();
  const base = `/worlds/${encodeURIComponent(context.worldId)}/branches/${encodeURIComponent(context.branchId)}`;
  const settlementId = byId("settlement-select").value;
  const filter = new URLSearchParams({ year: String(context.year), limit: "100" });
  if (settlementId) filter.set("settlement_id", settlementId);
  const [marketPage, settlementMarkets, trades, events, settlements] = await Promise.all([
    request(`${base}/markets?${filter}`),
    request(`${base}/markets/settlements?${filter}`),
    request(`${base}/markets/trades?${filter}`),
    request(`${base}/events?${filter}`),
    request(`${base}/settlements?${asOfQuery(context)}`),
  ]);
  const names = new Map((settlements.items ?? []).map((row) => [String(row.settlement_id), row.settlement_name]));
  const host = byId("economy-results");
  host.replaceChildren();
  const contextLabel = document.createElement("p");
  contextLabel.className = "muted";
  const selectedName = settlementId ? names.get(String(settlementId)) ?? settlementId : "todos los asentamientos";
  contextLabel.textContent = `Vista E3 · rama ${context.branchId} · año ${context.year} · ${selectedName}. Los sucesos son resultados realizados hasta este cierre.`;
  host.append(contextLabel);
  const groupRows = (marketPage.groups ?? []).flatMap((group) => {
    const goods = Object.entries(group.goods ?? {});
    if (!goods.length) return [{ settlement_id: group.group_id.settlement_id, profession: group.group_id.profession_id ?? "Consumidores", members: group.member_count_after, money: group.closing_money, good: "—", stock: "—" }];
    return goods.map(([good, ledger]) => ({
      settlement_id: group.group_id.settlement_id,
      profession: group.group_id.profession_id ?? "Consumidores",
      members: group.member_count_after,
      money: group.closing_money,
      good,
      stock: ledger.closing_stock,
      produced: ledger.produced,
      unmet: ledger.unmet_physical_need,
    }));
  });
  appendTable(host, marketPage.enabled ? "Saldos económicos por grupo" : "Economía", [
    { label: "Asentamiento", value: (row) => names.get(String(row.settlement_id)) ?? row.settlement_id },
    { label: "Profesión", value: (row) => row.profession },
    { label: "Miembros", value: (row) => row.members },
    { label: "Dinero al cierre", value: (row) => row.money },
    { label: "Bien", value: (row) => row.good },
    { label: "Existencias", value: (row) => row.stock },
    { label: "Producción", value: (row) => row.produced },
    { label: "Necesidad sin cubrir", value: (row) => row.unmet },
  ], groupRows, { total: marketPage.total, offset: marketPage.offset, visibleCount: marketPage.groups.length, itemLabel: "grupos económicos" });
  if (marketPage.money) {
    appendTable(host, "Balance monetario mundial", [
      { label: "Concepto", value: (row) => row.name },
      { label: "Importe", value: (row) => row.value },
    ], Object.entries(marketPage.money).map(([name, value]) => ({ name: name.replaceAll("_", " "), value })));
  }
  appendTable(host, "Existencias de tesorería", [
    { label: "Bien", value: (row) => row.good },
    { label: "Apertura", value: (row) => row.opening_stock },
    { label: "Transferencias recibidas", value: (row) => row.transfers_in },
    { label: "Transferencias enviadas", value: (row) => row.transfers_out },
    { label: "Destrucción", value: (row) => row.event_destroyed },
    { label: "Cierre", value: (row) => row.closing_stock },
  ], Object.entries(marketPage.treasury_goods ?? {}).map(([good, ledger]) => ({ good, ...ledger })));
  const settlementGoods = (settlementMarkets.settlements ?? []).flatMap((settlement) =>
    (settlement.goods ?? []).map((good) => ({ settlement_id: settlement.settlement_id, ...good })),
  );
  appendTable(host, "Necesidad, demanda y stock por bien", [
    { label: "Asentamiento", value: (row) => names.get(String(row.settlement_id)) ?? row.settlement_id },
    { label: "Bien", value: (row) => row.good_id },
    { label: "Precio inicial", value: (row) => row.opening_price },
    { label: "Precio siguiente", value: (row) => row.next_price },
    { label: "Necesidad física", value: (row) => row.physical_need },
    { label: "Necesidad sin cubrir", value: (row) => row.unmet_physical_need },
    { label: "Demanda solvente", value: (row) => row.solvent_demand },
    { label: "Insumos pendientes", value: (row) => row.unmet_input_demand },
    { label: "Oferta sin vender", value: (row) => row.unsold_offer },
    { label: "Importaciones", value: (row) => row.imports },
    { label: "Exportaciones", value: (row) => row.exports },
    { label: "Existencias finales", value: (row) => row.ending_stock },
  ], settlementGoods, { total: settlementGoods.length, offset: 0, visibleCount: settlementGoods.length, itemLabel: "bienes" });
  appendTable(host, "Intercambios realizados", [
    { label: "Vendedor", value: (row) => `${names.get(String(row.source_group.settlement_id)) ?? row.source_group.settlement_id} · ${row.source_group.profession_id ?? "Consumidores"}` },
    { label: "Comprador", value: (row) => `${names.get(String(row.destination_group.settlement_id)) ?? row.destination_group.settlement_id} · ${row.destination_group.profession_id ?? "Consumidores"}` },
    { label: "Bien", value: (row) => row.good_id },
    { label: "Cantidad", value: (row) => row.quantity },
    { label: "Pago", value: (row) => row.buyer_payment },
    { label: "Transporte", value: (row) => row.transport_cost },
    { label: "Motivo", value: (row) => row.purpose === "production_input" ? "Insumos" : "Necesidad" },
  ], trades.trades ?? [], { total: trades.total, offset: trades.offset, visibleCount: trades.trades.length, itemLabel: "intercambios" });
  appendTable(host, "Eventos y efectos realizados", [
    { label: "Año", value: (row) => row.year },
    { label: "Orden", value: (row) => row.order },
    { label: "Resultado", value: (row) => describeEvent(row) },
  ], events.events ?? [], { total: events.total, offset: events.offset, visibleCount: events.events.length, itemLabel: "eventos" });
}

function renderCensus(value) {
  const host = byId("results");
  const rows = value?.settlements ?? value?.items ?? [];
  host.replaceChildren();
  const contextLabel = document.createElement("p");
  contextLabel.className = "muted";
  contextLabel.textContent = `Censo · rama ${value?.branch_id ?? "desconocida"} · año ${value?.year ?? "desconocido"}.`;
  host.append(contextLabel);
  if (!Array.isArray(rows) || rows.length === 0) {
    const empty = document.createElement("p");
    empty.className = "muted";
    empty.textContent = "No hay asentamientos en la vista de censo para este año.";
    host.append(empty);
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
  host.append(table);
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
byId("load-e2").addEventListener("click", () => loadE2().then(() => showMessage("Vista territorial, cultural e institucional consultada para el año seleccionado.")).catch((e) => showMessage(e.message, true)));
byId("load-e3").addEventListener("click", () => loadE3().then(() => showMessage("Balances y eventos históricos consultados para el año seleccionado.")).catch((e) => showMessage(e.message, true)));
byId("validate-scenario").addEventListener("click", () => parseScenario().then((result) => {
  if (result.valid) showMessage("Escenario válido.");
  else showMessage(`El escenario tiene ${result.issues?.length ?? 0} errores de validación.`, true);
}).catch((e) => showMessage(e.message, true)));
byId("load-e2-demo").addEventListener("click", async () => {
  try {
    const demo = await request("/demo-scenario-e2");
    currentScenario = demo;
    byId("scenario-json").value = JSON.stringify(demo, null, 2);
    byId("through-year").value = String(demo.start_year + 3);
    showMessage("Demo E2 cargada: incluye ruta temporal, desplazamiento programado, afiliaciones y organizaciones ficticias.");
  } catch (error) { showMessage(error.message, true); }
});
byId("load-e3-demo").addEventListener("click", async () => {
  try {
    const demo = await request("/demo-scenario-e3");
    currentScenario = demo;
    byId("scenario-json").value = JSON.stringify(demo, null, 2);
    byId("through-year").value = String(demo.start_year + 3);
    showMessage("Demo E3 cargada: incluye producción, intercambios locales y un evento explícito ficticio.");
  } catch (error) { showMessage(error.message, true); }
});
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
