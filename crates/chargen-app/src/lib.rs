//! Local browser application and explicit service composition for Chargen.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{rejection::JsonRejection, DefaultBodyLimit, Path as AxumPath, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chargen_domain::{
    PersonId, Scenario, SettlementId, State as WorldState, ValidationIssue, Year,
};
use chargen_engine::{CancellationToken, Engine, YearCommit};
use chargen_query::{Census, Genealogy, HistoryQuery, PersonRecord, QueryError};
use chargen_store::{
    BranchSummary, ExportManifest, HistoryStore, StoreError, WorldSummary, WorldWriter,
};
use serde::{Deserialize, Serialize};

const DEMO_SCENARIO: &str = include_str!("../../../scenarios/demos/b0-family.json");
const INDEX_HTML: &str = include_str!("../../../web/index.html");
const APP_JS: &str = include_str!("../../../web/app.js");
const STYLES_CSS: &str = include_str!("../../../web/styles.css");
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const DEFAULT_MAX_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Service configuration is separate from a simulation scenario.
#[derive(Clone, Debug)]
pub struct ServiceConfig {
    pub bind: SocketAddr,
    pub database_path: PathBuf,
    pub exports_dir: PathBuf,
    pub restores_dir: PathBuf,
    pub max_body_bytes: usize,
    pub max_page_size: usize,
    pub max_offset: usize,
    pub max_genealogy_depth: usize,
    pub open_browser: bool,
}

impl ServiceConfig {
    pub fn new(database_path: impl Into<PathBuf>) -> Self {
        let database_path = database_path.into();
        let data_dir = database_path
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 7878),
            database_path,
            exports_dir: data_dir.join("exports"),
            restores_dir: data_dir.join("restores"),
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            max_page_size: 100,
            max_offset: 10_000,
            max_genealogy_depth: 12,
            open_browser: true,
        }
    }

    pub fn validate(&self) -> Result<(), ServiceFault> {
        if !self.bind.ip().is_loopback() {
            return Err(ServiceFault::Invalid(
                "El servidor solo puede escuchar en una dirección loopback.".into(),
            ));
        }
        if self.max_body_bytes == 0 || self.max_body_bytes > 256 * 1024 * 1024 {
            return Err(ServiceFault::Invalid(
                "El límite de solicitud debe estar entre 1 byte y 256 MiB.".into(),
            ));
        }
        if self.max_page_size == 0 || self.max_page_size > 500 {
            return Err(ServiceFault::Invalid(
                "El tamaño de página debe estar entre 1 y 500 filas.".into(),
            ));
        }
        if self.max_offset > 100_000
            || self.max_genealogy_depth == 0
            || self.max_genealogy_depth > 32
        {
            return Err(ServiceFault::Invalid(
                "Los límites de consulta exceden el máximo permitido.".into(),
            ));
        }
        if self.database_path.as_os_str().is_empty() {
            return Err(ServiceFault::Invalid(
                "La ruta de datos no puede estar vacía.".into(),
            ));
        }
        Ok(())
    }

    pub fn default_database_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        let base = env::var_os("LOCALAPPDATA").map(PathBuf::from);

        #[cfg(not(target_os = "windows"))]
        let base = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));

        base.unwrap_or_else(env::temp_dir)
            .join("chargen")
            .join("worlds.sqlite3")
    }

    /// Parse the small command-line surface without a global parser singleton.
    pub fn from_args(args: &[String]) -> Result<Self, ServiceFault> {
        let mut config = Self::new(Self::default_database_path());
        let mut index = usize::from(args.first().is_some_and(|arg| arg == "serve"));
        while index < args.len() {
            match args[index].as_str() {
                "--database" => {
                    index += 1;
                    let value = args.get(index).ok_or_else(|| {
                        ServiceFault::Invalid("Falta la ruta para --database.".into())
                    })?;
                    config.database_path = PathBuf::from(value);
                    let data_dir = config
                        .database_path
                        .parent()
                        .unwrap_or(Path::new("."))
                        .to_path_buf();
                    config.exports_dir = data_dir.join("exports");
                    config.restores_dir = data_dir.join("restores");
                }
                "--bind" => {
                    index += 1;
                    let value = args.get(index).ok_or_else(|| {
                        ServiceFault::Invalid("Falta la dirección para --bind.".into())
                    })?;
                    config.bind = value.parse().map_err(|_| {
                        ServiceFault::Invalid("La dirección de --bind no es válida.".into())
                    })?;
                }
                "--no-open" => config.open_browser = false,
                "--help" | "-h" => return Err(ServiceFault::Invalid(usage().into())),
                option => {
                    return Err(ServiceFault::Invalid(format!(
                        "Opción desconocida: {option}. {}",
                        usage()
                    )));
                }
            }
            index += 1;
        }
        config.validate()?;
        Ok(config)
    }
}

fn usage() -> &'static str {
    "Uso: chargen-app [serve] [--database RUTA] [--bind 127.0.0.1:7878] [--no-open]"
}

/// Errors crossing app boundaries. Internal details are logged, while HTTP
/// callers receive stable, sanitized messages.
#[derive(Debug)]
pub enum ServiceFault {
    Invalid(String),
    PayloadTooLarge,
    NotFound(String),
    Conflict(String),
    Internal(String),
}

impl ServiceFault {
    fn http(&self) -> (StatusCode, &'static str, String) {
        match self {
            Self::Invalid(message) => (StatusCode::BAD_REQUEST, "invalid_request", message.clone()),
            Self::PayloadTooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                "La solicitud supera el tamaño máximo permitido.".into(),
            ),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, "not_found", message.clone()),
            Self::Conflict(message) => (StatusCode::CONFLICT, "conflict", message.clone()),
            Self::Internal(message) => {
                eprintln!("Chargen internal operation failed: {message}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "La operación falló. El último cierre anual confirmado se conserva.".into(),
                )
            }
        }
    }
}

impl IntoResponse for ServiceFault {
    fn into_response(self) -> Response {
        let (status, code, message) = self.http();
        (
            status,
            Json(ApiError {
                code: code.into(),
                message,
                details: None,
            }),
        )
            .into_response()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidateScenarioResponse {
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateWorldResponse {
    pub world_id: String,
    pub root_branch_id: String,
    pub head_year: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScenarioEnvelope {
    pub scenario: Scenario,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateBranchRequest {
    pub parent_branch_id: String,
    pub branch_id: String,
    pub cut_year: Option<i32>,
    pub scenario: Scenario,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateBranchResponse {
    pub world_id: String,
    pub parent_branch_id: String,
    pub branch_id: String,
    pub cut_year: Option<i32>,
    pub head_year: Option<i32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PageQuery {
    pub year: i32,
    pub settlement_id: Option<u64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct YearQuery {
    pub year: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GenealogyQuery {
    pub year: i32,
    pub depth: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RunRequest {
    pub through_year: i32,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunSnapshot {
    pub job_id: String,
    pub world_id: String,
    pub branch_id: String,
    pub status: RunStatus,
    pub start_year: i32,
    pub through_year: i32,
    pub current_year: Option<i32>,
    pub completed_years: u64,
    pub total_years: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CensusPage {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub total_population: u64,
    pub settlements: Vec<chargen_query::SettlementCensus>,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct PeoplePage {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub limit: usize,
    pub offset: usize,
    pub people: Vec<PersonRecord>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PersonHistoryResponse {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub person: PersonRecord,
    pub events: Vec<chargen_domain::HistoricalEvent>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ExportResponse {
    pub export_id: String,
    pub manifest: ExportManifest,
    pub download_url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RestoreRequest {
    pub export_id: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RestoreResponse {
    pub restored_id: String,
    pub database_file: String,
    pub worlds: Vec<String>,
    pub restart_required: bool,
}

/// Replaceable annual writer interface for service composition and tests.
pub trait CommitWriter: Send {
    fn commit(&mut self, year: &YearCommit) -> Result<(), ServiceFault>;
    fn checkpoint(&mut self) -> Result<(), ServiceFault>;
}

impl CommitWriter for WorldWriter {
    fn commit(&mut self, year: &YearCommit) -> Result<(), ServiceFault> {
        self.commit_year(year).map_err(map_store_error)
    }

    fn checkpoint(&mut self) -> Result<(), ServiceFault> {
        WorldWriter::checkpoint(self).map_err(map_store_error)
    }
}

/// A storage port keeps path ownership, IDs and SQLite details out of HTTP handlers.
pub trait HistoryRepository: Send + Sync {
    fn create_world(
        &self,
        world_id: &str,
        scenario_json: &str,
        state: &WorldState,
    ) -> Result<(), ServiceFault>;
    fn list_worlds(&self) -> Result<Vec<WorldSummary>, ServiceFault>;
    fn list_branches(&self, world_id: &str) -> Result<Vec<BranchSummary>, ServiceFault>;
    fn scenario_json(&self, world_id: &str, branch_id: &str) -> Result<String, ServiceFault>;
    fn head(&self, world_id: &str, branch_id: &str) -> Result<Option<Year>, ServiceFault>;
    fn load_state(&self, world_id: &str, branch_id: &str) -> Result<WorldState, ServiceFault>;
    fn load_state_at(
        &self,
        world_id: &str,
        branch_id: &str,
        year: Year,
    ) -> Result<WorldState, ServiceFault>;
    fn fork_branch(
        &self,
        world_id: &str,
        parent: &str,
        child: &str,
        cut: Year,
        scenario_json: &str,
    ) -> Result<WorldState, ServiceFault>;
    fn fork_initial_branch(
        &self,
        world_id: &str,
        parent: &str,
        child: &str,
        scenario_json: &str,
    ) -> Result<WorldState, ServiceFault>;
    fn acquire_writer(
        &self,
        world_id: &str,
        branch_id: &str,
    ) -> Result<Box<dyn CommitWriter>, ServiceFault>;
    fn checkpoint(&self, world_id: &str, branch_id: &str) -> Result<(), ServiceFault>;
    fn export_world(
        &self,
        world_id: &str,
        destination: &Path,
    ) -> Result<ExportManifest, ServiceFault>;
    fn restore_world(&self, source: &Path, destination: &Path)
        -> Result<Vec<String>, ServiceFault>;
}

impl HistoryRepository for HistoryStore {
    fn create_world(
        &self,
        world_id: &str,
        scenario_json: &str,
        state: &WorldState,
    ) -> Result<(), ServiceFault> {
        HistoryStore::create_world(self, world_id, "main", scenario_json, state)
            .map_err(map_store_error)
    }
    fn list_worlds(&self) -> Result<Vec<WorldSummary>, ServiceFault> {
        HistoryStore::list_worlds(self).map_err(map_store_error)
    }
    fn list_branches(&self, world_id: &str) -> Result<Vec<BranchSummary>, ServiceFault> {
        let known = self
            .list_worlds()
            .map_err(map_store_error)?
            .iter()
            .any(|world| world.world_id == world_id);
        if !known {
            return Err(ServiceFault::NotFound(
                "No se encontró el mundo solicitado.".into(),
            ));
        }
        HistoryStore::list_branches(self, world_id).map_err(map_store_error)
    }
    fn scenario_json(&self, world_id: &str, branch_id: &str) -> Result<String, ServiceFault> {
        HistoryStore::scenario_json(self, world_id, branch_id).map_err(map_store_error)
    }
    fn head(&self, world_id: &str, branch_id: &str) -> Result<Option<Year>, ServiceFault> {
        HistoryStore::head(self, world_id, branch_id).map_err(map_store_error)
    }
    fn load_state(&self, world_id: &str, branch_id: &str) -> Result<WorldState, ServiceFault> {
        HistoryStore::load_state(self, world_id, branch_id).map_err(map_store_error)
    }
    fn load_state_at(
        &self,
        world_id: &str,
        branch_id: &str,
        year: Year,
    ) -> Result<WorldState, ServiceFault> {
        HistoryStore::load_state_at(self, world_id, branch_id, year).map_err(map_store_error)
    }
    fn fork_branch(
        &self,
        world_id: &str,
        parent: &str,
        child: &str,
        cut: Year,
        scenario_json: &str,
    ) -> Result<WorldState, ServiceFault> {
        HistoryStore::fork_branch(self, world_id, parent, child, cut, scenario_json)
            .map_err(map_store_error)
    }
    fn fork_initial_branch(
        &self,
        world_id: &str,
        parent: &str,
        child: &str,
        scenario_json: &str,
    ) -> Result<WorldState, ServiceFault> {
        HistoryStore::fork_initial_branch(self, world_id, parent, child, scenario_json)
            .map_err(map_store_error)
    }
    fn acquire_writer(
        &self,
        world_id: &str,
        branch_id: &str,
    ) -> Result<Box<dyn CommitWriter>, ServiceFault> {
        HistoryStore::acquire_writer(self, world_id, branch_id)
            .map(|writer| Box::new(writer) as Box<dyn CommitWriter>)
            .map_err(map_store_error)
    }
    fn checkpoint(&self, world_id: &str, branch_id: &str) -> Result<(), ServiceFault> {
        HistoryStore::checkpoint(self, world_id, branch_id).map_err(map_store_error)
    }
    fn export_world(
        &self,
        world_id: &str,
        destination: &Path,
    ) -> Result<ExportManifest, ServiceFault> {
        HistoryStore::export_single_world(self, world_id, destination).map_err(map_store_error)
    }
    fn restore_world(
        &self,
        source: &Path,
        destination: &Path,
    ) -> Result<Vec<String>, ServiceFault> {
        let store = HistoryStore::restore_world(source, destination).map_err(map_store_error)?;
        let ids = store
            .list_worlds()
            .map_err(map_store_error)?
            .into_iter()
            .map(|world| world.world_id)
            .collect();
        Ok(ids)
    }
}

/// Historical projection port; each instance is pinned to a single cutoff.
pub trait HistoricalView: Send + Sync {
    fn census(&self, settlement: Option<SettlementId>) -> Census;
    fn people(
        &self,
        settlement: Option<SettlementId>,
        living_only: bool,
        offset: usize,
        limit: usize,
    ) -> Vec<PersonRecord>;
    fn person(&self, person: PersonId) -> Option<PersonRecord>;
    fn genealogy(&self, person: PersonId, depth: usize) -> Result<Genealogy, ServiceFault>;
    fn events_for_person(
        &self,
        person: PersonId,
    ) -> Result<Vec<chargen_domain::HistoricalEvent>, ServiceFault>;
}

impl HistoricalView for HistoryQuery {
    fn census(&self, settlement: Option<SettlementId>) -> Census {
        HistoryQuery::census(self, settlement)
    }
    fn people(
        &self,
        settlement: Option<SettlementId>,
        living_only: bool,
        offset: usize,
        limit: usize,
    ) -> Vec<PersonRecord> {
        HistoryQuery::people(self, settlement, living_only, offset, limit)
    }
    fn person(&self, person: PersonId) -> Option<PersonRecord> {
        HistoryQuery::person(self, person)
    }
    fn genealogy(&self, person: PersonId, depth: usize) -> Result<Genealogy, ServiceFault> {
        HistoryQuery::genealogy(self, person, depth).map_err(map_query_error)
    }
    fn events_for_person(
        &self,
        person: PersonId,
    ) -> Result<Vec<chargen_domain::HistoricalEvent>, ServiceFault> {
        HistoryQuery::events_for_person(self, person).map_err(map_query_error)
    }
}

pub trait QueryFactory: Send + Sync {
    fn open(&self, context: &QueryContext) -> Result<Box<dyn HistoricalView>, ServiceFault>;
}

/// Immutable identity for a historical read. A query cannot silently move to
/// another world, branch or year after its factory opens the view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryContext {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
}

impl QueryContext {
    fn new(world_id: String, branch_id: String, year: Year) -> Self {
        Self {
            world_id,
            branch_id,
            year,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SqliteQueryFactory {
    path: PathBuf,
}

impl SqliteQueryFactory {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl QueryFactory for SqliteQueryFactory {
    fn open(&self, context: &QueryContext) -> Result<Box<dyn HistoricalView>, ServiceFault> {
        HistoryQuery::open(
            &self.path,
            &context.world_id,
            &context.branch_id,
            context.year,
        )
        .map(|view| Box::new(view) as Box<dyn HistoricalView>)
        .map_err(map_query_error)
    }
}

/// Per-world/branch services hold an explicit scope while preserving the
/// shared, injected history backend.
#[derive(Clone)]
pub struct WorldServices {
    pub world_id: String,
    pub branch_id: String,
    repository: Arc<dyn HistoryRepository>,
}

impl WorldServices {
    fn scenario_json(&self) -> Result<String, ServiceFault> {
        self.repository
            .scenario_json(&self.world_id, &self.branch_id)
    }

    fn load_state(&self) -> Result<WorldState, ServiceFault> {
        self.repository.load_state(&self.world_id, &self.branch_id)
    }

    fn acquire_writer(&self) -> Result<Box<dyn CommitWriter>, ServiceFault> {
        self.repository
            .acquire_writer(&self.world_id, &self.branch_id)
    }
}

fn map_store_error(error: StoreError) -> ServiceFault {
    match error {
        StoreError::WorldExists(_) => {
            ServiceFault::Conflict("Ya existe un mundo con ese identificador.".into())
        }
        StoreError::WorldNotFound(_) | StoreError::BranchNotFound { .. } => {
            ServiceFault::NotFound("No se encontró el mundo o la rama solicitada.".into())
        }
        StoreError::WriterAlreadyActive(_)
        | StoreError::YearUnavailable { .. }
        | StoreError::Invalid(_) => ServiceFault::Conflict(
            "La operación no coincide con el estado histórico confirmado.".into(),
        ),
        StoreError::ScenarioInvalid(_) => {
            ServiceFault::Invalid("El escenario de la revisión no es válido.".into())
        }
        other => ServiceFault::Internal(other.to_string()),
    }
}

fn map_query_error(error: QueryError) -> ServiceFault {
    match error {
        QueryError::PersonNotFound(_) => {
            ServiceFault::NotFound("La persona no existe en el año consultado.".into())
        }
        QueryError::Store(error) => map_store_error(error),
    }
}

pub trait MeasurementClock: Send + Sync {
    fn now_micros(&self) -> u128;
}

#[derive(Debug)]
pub struct SystemMeasurementClock {
    started: Instant,
}

impl Default for SystemMeasurementClock {
    fn default() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl MeasurementClock for SystemMeasurementClock {
    fn now_micros(&self) -> u128 {
        self.started.elapsed().as_micros()
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct YearMetric {
    pub world_id: String,
    pub branch_id: String,
    pub year: i32,
    pub elapsed_micros: u128,
    pub population: usize,
    pub event_count: usize,
}

pub trait MetricsSink: Send + Sync {
    fn year_committed(&self, metric: YearMetric);
}

#[derive(Default)]
pub struct NullMetricsSink;

impl MetricsSink for NullMetricsSink {
    fn year_committed(&self, _metric: YearMetric) {}
}

pub struct ContainerBuilder {
    config: ServiceConfig,
    repository: Option<Arc<dyn HistoryRepository>>,
    query_factory: Option<Arc<dyn QueryFactory>>,
    clock: Arc<dyn MeasurementClock>,
    metrics: Arc<dyn MetricsSink>,
}

impl ContainerBuilder {
    pub fn new(config: ServiceConfig) -> Self {
        Self {
            config,
            repository: None,
            query_factory: None,
            clock: Arc::new(SystemMeasurementClock::default()),
            metrics: Arc::new(NullMetricsSink),
        }
    }

    pub fn repository(mut self, repository: Arc<dyn HistoryRepository>) -> Self {
        self.repository = Some(repository);
        self
    }

    pub fn query_factory(mut self, queries: Arc<dyn QueryFactory>) -> Self {
        self.query_factory = Some(queries);
        self
    }

    pub fn clock(mut self, clock: Arc<dyn MeasurementClock>) -> Self {
        self.clock = clock;
        self
    }

    pub fn metrics(mut self, metrics: Arc<dyn MetricsSink>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn build(self) -> Result<AppContainer, ServiceFault> {
        self.config.validate()?;
        fs::create_dir_all(&self.config.exports_dir)
            .map_err(|error| ServiceFault::Internal(error.to_string()))?;
        fs::create_dir_all(&self.config.restores_dir)
            .map_err(|error| ServiceFault::Internal(error.to_string()))?;
        let store = if self.repository.is_none() || self.query_factory.is_none() {
            Some(HistoryStore::open(&self.config.database_path).map_err(map_store_error)?)
        } else {
            None
        };
        let repository = self.repository.unwrap_or_else(|| {
            Arc::new(store.as_ref().expect("store opened above").clone())
                as Arc<dyn HistoryRepository>
        });
        let query_factory = self.query_factory.unwrap_or_else(|| {
            Arc::new(SqliteQueryFactory::new(self.config.database_path.clone()))
                as Arc<dyn QueryFactory>
        });
        Ok(AppContainer {
            config: Arc::new(self.config),
            repository,
            query_factory,
            clock: self.clock,
            metrics: self.metrics,
            jobs: Arc::new(RwLock::new(BTreeMap::new())),
            job_counter: Arc::new(AtomicU64::new(0)),
            export_counter: Arc::new(AtomicU64::new(0)),
        })
    }
}

#[derive(Clone)]
pub struct AppContainer {
    config: Arc<ServiceConfig>,
    repository: Arc<dyn HistoryRepository>,
    query_factory: Arc<dyn QueryFactory>,
    clock: Arc<dyn MeasurementClock>,
    metrics: Arc<dyn MetricsSink>,
    jobs: Arc<RwLock<BTreeMap<String, Arc<JobControl>>>>,
    job_counter: Arc<AtomicU64>,
    export_counter: Arc<AtomicU64>,
}

struct JobControl {
    cancellation: CancellationToken,
    gate: Mutex<JobGate>,
    gate_changed: Condvar,
    snapshot: Mutex<RunSnapshot>,
}

#[derive(Default)]
struct JobGate {
    pause_requested: bool,
    cancelled: bool,
}

impl AppContainer {
    pub fn config(&self) -> &ServiceConfig {
        &self.config
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/", get(index))
            .route("/app.js", get(app_js))
            .route("/styles.css", get(styles_css))
            .route("/api/health", get(health))
            .route("/api/demo-scenario", get(demo_scenario))
            .route("/api/scenarios/validate", post(validate_scenario))
            .route("/api/worlds", get(list_worlds).post(create_world))
            .route(
                "/api/worlds/{world_id}/branches",
                get(list_branches).post(create_branch),
            )
            .route(
                "/api/worlds/{world_id}/branches/{branch_id}/census",
                get(get_census),
            )
            .route(
                "/api/worlds/{world_id}/branches/{branch_id}/people",
                get(list_people),
            )
            .route(
                "/api/worlds/{world_id}/branches/{branch_id}/people/{person_id}",
                get(get_person),
            )
            .route(
                "/api/worlds/{world_id}/branches/{branch_id}/people/{person_id}/genealogy",
                get(get_genealogy),
            )
            .route(
                "/api/worlds/{world_id}/branches/{branch_id}/runs",
                post(start_run),
            )
            .route(
                "/api/worlds/{world_id}/branches/{branch_id}/checkpoint",
                post(checkpoint),
            )
            .route("/api/worlds/{world_id}/export", post(export_world))
            .route("/api/runs/{job_id}", get(get_run))
            .route("/api/runs/{job_id}/pause", post(pause_run))
            .route("/api/runs/{job_id}/resume", post(resume_run))
            .route("/api/runs/{job_id}/cancel", post(cancel_run))
            .route("/api/exports/{export_id}", get(download_export))
            .route("/api/restore", post(restore_world))
            .layer(DefaultBodyLimit::max(self.config.max_body_bytes))
            .with_state(self.clone())
    }

    pub async fn serve(self) -> Result<SocketAddr, std::io::Error> {
        let listener = tokio::net::TcpListener::bind(self.config.bind).await?;
        let address = listener.local_addr()?;
        let url = format!("http://{address}/");
        println!("Chargen listening on {url}");
        if self.config.open_browser {
            let browser_url = url.clone();
            let _ = tokio::task::spawn_blocking(move || open_browser(&browser_url)).await;
        }
        axum::serve(listener, self.router())
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        Ok(address)
    }

    fn lookup_job(&self, job_id: &str) -> Result<Arc<JobControl>, ServiceFault> {
        validate_key(job_id, "trabajo")?;
        self.jobs
            .read()
            .map_err(|_| ServiceFault::Internal("job registry lock poisoned".into()))?
            .get(job_id)
            .cloned()
            .ok_or_else(|| ServiceFault::NotFound("No se encontró el trabajo solicitado.".into()))
    }
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = Command::new("xdg-open").arg(url).spawn();
    if let Err(error) = result {
        eprintln!("Could not open browser automatically: {error}");
    }
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn app_js() -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/javascript; charset=utf-8")
        .body(Body::from(APP_JS))
        .expect("static response is valid")
}

async fn styles_css() -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, "text/css; charset=utf-8")
        .body(Body::from(STYLES_CSS))
        .expect("static response is valid")
}

#[derive(Serialize)]
struct HealthResponse {
    version: &'static str,
    offline: bool,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        version: env!("CARGO_PKG_VERSION"),
        offline: true,
    })
}

async fn demo_scenario() -> Response {
    Response::builder()
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .body(Body::from(DEMO_SCENARIO))
        .expect("embedded scenario is valid")
}

async fn validate_scenario(
    body: Result<Json<ScenarioEnvelope>, JsonRejection>,
) -> Result<Json<ValidateScenarioResponse>, ServiceFault> {
    let Json(envelope) = body.map_err(map_json_rejection)?;
    validate_safe_scenario_ids(&envelope.scenario)?;
    match envelope.scenario.validate() {
        Ok(_) => Ok(Json(ValidateScenarioResponse {
            valid: true,
            issues: Vec::new(),
        })),
        Err(issues) => Ok(Json(ValidateScenarioResponse {
            valid: false,
            issues,
        })),
    }
}

async fn create_world(
    State(app): State<AppContainer>,
    body: Result<Json<ScenarioEnvelope>, JsonRejection>,
) -> Result<Json<CreateWorldResponse>, ServiceFault> {
    let Json(envelope) = body.map_err(map_json_rejection)?;
    validate_safe_scenario_ids(&envelope.scenario)?;
    let validated = envelope.scenario.validate().map_err(validation_fault)?;
    let world_id = envelope.scenario.world_id.0.to_string();
    let initial_state = chargen_engine::generate_initial_state(&validated).map_err(|_| {
        ServiceFault::Invalid("No se pudo materializar la población inicial del escenario.".into())
    })?;
    let scenario_json = serde_json::to_string(&envelope.scenario)
        .map_err(|_| ServiceFault::Internal("scenario serialization failed".into()))?;
    let repository = app.repository.clone();
    let world_for_thread = world_id.clone();
    tokio::task::spawn_blocking(move || {
        repository.create_world(&world_for_thread, &scenario_json, &initial_state)
    })
    .await
    .map_err(|error| ServiceFault::Internal(error.to_string()))??;
    Ok(Json(CreateWorldResponse {
        world_id,
        root_branch_id: "main".into(),
        head_year: None,
    }))
}

async fn list_worlds(
    State(app): State<AppContainer>,
) -> Result<Json<Vec<WorldSummary>>, ServiceFault> {
    let repository = app.repository.clone();
    tokio::task::spawn_blocking(move || repository.list_worlds())
        .await
        .map_err(|error| ServiceFault::Internal(error.to_string()))?
        .map(Json)
}

async fn list_branches(
    State(app): State<AppContainer>,
    AxumPath(world_id): AxumPath<String>,
) -> Result<Json<Vec<BranchSummary>>, ServiceFault> {
    validate_key(&world_id, "mundo")?;
    let repository = app.repository.clone();
    tokio::task::spawn_blocking(move || repository.list_branches(&world_id))
        .await
        .map_err(|error| ServiceFault::Internal(error.to_string()))?
        .map(Json)
}

async fn create_branch(
    State(app): State<AppContainer>,
    AxumPath(world_id): AxumPath<String>,
    body: Result<Json<CreateBranchRequest>, JsonRejection>,
) -> Result<Json<CreateBranchResponse>, ServiceFault> {
    validate_key(&world_id, "mundo")?;
    let Json(request) = body.map_err(map_json_rejection)?;
    validate_key(&request.parent_branch_id, "rama madre")?;
    validate_key(&request.branch_id, "rama")?;
    validate_safe_scenario_ids(&request.scenario)?;
    if request.scenario.world_id.0.to_string() != world_id {
        return Err(ServiceFault::Invalid(
            "El identificador del escenario no coincide con el mundo de la ruta.".into(),
        ));
    }
    request.scenario.validate().map_err(validation_fault)?;
    let scenario_json = serde_json::to_string(&request.scenario)
        .map_err(|_| ServiceFault::Internal("scenario serialization failed".into()))?;
    let repository = app.repository.clone();
    let world_for_thread = world_id.clone();
    let parent = request.parent_branch_id.clone();
    let child = request.branch_id.clone();
    let cut = request.cut_year.map(Year);
    tokio::task::spawn_blocking(move || match cut {
        Some(cut) => repository
            .fork_branch(&world_for_thread, &parent, &child, cut, &scenario_json)
            .map(|_| ()),
        None => repository
            .fork_initial_branch(&world_for_thread, &parent, &child, &scenario_json)
            .map(|_| ()),
    })
    .await
    .map_err(|error| ServiceFault::Internal(error.to_string()))??;
    Ok(Json(CreateBranchResponse {
        world_id,
        parent_branch_id: request.parent_branch_id,
        branch_id: request.branch_id,
        cut_year: request.cut_year,
        head_year: request.cut_year,
    }))
}

async fn get_census(
    State(app): State<AppContainer>,
    AxumPath((world_id, branch_id)): AxumPath<(String, String)>,
    Query(query): Query<PageQuery>,
) -> Result<Json<CensusPage>, ServiceFault> {
    validate_context(&world_id, &branch_id)?;
    let (limit, offset) = checked_page(&app.config, query.limit, query.offset)?;
    if let Some(id) = query.settlement_id {
        validate_numeric_id(id, "asentamiento")?;
    }
    let factory = app.query_factory.clone();
    let year = Year(query.year);
    let context = QueryContext::new(world_id.clone(), branch_id.clone(), year);
    let settlement = query.settlement_id.map(SettlementId);
    let view = tokio::task::spawn_blocking(move || factory.open(&context))
        .await
        .map_err(|error| ServiceFault::Internal(error.to_string()))??;
    let Census {
        world_id,
        branch_id,
        year,
        total_population,
        settlements,
    } = view.census(settlement);
    let settlements = settlements.into_iter().skip(offset).take(limit).collect();
    Ok(Json(CensusPage {
        world_id,
        branch_id,
        year,
        total_population,
        settlements,
        limit,
        offset,
    }))
}

async fn list_people(
    State(app): State<AppContainer>,
    AxumPath((world_id, branch_id)): AxumPath<(String, String)>,
    Query(query): Query<PageQuery>,
) -> Result<Json<PeoplePage>, ServiceFault> {
    validate_context(&world_id, &branch_id)?;
    let (limit, offset) = checked_page(&app.config, query.limit, query.offset)?;
    if let Some(id) = query.settlement_id {
        validate_numeric_id(id, "asentamiento")?;
    }
    let factory = app.query_factory.clone();
    let year = Year(query.year);
    let context = QueryContext::new(world_id.clone(), branch_id.clone(), year);
    let settlement = query.settlement_id.map(SettlementId);
    let view = tokio::task::spawn_blocking(move || factory.open(&context))
        .await
        .map_err(|error| ServiceFault::Internal(error.to_string()))??;
    let people = view
        .people(settlement, false, offset, limit)
        .into_iter()
        .map(|record| sanitize_person_record(record, year))
        .collect();
    Ok(Json(PeoplePage {
        world_id,
        branch_id,
        year,
        limit,
        offset,
        people,
    }))
}

async fn get_person(
    State(app): State<AppContainer>,
    AxumPath((world_id, branch_id, person_id)): AxumPath<(String, String, u64)>,
    Query(query): Query<YearQuery>,
) -> Result<Json<PersonHistoryResponse>, ServiceFault> {
    validate_context(&world_id, &branch_id)?;
    validate_numeric_id(person_id, "persona")?;
    let factory = app.query_factory.clone();
    let year = Year(query.year);
    let context = QueryContext::new(world_id.clone(), branch_id.clone(), year);
    let view = tokio::task::spawn_blocking(move || factory.open(&context))
        .await
        .map_err(|error| ServiceFault::Internal(error.to_string()))??;
    let person = view.person(PersonId(person_id)).ok_or_else(|| {
        ServiceFault::NotFound("La persona no existe en el año consultado.".into())
    })?;
    let person = sanitize_person_record(person, year);
    let events = view.events_for_person(PersonId(person_id))?;
    Ok(Json(PersonHistoryResponse {
        world_id,
        branch_id,
        year,
        person,
        events,
    }))
}

async fn get_genealogy(
    State(app): State<AppContainer>,
    AxumPath((world_id, branch_id, person_id)): AxumPath<(String, String, u64)>,
    Query(query): Query<GenealogyQuery>,
) -> Result<Json<Genealogy>, ServiceFault> {
    validate_context(&world_id, &branch_id)?;
    validate_numeric_id(person_id, "persona")?;
    let depth = query.depth.unwrap_or(4);
    if depth == 0 || depth > app.config.max_genealogy_depth {
        return Err(ServiceFault::Invalid(format!(
            "La profundidad debe estar entre 1 y {}.",
            app.config.max_genealogy_depth
        )));
    }
    let factory = app.query_factory.clone();
    let year = Year(query.year);
    let context = QueryContext::new(world_id.clone(), branch_id.clone(), year);
    let view = tokio::task::spawn_blocking(move || factory.open(&context))
        .await
        .map_err(|error| ServiceFault::Internal(error.to_string()))??;
    let mut genealogy = view.genealogy(PersonId(person_id), depth)?;
    genealogy.subject = sanitize_person_record(genealogy.subject, year);
    for relation in genealogy
        .ancestors
        .iter_mut()
        .chain(genealogy.descendants.iter_mut())
    {
        relation.relative = sanitize_person_record(relation.relative.clone(), year);
    }
    Ok(Json(genealogy))
}

async fn checkpoint(
    State(app): State<AppContainer>,
    AxumPath((world_id, branch_id)): AxumPath<(String, String)>,
) -> Result<StatusCode, ServiceFault> {
    validate_context(&world_id, &branch_id)?;
    let repository = app.repository.clone();
    tokio::task::spawn_blocking(move || repository.checkpoint(&world_id, &branch_id))
        .await
        .map_err(|error| ServiceFault::Internal(error.to_string()))??;
    Ok(StatusCode::NO_CONTENT)
}

async fn start_run(
    State(app): State<AppContainer>,
    AxumPath((world_id, branch_id)): AxumPath<(String, String)>,
    body: Result<Json<RunRequest>, JsonRejection>,
) -> Result<Json<RunSnapshot>, ServiceFault> {
    validate_context(&world_id, &branch_id)?;
    let Json(request) = body.map_err(map_json_rejection)?;
    let through_year = request.through_year;
    let world = WorldServices {
        world_id: world_id.clone(),
        branch_id: branch_id.clone(),
        repository: app.repository.clone(),
    };
    let world_for_thread = world.clone();
    let prepared = tokio::task::spawn_blocking(move || prepare_run(world_for_thread, through_year))
        .await
        .map_err(|error| ServiceFault::Internal(error.to_string()))??;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    let serial = app.job_counter.fetch_add(1, Ordering::Relaxed);
    let job_id = format!("job-{timestamp:x}-{serial:x}");
    let snapshot = RunSnapshot {
        job_id: job_id.clone(),
        world_id: world_id.clone(),
        branch_id: branch_id.clone(),
        status: RunStatus::Queued,
        start_year: prepared.start_year,
        through_year,
        current_year: prepared.current_year,
        completed_years: 0,
        total_years: prepared.total_years,
        message: None,
    };
    let job = Arc::new(JobControl {
        cancellation: CancellationToken::default(),
        gate: Mutex::new(JobGate::default()),
        gate_changed: Condvar::new(),
        snapshot: Mutex::new(snapshot.clone()),
    });
    app.jobs
        .write()
        .map_err(|_| ServiceFault::Internal("job registry lock poisoned".into()))?
        .insert(job_id, job.clone());

    let metrics = app.metrics.clone();
    let clock = app.clock.clone();
    let context = SimulationJobContext {
        world,
        job,
        prepared,
        through_year,
    };
    tokio::task::spawn_blocking(move || run_worker(metrics, clock, context));
    Ok(Json(snapshot))
}

struct PreparedRun {
    scenario: chargen_domain::ValidatedScenario,
    state: WorldState,
    writer: Box<dyn CommitWriter>,
    start_year: i32,
    current_year: Option<i32>,
    total_years: u64,
}

/// Owns the immutable world/branch identity, durable store and writer lease for
/// one simulation run. Acquiring the lease before loading the head prevents
/// concurrent start requests from advancing a stale in-memory state.
struct SimulationJobContext {
    world: WorldServices,
    job: Arc<JobControl>,
    prepared: PreparedRun,
    through_year: i32,
}

fn prepare_run(world: WorldServices, through_year: i32) -> Result<PreparedRun, ServiceFault> {
    let writer = world.acquire_writer()?;
    let scenario_json = world.scenario_json()?;
    let scenario: Scenario = serde_json::from_str(&scenario_json)
        .map_err(|_| ServiceFault::Internal("stored scenario could not be decoded".into()))?;
    validate_safe_scenario_ids(&scenario)?;
    let validated = scenario.validate().map_err(validation_fault)?;
    if scenario.world_id.0.to_string() != world.world_id {
        return Err(ServiceFault::Conflict(
            "La revisión de escenario pertenece a otro mundo.".into(),
        ));
    }
    let state = world.load_state()?;
    validated.validate_state(&state).map_err(validation_fault)?;
    if state.next_person_id > MAX_SAFE_INTEGER - 100_000
        || state.next_household_id > MAX_SAFE_INTEGER - 100_000
        || state.next_union_id > MAX_SAFE_INTEGER - 100_000
        || state.next_family_link_id > MAX_SAFE_INTEGER - 100_000
    {
        return Err(ServiceFault::Invalid(
            "Los contadores de identidad están demasiado cerca del límite seguro del navegador."
                .into(),
        ));
    }
    let current_year = state.current_year.map(|year| year.0);
    let start_year = match state.current_year {
        Some(year) => year
            .0
            .checked_add(1)
            .ok_or_else(|| ServiceFault::Invalid("Se alcanzó el límite de años.".into()))?,
        None => state.start_year.0,
    };
    if through_year < start_year {
        return Err(ServiceFault::Invalid(
            "through_year debe ser igual o posterior al siguiente cierre anual.".into(),
        ));
    }
    let total_years = (i64::from(through_year) - i64::from(start_year) + 1) as u64;
    if total_years > 100_000 {
        return Err(ServiceFault::Invalid(
            "Una solicitud puede avanzar como máximo 100.000 años.".into(),
        ));
    }
    Ok(PreparedRun {
        scenario: validated,
        state,
        writer,
        start_year,
        current_year,
        total_years,
    })
}

fn run_worker(
    metrics: Arc<dyn MetricsSink>,
    clock: Arc<dyn MeasurementClock>,
    context: SimulationJobContext,
) {
    let SimulationJobContext {
        world,
        job,
        prepared,
        through_year,
    } = context;
    let PreparedRun {
        scenario,
        state,
        mut writer,
        ..
    } = prepared;
    set_job_status(&job, RunStatus::Running, None);
    let mut engine = match Engine::new(scenario, state) {
        Ok(engine) => engine,
        Err(error) => {
            fail_job(&job, format!("engine initialization failed: {error}"));
            return;
        }
    };
    loop {
        if !await_run_gate(&job, writer.as_mut(), engine.state().current_year.is_some()) {
            break;
        }
        if job.cancellation.is_cancelled() {
            break;
        }
        let next_year = engine
            .state()
            .current_year
            .and_then(|year| year.0.checked_add(1))
            .unwrap_or(engine.state().start_year.0);
        if next_year > through_year {
            break;
        }
        let started = clock.now_micros();
        match engine.advance_year(Some(&job.cancellation)) {
            Ok(commit) => {
                if let Err(error) = writer.commit(&commit) {
                    fail_job(&job, format!("year commit failed: {error:?}"));
                    return;
                }
                let current_year = commit.year.0;
                let population = commit.state.living_people_at(commit.year).count();
                let event_count = commit.events.len();
                update_after_commit(&job, current_year);
                metrics.year_committed(YearMetric {
                    world_id: world.world_id.clone(),
                    branch_id: world.branch_id.clone(),
                    year: current_year,
                    elapsed_micros: clock.now_micros().saturating_sub(started),
                    population,
                    event_count,
                });
            }
            Err(_error) if job.cancellation.is_cancelled() => {
                set_job_status(
                    &job,
                    RunStatus::Cancelled,
                    Some("Cancelado en el límite anual seguro.".into()),
                );
                return;
            }
            Err(error) => {
                fail_job(&job, format!("engine annual step failed: {error}"));
                return;
            }
        }
    }
    if job_snapshot(&job).status == RunStatus::Failed {
        return;
    }
    if job.cancellation.is_cancelled() {
        set_job_status(
            &job,
            RunStatus::Cancelled,
            Some("Cancelado; se conserva el último cierre confirmado.".into()),
        );
    } else {
        set_job_status(&job, RunStatus::Completed, None);
    }
}

fn await_run_gate(
    job: &Arc<JobControl>,
    writer: &mut dyn CommitWriter,
    has_confirmed_close: bool,
) -> bool {
    let mut gate = match job.gate.lock() {
        Ok(gate) => gate,
        Err(_) => {
            fail_job(job, "run gate lock poisoned".into());
            return false;
        }
    };
    if gate.cancelled {
        return false;
    }
    if !gate.pause_requested {
        return true;
    }
    drop(gate);
    if has_confirmed_close {
        if let Err(error) = writer.checkpoint() {
            fail_job(job, format!("pause checkpoint failed: {error:?}"));
            return false;
        }
    }
    set_job_status(
        job,
        RunStatus::Paused,
        Some("En pausa en un cierre anual confirmado.".into()),
    );
    gate = match job.gate.lock() {
        Ok(gate) => gate,
        Err(_) => {
            fail_job(job, "run gate lock poisoned".into());
            return false;
        }
    };
    while gate.pause_requested && !gate.cancelled {
        match job
            .gate_changed
            .wait_timeout(gate, Duration::from_millis(250))
        {
            Ok((next, _)) => gate = next,
            Err(_) => {
                fail_job(job, "run gate wait poisoned".into());
                return false;
            }
        }
    }
    if gate.cancelled {
        return false;
    }
    drop(gate);
    set_job_status(job, RunStatus::Running, None);
    true
}

fn set_job_status(job: &JobControl, status: RunStatus, message: Option<String>) {
    if let Ok(mut snapshot) = job.snapshot.lock() {
        snapshot.status = status;
        snapshot.message = message;
    }
}

fn update_after_commit(job: &JobControl, year: i32) {
    if let Ok(mut snapshot) = job.snapshot.lock() {
        snapshot.current_year = Some(year);
        snapshot.completed_years = snapshot.completed_years.saturating_add(1);
        snapshot.status = RunStatus::Running;
        snapshot.message = None;
    }
}

fn fail_job(job: &JobControl, detail: String) {
    eprintln!("Chargen run failed: {detail}");
    set_job_status(
        job,
        RunStatus::Failed,
        Some("La corrida falló; el último cierre anual confirmado se conserva.".into()),
    );
}

fn job_snapshot(job: &JobControl) -> RunSnapshot {
    job.snapshot
        .lock()
        .map(|snapshot| snapshot.clone())
        .unwrap_or_else(|_| RunSnapshot {
            job_id: "unknown".into(),
            world_id: String::new(),
            branch_id: String::new(),
            status: RunStatus::Failed,
            start_year: 0,
            through_year: 0,
            current_year: None,
            completed_years: 0,
            total_years: 0,
            message: Some("No se pudo leer el estado del trabajo.".into()),
        })
}

async fn get_run(
    State(app): State<AppContainer>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<RunSnapshot>, ServiceFault> {
    let job = app.lookup_job(&job_id)?;
    Ok(Json(job_snapshot(&job)))
}

async fn pause_run(
    State(app): State<AppContainer>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<RunSnapshot>, ServiceFault> {
    let job = app.lookup_job(&job_id)?;
    let mut gate = job
        .gate
        .lock()
        .map_err(|_| ServiceFault::Internal("run gate lock poisoned".into()))?;
    let status = job_snapshot(&job).status;
    if status != RunStatus::Running {
        return Err(ServiceFault::Conflict(
            "Solo se puede pausar un trabajo en marcha.".into(),
        ));
    }
    gate.pause_requested = true;
    drop(gate);
    let mut snapshot = job_snapshot(&job);
    snapshot.message = Some("Pausa solicitada; se aplicará al cerrar el año actual.".into());
    Ok(Json(snapshot))
}

async fn resume_run(
    State(app): State<AppContainer>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<RunSnapshot>, ServiceFault> {
    let job = app.lookup_job(&job_id)?;
    let mut gate = job
        .gate
        .lock()
        .map_err(|_| ServiceFault::Internal("run gate lock poisoned".into()))?;
    if job_snapshot(&job).status != RunStatus::Paused {
        return Err(ServiceFault::Conflict(
            "El trabajo no está en pausa.".into(),
        ));
    }
    gate.pause_requested = false;
    job.gate_changed.notify_all();
    drop(gate);
    Ok(Json(job_snapshot(&job)))
}

async fn cancel_run(
    State(app): State<AppContainer>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<RunSnapshot>, ServiceFault> {
    let job = app.lookup_job(&job_id)?;
    let mut gate = job
        .gate
        .lock()
        .map_err(|_| ServiceFault::Internal("run gate lock poisoned".into()))?;
    let snapshot = job_snapshot(&job);
    if matches!(
        snapshot.status,
        RunStatus::Completed | RunStatus::Cancelled | RunStatus::Failed
    ) {
        return Err(ServiceFault::Conflict("El trabajo ya terminó.".into()));
    }
    gate.cancelled = true;
    gate.pause_requested = false;
    job.cancellation.cancel();
    job.gate_changed.notify_all();
    drop(gate);
    Ok(Json(job_snapshot(&job)))
}

async fn export_world(
    State(app): State<AppContainer>,
    AxumPath(world_id): AxumPath<String>,
) -> Result<Json<ExportResponse>, ServiceFault> {
    validate_key(&world_id, "mundo")?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    let serial = app.export_counter.fetch_add(1, Ordering::Relaxed);
    let export_id = format!("export-{timestamp:x}-{serial:x}");
    let path = app.config.exports_dir.join(format!("{export_id}.sqlite3"));
    let repository = app.repository.clone();
    let world_for_thread = world_id.clone();
    let manifest =
        tokio::task::spawn_blocking(move || repository.export_world(&world_for_thread, &path))
            .await
            .map_err(|error| ServiceFault::Internal(error.to_string()))??;
    if !manifest.world_ids.iter().any(|id| id == &world_id) {
        return Err(ServiceFault::NotFound(
            "No se encontró el mundo solicitado.".into(),
        ));
    }
    Ok(Json(ExportResponse {
        export_id: export_id.clone(),
        manifest,
        download_url: format!("/api/exports/{export_id}"),
    }))
}

async fn download_export(
    State(app): State<AppContainer>,
    AxumPath(export_id): AxumPath<String>,
) -> Result<Response, ServiceFault> {
    validate_key(&export_id, "exportación")?;
    let path = app.config.exports_dir.join(format!("{export_id}.sqlite3"));
    let bytes = tokio::task::spawn_blocking(move || fs::read(path))
        .await
        .map_err(|error| ServiceFault::Internal(error.to_string()))?
        .map_err(|_| ServiceFault::NotFound("No se encontró la exportación solicitada.".into()))?;
    let mut response = Response::new(Body::from(bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.sqlite3"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{export_id}.sqlite3\"")).unwrap(),
    );
    Ok(response)
}

async fn restore_world(
    State(app): State<AppContainer>,
    body: Result<Json<RestoreRequest>, JsonRejection>,
) -> Result<Json<RestoreResponse>, ServiceFault> {
    let Json(request) = body.map_err(map_json_rejection)?;
    validate_key(&request.export_id, "exportación")?;
    let source = app
        .config
        .exports_dir
        .join(format!("{}.sqlite3", request.export_id));
    if !source.is_file() {
        return Err(ServiceFault::NotFound(
            "No se encontró la exportación solicitada.".into(),
        ));
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    let serial = app.export_counter.fetch_add(1, Ordering::Relaxed);
    let restored_id = format!("restored-{timestamp:x}-{serial:x}");
    let destination = app
        .config
        .restores_dir
        .join(format!("{restored_id}.sqlite3"));
    let repository = app.repository.clone();
    let source_for_thread = source.clone();
    let worlds = tokio::task::spawn_blocking(move || {
        repository.restore_world(&source_for_thread, &destination)
    })
    .await
    .map_err(|error| ServiceFault::Internal(error.to_string()))??;
    Ok(Json(RestoreResponse {
        restored_id: restored_id.clone(),
        database_file: format!("restores/{restored_id}.sqlite3"),
        worlds,
        restart_required: true,
    }))
}

fn sanitize_person_record(mut record: PersonRecord, year: Year) -> PersonRecord {
    if record.person.death_year.is_some_and(|death| death > year) {
        record.person.death_year = None;
    }
    record
}

fn validate_context(world_id: &str, branch_id: &str) -> Result<(), ServiceFault> {
    validate_key(world_id, "mundo")?;
    validate_key(branch_id, "rama")
}

fn validate_key(value: &str, label: &str) -> Result<(), ServiceFault> {
    if value.is_empty()
        || value.len() > 80
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(ServiceFault::Invalid(format!("El identificador de {label} solo admite letras, números, guion y guion bajo, hasta 80 caracteres.")));
    }
    Ok(())
}

fn validate_numeric_id(value: u64, label: &str) -> Result<(), ServiceFault> {
    if value > MAX_SAFE_INTEGER {
        return Err(ServiceFault::Invalid(format!(
            "El ID de {label} excede el rango entero exacto del navegador."
        )));
    }
    Ok(())
}

fn validate_safe_scenario_ids(scenario: &Scenario) -> Result<(), ServiceFault> {
    let mut ids = vec![scenario.world_id.0, scenario.seed];
    ids.extend(
        scenario
            .settlements
            .iter()
            .flat_map(|item| [item.id.0, item.logical_id.0]),
    );
    ids.extend(
        scenario
            .initial_people
            .iter()
            .flat_map(|item| [item.id.0, item.logical_id.0]),
    );
    ids.extend(
        scenario
            .initial_households
            .iter()
            .flat_map(|item| [item.id.0, item.logical_id.0]),
    );
    ids.extend(
        scenario
            .initial_unions
            .iter()
            .flat_map(|item| [item.id.0, item.logical_id.0]),
    );
    ids.extend(
        scenario
            .initial_family_links
            .iter()
            .flat_map(|item| [item.id.0, item.logical_id.0]),
    );
    if ids.into_iter().any(|id| id > MAX_SAFE_INTEGER) {
        return Err(ServiceFault::Invalid(
            "Los IDs y la semilla deben quedar dentro del rango entero exacto del navegador."
                .into(),
        ));
    }
    Ok(())
}

fn checked_page(
    config: &ServiceConfig,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<(usize, usize), ServiceFault> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    if limit == 0 || limit > config.max_page_size {
        return Err(ServiceFault::Invalid(format!(
            "limit debe estar entre 1 y {}.",
            config.max_page_size
        )));
    }
    if offset > config.max_offset {
        return Err(ServiceFault::Invalid(format!(
            "offset no puede superar {}.",
            config.max_offset
        )));
    }
    Ok((limit, offset))
}

fn validation_fault(issues: Vec<ValidationIssue>) -> ServiceFault {
    let summary = issues
        .iter()
        .take(6)
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ");
    ServiceFault::Invalid(format!(
        "El escenario tiene errores de validación: {summary}"
    ))
}

fn map_json_rejection(rejection: JsonRejection) -> ServiceFault {
    let status = rejection.status();
    if status == StatusCode::PAYLOAD_TOO_LARGE {
        return ServiceFault::PayloadTooLarge;
    }
    ServiceFault::Invalid("El cuerpo debe ser JSON válido y respetar el contrato de la API.".into())
}

#[derive(Debug, Deserialize)]
struct BenchmarkProfiles {
    profile_version: u32,
    profiles: Vec<BenchmarkProfile>,
}

#[derive(Clone, Debug, Deserialize)]
struct BenchmarkProfile {
    id: String,
    population: u32,
    settlements: u32,
    years: u32,
    seed: u64,
    description: String,
}

#[derive(Debug, Serialize)]
struct BenchmarkReport {
    report_version: u32,
    profile_version: u32,
    profile: String,
    description: String,
    seed: u64,
    engine_version: &'static str,
    rules_version: u32,
    os: &'static str,
    architecture: &'static str,
    cpu_name: Option<String>,
    logical_cpu_count: Option<usize>,
    host_memory_bytes: Option<u64>,
    initial_population: u64,
    mean_population: f64,
    peak_population: u64,
    final_population: u64,
    settlements: u32,
    years: u32,
    total_events: u64,
    initialization_elapsed_micros: u128,
    total_elapsed_micros: u128,
    mean_year_micros: u128,
    median_year_micros: u128,
    max_year_micros: u128,
    engine_mean_year_micros: u128,
    engine_max_year_micros: u128,
    engine_phase_mean_micros: BTreeMap<String, u128>,
    historical_census_micros: u128,
    database_bytes: u64,
    sampled_peak_rss_bytes: Option<u64>,
    deterministic_digest: String,
}

/// Stable command-line help for the standalone local executable.
pub fn command_usage() -> &'static str {
    "Chargen E1 local population-history prototype\n\nUso: chargen-app [serve] [--database RUTA] [--bind 127.0.0.1:7878] [--no-open]\n     chargen-app bench --profile B0|B1|B2 [--database RUTA] [--output RUTA]"
}

/// Run a fully local, reproducible workload and emit measured runtime, query,
/// storage, memory samples and a deterministic result fingerprint.
pub fn run_bench_cli(args: &[String]) -> Result<(), String> {
    let mut profile_id: Option<String> = None;
    let mut database: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--profile" => {
                index += 1;
                profile_id = Some(
                    args.get(index)
                        .ok_or("Falta el perfil para --profile.")?
                        .clone(),
                );
            }
            "--database" => {
                index += 1;
                database = Some(PathBuf::from(
                    args.get(index).ok_or("Falta la ruta para --database.")?,
                ));
            }
            "--output" => {
                index += 1;
                output = Some(PathBuf::from(
                    args.get(index).ok_or("Falta la ruta para --output.")?,
                ));
            }
            option => {
                return Err(format!(
                    "Opción desconocida para bench: {option}. {}",
                    command_usage()
                ))
            }
        }
        index += 1;
    }
    let profile_id = profile_id.ok_or_else(|| format!("Falta --profile. {}", command_usage()))?;
    let profiles: BenchmarkProfiles =
        serde_json::from_str(include_str!("../../../scenarios/benchmarks/profiles.json"))
            .map_err(|_| "No se pudo leer el catálogo de perfiles B0–B2.".to_owned())?;
    let profile = profiles
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| format!("Perfil desconocido: {profile_id}. Usa B0, B1 o B2."))?;
    let database = database.unwrap_or_else(|| {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();
        env::temp_dir().join(format!("chargen-bench-{}-{stamp}.sqlite3", profile.id))
    });
    if database.exists() {
        return Err(format!(
            "La base de benchmark ya existe: {}. Elige una ruta nueva para conservar sus datos.",
            database.display()
        ));
    }
    let report = run_benchmark(profile, profiles.profile_version, &database)?;
    let serialized = serde_json::to_string_pretty(&report)
        .map_err(|_| "No se pudo serializar el informe de benchmark.".to_owned())?;
    if let Some(output) = output {
        if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
            fs::create_dir_all(parent)
                .map_err(|_| "No se pudo crear el directorio del informe.".to_owned())?;
        }
        fs::write(&output, format!("{serialized}\n"))
            .map_err(|_| "No se pudo escribir el informe de benchmark.".to_owned())?;
        println!("Informe B{}: {}", report.profile, output.display());
        println!("Huella determinista: {}", report.deterministic_digest);
    } else {
        println!("{serialized}");
    }
    Ok(())
}

fn run_benchmark(
    profile: &BenchmarkProfile,
    profile_version: u32,
    database: &Path,
) -> Result<BenchmarkReport, String> {
    let initialization_started = Instant::now();
    let mut scenario: Scenario = serde_json::from_str(DEMO_SCENARIO).map_err(|_| {
        "El escenario demostrativo no coincide con el modelo de dominio.".to_owned()
    })?;
    configure_benchmark_scenario(&mut scenario, profile)?;
    let validated = scenario.validate().map_err(|issues| {
        format!(
            "Perfil {0} inválido: {1}",
            profile.id,
            issues
                .iter()
                .take(5)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        )
    })?;
    let initial_state = chargen_engine::generate_initial_state(&validated)
        .map_err(|_| "No se pudo materializar el escenario de benchmark.".to_owned())?;
    let world_id = scenario.world_id.0.to_string();
    let scenario_json = serde_json::to_string(&scenario)
        .map_err(|_| "No se pudo serializar el escenario de benchmark.".to_owned())?;
    let store = HistoryStore::open(database)
        .map_err(|error| format!("No se pudo abrir la base de benchmark: {error}"))?;
    store
        .create_world(&world_id, "main", &scenario_json, &initial_state)
        .map_err(|error| format!("No se pudo crear el mundo de benchmark: {error}"))?;
    let mut writer = store
        .acquire_writer(&world_id, "main")
        .map_err(|error| format!("No se pudo reservar la rama de benchmark: {error}"))?;
    let mut engine = Engine::new(validated, initial_state)
        .map_err(|_| "El motor rechazó el estado inicial de benchmark.".to_owned())?;
    let initialization_micros = initialization_started.elapsed().as_micros();

    let mut year_durations = Vec::with_capacity(profile.years as usize);
    let mut total_events = 0_u64;
    let mut total_population = 0_u128;
    let mut peak_population = 0_u64;
    let mut engine_year_total_nanos = 0_u128;
    let mut engine_year_max_nanos = 0_u128;
    let mut engine_phase_total_nanos = BTreeMap::<String, u128>::new();
    let mut digest = DeterministicDigest::default();
    let mut peak_rss = resident_bytes();
    for _ in 0..profile.years {
        let started = Instant::now();
        let commit = engine
            .advance_year(None)
            .map_err(|_| "El motor falló al cerrar un año de benchmark.".to_owned())?;
        writer.commit_year(&commit).map_err(|error| {
            format!("No se pudo confirmar un cierre anual de benchmark: {error}")
        })?;
        let elapsed = started.elapsed().as_micros();
        let population = commit.state.living_people_at(commit.year).count() as u64;
        total_population = total_population.saturating_add(u128::from(population));
        peak_population = peak_population.max(population);
        total_events = total_events.saturating_add(commit.events.len() as u64);
        if let Some(metrics) = engine.last_metrics() {
            engine_year_total_nanos =
                engine_year_total_nanos.saturating_add(metrics.total_duration_nanos);
            engine_year_max_nanos = engine_year_max_nanos.max(metrics.total_duration_nanos);
            for phase in &metrics.phases {
                let name = format!("{:?}", phase.phase);
                let total = engine_phase_total_nanos.entry(name).or_default();
                *total = total.saturating_add(phase.duration_nanos);
            }
        }
        digest.include_commit(&commit)?;
        year_durations.push(elapsed);
        if let Some(sample) = resident_bytes() {
            peak_rss = Some(peak_rss.unwrap_or_default().max(sample));
        }
    }
    writer
        .checkpoint()
        .map_err(|error| format!("No se pudo guardar el checkpoint del benchmark: {error}"))?;
    drop(writer);
    // Sum annual engine+durable-commit timings so the diagnostic full-state
    // digest does not inflate the benchmark's simulation runtime.
    let total_elapsed_micros = year_durations.iter().copied().sum::<u128>();
    let final_year = scenario
        .start_year
        .0
        .checked_add(profile.years as i32 - 1)
        .ok_or_else(|| "El año de benchmark excede el rango del dominio.".to_owned())?;
    let query_started = Instant::now();
    let query =
        HistoryQuery::open(database, &world_id, "main", Year(final_year)).map_err(|error| {
            format!("No se pudo abrir la consulta histórica del benchmark: {error}")
        })?;
    let final_population = query.census(None).total_population;
    let historical_census_micros = query_started.elapsed().as_micros();
    let database_bytes = fs::metadata(database)
        .map_err(|_| "No se pudo medir el archivo histórico del benchmark.".to_owned())?
        .len();
    let mut sorted = year_durations.clone();
    sorted.sort_unstable();
    let mean_year_micros = sorted.iter().copied().sum::<u128>() / sorted.len().max(1) as u128;
    let median_year_micros = sorted.get(sorted.len() / 2).copied().unwrap_or_default();
    let max_year_micros = sorted.last().copied().unwrap_or_default();
    let years = u128::from(profile.years);
    let engine_phase_mean_micros = engine_phase_total_nanos
        .into_iter()
        .map(|(phase, total)| (phase, total / years / 1_000))
        .collect();
    Ok(BenchmarkReport {
        report_version: 2,
        profile_version,
        profile: profile.id.clone(),
        description: profile.description.clone(),
        seed: profile.seed,
        engine_version: env!("CARGO_PKG_VERSION"),
        rules_version: scenario.rules_version,
        os: env::consts::OS,
        architecture: env::consts::ARCH,
        cpu_name: cpu_name(),
        logical_cpu_count: std::thread::available_parallelism()
            .ok()
            .map(|count| count.get()),
        host_memory_bytes: host_memory_bytes(),
        initial_population: profile.population.into(),
        mean_population: total_population as f64 / f64::from(profile.years),
        peak_population,
        final_population,
        settlements: profile.settlements,
        years: profile.years,
        total_events,
        initialization_elapsed_micros: initialization_micros,
        total_elapsed_micros,
        mean_year_micros,
        median_year_micros,
        max_year_micros,
        engine_mean_year_micros: engine_year_total_nanos / years / 1_000,
        engine_max_year_micros: engine_year_max_nanos / 1_000,
        engine_phase_mean_micros,
        historical_census_micros,
        database_bytes,
        sampled_peak_rss_bytes: peak_rss,
        deterministic_digest: digest.finish(),
    })
}

fn configure_benchmark_scenario(
    scenario: &mut Scenario,
    profile: &BenchmarkProfile,
) -> Result<(), String> {
    use chargen_domain::{
        Gamete, HouseholdKind, InitialPopulationProfile, LogicalId, ReproductiveProfile,
        Settlement, SettlementId, WeightedAgeBand, WeightedReproductiveProfile, WeightedTraitValue,
        WorldId, Year,
    };
    use std::collections::{BTreeMap, BTreeSet};

    if profile.settlements == 0
        || profile.population == 0
        || profile.years == 0
        || profile.years > i32::MAX as u32
    {
        return Err(format!(
            "El perfil {} tiene dimensiones fuera de rango.",
            profile.id
        ));
    }
    scenario.world_id = WorldId(profile.seed);
    scenario.seed = profile.seed;
    scenario.start_year = Year(0);
    let template = scenario
        .settlements
        .first()
        .cloned()
        .ok_or("El demo no tiene asentamientos.")?;
    scenario.settlements = (0..profile.settlements)
        .map(|index| Settlement {
            id: SettlementId(u64::from(index) + 1),
            logical_id: LogicalId(10_001 + u64::from(index)),
            name: format!("Asentamiento {:03}", index + 1),
            culture_values: template.culture_values.clone(),
        })
        .collect();
    let ovum = ReproductiveProfile {
        gametes: BTreeSet::from([Gamete::Ovum]),
        can_gestate: true,
        fertility_enabled: true,
    };
    let sperm = ReproductiveProfile {
        gametes: BTreeSet::from([Gamete::Sperm]),
        can_gestate: false,
        fertility_enabled: true,
    };
    scenario.initial_people.clear();
    scenario.initial_households.clear();
    scenario.initial_unions.clear();
    scenario.initial_family_links.clear();
    let mut remaining = profile.population;
    scenario.initial_population_profiles = (0..profile.settlements)
        .map(|index| {
            let settlements_left = profile.settlements - index;
            let count = remaining.div_ceil(settlements_left);
            remaining -= count;
            let mut trait_distributions = BTreeMap::new();
            for definition in &scenario.trait_catalog {
                trait_distributions.insert(
                    definition.key.clone(),
                    vec![WeightedTraitValue {
                        value: (i32::from(definition.minimum)
                            + (i32::from(definition.maximum) - i32::from(definition.minimum)) / 2)
                            as i16,
                        weight: 1,
                    }],
                );
            }
            InitialPopulationProfile {
                settlement_id: SettlementId(u64::from(index) + 1),
                population_size: count,
                household_size_minimum: 1,
                household_size_maximum: 5,
                household_kind: HouseholdKind::Family,
                age_bands: vec![WeightedAgeBand {
                    minimum_age: 18,
                    maximum_age: 45,
                    weight: 1,
                }],
                reproductive_profiles: vec![
                    WeightedReproductiveProfile {
                        profile: ovum.clone(),
                        weight: 1,
                    },
                    WeightedReproductiveProfile {
                        profile: sperm.clone(),
                        weight: 1,
                    },
                ],
                trait_distributions,
            }
        })
        .collect();
    scenario.policies.births.child_reproductive_profiles = vec![
        WeightedReproductiveProfile {
            profile: ovum,
            weight: 1,
        },
        WeightedReproductiveProfile {
            profile: sperm,
            weight: 1,
        },
    ];
    Ok(())
}

#[derive(Default)]
struct DeterministicDigest(u64);

impl DeterministicDigest {
    fn include_commit(&mut self, commit: &YearCommit) -> Result<(), String> {
        if self.0 == 0 {
            self.0 = 0xcbf29ce484222325;
        }
        self.write(&commit.year.0.to_le_bytes());
        let encoded_events = serde_json::to_vec(&commit.events)
            .map_err(|_| "No se pudo serializar un lote de eventos para la huella.".to_owned())?;
        self.write(&encoded_events);
        // Hash the full durable state, including IDs, ages, relationships,
        // traits and genetics, not only the census projection.
        let encoded_state = serde_json::to_vec(&commit.state)
            .map_err(|_| "No se pudo serializar el estado para la huella.".to_owned())?;
        self.write(&encoded_state);
        Ok(())
    }

    fn write(&mut self, bytes: &[u8]) {
        if self.0 == 0 {
            self.0 = 0xcbf29ce484222325;
        }
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(&self) -> String {
        format!(
            "{:016x}",
            if self.0 == 0 {
                0xcbf29ce484222325
            } else {
                self.0
            }
        )
    }
}

fn resident_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = fs::read_to_string("/proc/self/status").ok()?;
        let kibibytes = status.lines().find_map(|line| {
            line.strip_prefix("VmRSS:")?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        })?;
        return Some(kibibytes.saturating_mul(1024));
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()?;
        return String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .ok()
            .map(|kibibytes| kibibytes.saturating_mul(1024));
    }
    #[cfg(target_os = "windows")]
    {
        let pid = std::process::id().to_string();
        let output = Command::new("tasklist")
            .args(["/FO", "CSV", "/NH", "/FI", &format!("PID eq {pid}")])
            .output()
            .ok()?;
        let row = String::from_utf8_lossy(&output.stdout);
        let memory = row
            .trim()
            .trim_matches('"')
            .split("\",\"")
            .last()?
            .split_whitespace()
            .next()?
            .replace(',', "");
        return memory
            .parse::<u64>()
            .ok()
            .map(|kibibytes| kibibytes.saturating_mul(1024));
    }
    #[allow(unreachable_code)]
    None
}

fn cpu_name() -> Option<String> {
    if let Ok(value) = env::var("PROCESSOR_IDENTIFIER") {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        if let Ok(contents) = fs::read_to_string("/proc/cpuinfo") {
            if let Some(model) = contents.lines().find_map(|line| {
                line.strip_prefix("model name\t: ")
                    .or_else(|| line.strip_prefix("Hardware\t: "))
                    .map(str::trim)
            }) {
                return Some(model.to_owned());
            }
        }
        #[cfg(target_os = "macos")]
        if let Ok(output) = Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
        {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn host_memory_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let contents = fs::read_to_string("/proc/meminfo").ok()?;
        let kibibytes = contents.lines().find_map(|line| {
            line.strip_prefix("MemTotal:")?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        })?;
        return Some(kibibytes.saturating_mul(1024));
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        return String::from_utf8_lossy(&output.stdout).trim().parse().ok();
    }
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
            ])
            .output()
            .ok()?;
        return String::from_utf8_lossy(&output.stdout).trim().parse().ok();
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    struct StepClock(AtomicU64);

    impl MeasurementClock for StepClock {
        fn now_micros(&self) -> u128 {
            u128::from(self.0.fetch_add(10, Ordering::SeqCst))
        }
    }

    #[derive(Default)]
    struct RecordingMetrics(Mutex<Vec<YearMetric>>);

    impl MetricsSink for RecordingMetrics {
        fn year_committed(&self, metric: YearMetric) {
            self.0.lock().unwrap().push(metric);
        }
    }

    struct RecordingWriter(Arc<Mutex<Vec<YearCommit>>>);

    impl CommitWriter for RecordingWriter {
        fn commit(&mut self, year: &YearCommit) -> Result<(), ServiceFault> {
            self.0.lock().unwrap().push(year.clone());
            Ok(())
        }

        fn checkpoint(&mut self) -> Result<(), ServiceFault> {
            Ok(())
        }
    }

    struct UnusedRepository;

    impl HistoryRepository for UnusedRepository {
        fn create_world(&self, _: &str, _: &str, _: &WorldState) -> Result<(), ServiceFault> {
            unreachable!()
        }
        fn list_worlds(&self) -> Result<Vec<WorldSummary>, ServiceFault> {
            unreachable!()
        }
        fn list_branches(&self, _: &str) -> Result<Vec<BranchSummary>, ServiceFault> {
            unreachable!()
        }
        fn scenario_json(&self, _: &str, _: &str) -> Result<String, ServiceFault> {
            unreachable!()
        }
        fn head(&self, _: &str, _: &str) -> Result<Option<Year>, ServiceFault> {
            unreachable!()
        }
        fn load_state(&self, _: &str, _: &str) -> Result<WorldState, ServiceFault> {
            unreachable!()
        }
        fn load_state_at(&self, _: &str, _: &str, _: Year) -> Result<WorldState, ServiceFault> {
            unreachable!()
        }
        fn fork_branch(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: Year,
            _: &str,
        ) -> Result<WorldState, ServiceFault> {
            unreachable!()
        }
        fn fork_initial_branch(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<WorldState, ServiceFault> {
            unreachable!()
        }
        fn acquire_writer(&self, _: &str, _: &str) -> Result<Box<dyn CommitWriter>, ServiceFault> {
            unreachable!()
        }
        fn checkpoint(&self, _: &str, _: &str) -> Result<(), ServiceFault> {
            unreachable!()
        }
        fn export_world(&self, _: &str, _: &Path) -> Result<ExportManifest, ServiceFault> {
            unreachable!()
        }
        fn restore_world(&self, _: &Path, _: &Path) -> Result<Vec<String>, ServiceFault> {
            unreachable!()
        }
    }

    #[test]
    fn service_rejects_public_bind_addresses() {
        let mut config = ServiceConfig::new("/tmp/chargen-test.sqlite3");
        config.bind = "0.0.0.0:7878".parse().unwrap();
        assert!(matches!(config.validate(), Err(ServiceFault::Invalid(_))));
    }

    #[test]
    fn ids_and_page_limits_are_checked_before_storage_reads() {
        assert!(validate_key("main/../../tmp", "rama").is_err());
        assert!(validate_numeric_id(MAX_SAFE_INTEGER + 1, "persona").is_err());
        let mut config = ServiceConfig::new("/tmp/chargen-test.sqlite3");
        config.max_page_size = 100;
        assert!(checked_page(&config, Some(101), Some(0)).is_err());
        assert!(checked_page(&config, Some(25), Some(10_001)).is_err());
    }

    #[test]
    fn worker_uses_injected_clock_metrics_and_commit_writer() {
        let scenario: Scenario = serde_json::from_str(DEMO_SCENARIO).unwrap();
        let validated = scenario.validate().unwrap();
        let state = chargen_engine::generate_initial_state(&validated).unwrap();
        let start_year = state.start_year.0;
        let world_id = scenario.world_id.0.to_string();
        let commits = Arc::new(Mutex::new(Vec::new()));
        let records = Arc::new(RecordingMetrics::default());
        let job = Arc::new(JobControl {
            cancellation: CancellationToken::new(),
            gate: Mutex::new(JobGate::default()),
            gate_changed: Condvar::new(),
            snapshot: Mutex::new(RunSnapshot {
                job_id: "job-test".into(),
                world_id: world_id.clone(),
                branch_id: "main".into(),
                status: RunStatus::Queued,
                start_year,
                through_year: start_year,
                current_year: None,
                completed_years: 0,
                total_years: 1,
                message: None,
            }),
        });
        let context = SimulationJobContext {
            world: WorldServices {
                world_id: world_id.clone(),
                branch_id: "main".into(),
                repository: Arc::new(UnusedRepository),
            },
            job: job.clone(),
            prepared: PreparedRun {
                scenario: validated,
                state,
                writer: Box::new(RecordingWriter(commits.clone())),
                start_year,
                current_year: None,
                total_years: 1,
            },
            through_year: start_year,
        };

        run_worker(
            records.clone(),
            Arc::new(StepClock(AtomicU64::new(0))),
            context,
        );

        let commits = commits.lock().unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].year, Year(start_year));
        let metrics = records.0.lock().unwrap();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].elapsed_micros, 10);
        assert_eq!(metrics[0].year, start_year);
        assert_eq!(job_snapshot(&job).status, RunStatus::Completed);
    }
}
