//! Versioned SQLite history for a deterministic chargen world.
//!
//! A database can hold several worlds. Writers are serialized per world with
//! an operating-system file lock, and each annual state/event batch is one
//! SQLite transaction. Readers use a separate read-only connection.

mod schema;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use chargen_domain::{HistoricalEvent, Scenario, State, ValidatedScenario, Year};
use chargen_engine::{generate_initial_state, YearCommit};
use rusqlite::{
    backup::Backup, params, Connection, OpenFlags, OptionalExtension, Transaction,
    TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const CURRENT_SCHEMA_VERSION: u32 = schema::CURRENT_SCHEMA_VERSION;
pub const DEFAULT_SNAPSHOT_INTERVAL: i32 = 25;

/// Open a catalog/history SQLite file. The object stores only its path and is
/// safe to share; operations open their own connections.
#[derive(Clone, Debug)]
pub struct HistoryStore {
    path: PathBuf,
    snapshot_interval: i32,
}

/// A single writer session scoped to one world and branch. It is intentionally
/// owned by the simulation worker for the life of the run.
pub struct WorldWriter {
    conn: Connection,
    _lock_file: File,
    world_id: String,
    branch_id: String,
    snapshot_interval: i32,
    cached_state: State,
    cached_head_year: Option<i32>,
    cached_head_hash: String,
    validated_scenario: ValidatedScenario,
    #[cfg(test)]
    fail_at: Option<CommitFailurePoint>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CommitFailurePoint {
    AfterCommitRow,
    AfterEventInsert,
    BeforeHeadUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldSummary {
    pub world_id: String,
    pub created_at: String,
    pub root_branch_id: String,
    pub branch_count: u64,
    pub latest_year: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchSummary {
    pub world_id: String,
    pub branch_id: String,
    pub parent_branch_id: Option<String>,
    pub fork_year: Option<i32>,
    pub head_year: Option<i32>,
    pub revision_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportManifest {
    pub format_version: u32,
    pub schema_version: u32,
    pub world_ids: Vec<String>,
    pub database_file: String,
    pub byte_length: u64,
    /// FNV-1a-64 for accidental-corruption detection; this is not a signature.
    pub checksum_fnv1a64: String,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("SQLite history error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("history serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("history I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("world '{0}' already exists")]
    WorldExists(String),
    #[error("world '{0}' does not exist")]
    WorldNotFound(String),
    #[error("branch '{branch_id}' does not exist in world '{world_id}'")]
    BranchNotFound { world_id: String, branch_id: String },
    #[error("writer for world '{0}' is already active")]
    WriterAlreadyActive(String),
    #[error("schema version {found} is newer than supported version {supported}")]
    NewerSchema { found: u32, supported: u32 },
    #[error("history integrity check failed: {0}")]
    Integrity(String),
    #[error("invalid history operation: {0}")]
    Invalid(String),
    #[error("requested year {requested} is not a committed year for branch '{branch_id}'")]
    YearUnavailable { branch_id: String, requested: i32 },
    #[error("scenario validation failed: {0}")]
    ScenarioInvalid(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

impl HistoryStore {
    /// Open or create a catalog/history database and apply all known migrations.
    pub fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let conn = open_rw(&path)?;
        schema::migrate(&conn)?;
        Ok(Self {
            path,
            snapshot_interval: DEFAULT_SNAPSHOT_INTERVAL,
        })
    }

    /// Open an existing database without running migrations or changing it.
    /// The query crate uses this path to keep reads strictly read-only.
    pub fn open_existing(path: impl AsRef<Path>) -> StoreResult<Self> {
        let path = path.as_ref().to_path_buf();
        let conn = open_ro(&path)?;
        let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version != CURRENT_SCHEMA_VERSION {
            return Err(StoreError::Invalid(format!(
                "query requires schema version {CURRENT_SCHEMA_VERSION}, found {version}"
            )));
        }
        Ok(Self {
            path,
            snapshot_interval: DEFAULT_SNAPSHOT_INTERVAL,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Change snapshot cadence for future commits. A zero or negative cadence
    /// disables automatic snapshots; explicit checkpoints remain available.
    pub fn with_snapshot_interval(mut self, years: i32) -> Self {
        self.snapshot_interval = years;
        self
    }

    /// Create the root branch and persist the initial state. Scenario JSON is
    /// validated by the caller's domain validator and stored verbatim as a
    /// versioned revision.
    pub fn create_world(
        &self,
        world_id: &str,
        root_branch_id: &str,
        scenario_json: &str,
        initial_state: &State,
    ) -> StoreResult<()> {
        let scenario = parse_and_validate_scenario(scenario_json, initial_state)?;
        if json_id(&scenario.world_id)? != world_id {
            return Err(StoreError::ScenarioInvalid(
                "scenario world_id does not match requested world".into(),
            ));
        }
        validate_state_identity(initial_state, world_id)?;
        if initial_state.current_year.is_some() {
            return Err(StoreError::Invalid(
                "the initial world state must not have a confirmed year".into(),
            ));
        }
        let core = state_core_value(initial_state)?;
        let core_json = canonical_json(&core)?;
        let now = utc_timestamp();
        let conn = open_rw(&self.path)?;
        let tx = conn.unchecked_transaction()?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM worlds WHERE world_id=?1)",
            [world_id],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(StoreError::WorldExists(world_id.to_owned()));
        }
        tx.execute(
            "INSERT INTO worlds(world_id,created_at,format_version,schema_version,rules_version,seed,root_branch_id) VALUES(?1,?2,1,?3,?4,?5,?6)",
            params![world_id, now, CURRENT_SCHEMA_VERSION, initial_state.rules_version, initial_state.seed.to_string(), root_branch_id],
        )?;
        let revision_id = insert_revision(&tx, world_id, root_branch_id, None, scenario_json)?;
        tx.execute(
            "INSERT INTO branches(world_id,branch_id,parent_branch_id,fork_year,base_year,base_state_json,base_state_hash,base_chain_hash,head_year,head_hash,revision_id,created_at) VALUES(?1,?2,NULL,NULL,NULL,?3,?4,?4,NULL,?4,?5,?6)",
            params![world_id, root_branch_id, core_json, fnv_hex(core_json.as_bytes()), revision_id, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Acquire the exclusive writer lease for a world. The OS releases this
    /// lock if the process exits unexpectedly.
    pub fn acquire_writer(&self, world_id: &str, branch_id: &str) -> StoreResult<WorldWriter> {
        let lock_path = lock_path(&self.path, world_id);
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        match lock_file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(StoreError::WriterAlreadyActive(world_id.to_owned()))
            }
            Err(TryLockError::Error(error)) => return Err(StoreError::Io(error)),
        }
        let conn = open_rw(&self.path)?;
        let exists = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM branches WHERE world_id=?1 AND branch_id=?2)",
            params![world_id, branch_id],
            |r| r.get::<_, bool>(0),
        )?;
        if !exists {
            let world_exists = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM worlds WHERE world_id=?1)",
                [world_id],
                |r| r.get::<_, bool>(0),
            )?;
            if !world_exists {
                return Err(StoreError::WorldNotFound(world_id.to_owned()));
            }
            return Err(StoreError::BranchNotFound {
                world_id: world_id.to_owned(),
                branch_id: branch_id.to_owned(),
            });
        }
        let branch = get_branch(&conn, world_id, branch_id)?;
        let cached_state = if let Some(year) = branch.head_year {
            self.load_state_at_i32(world_id, branch_id, year)?
        } else {
            serde_json::from_str(&branch.base_state_json)?
        };
        let scenario_json: String = conn.query_row(
            "SELECT scenario_json FROM scenario_revisions WHERE revision_id=?1",
            [branch.revision_id],
            |row| row.get(0),
        )?;
        let validated_scenario = parse_validated_scenario(&scenario_json)?;
        validated_scenario
            .validate_state(&cached_state)
            .map_err(|issues| {
                StoreError::ScenarioInvalid(format!(
                    "saved branch state does not match its active scenario: {}",
                    issues
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ))
            })?;
        Ok(WorldWriter {
            conn,
            _lock_file: lock_file,
            world_id: world_id.to_owned(),
            branch_id: branch_id.to_owned(),
            snapshot_interval: self.snapshot_interval,
            cached_state,
            cached_head_year: branch.head_year,
            cached_head_hash: branch.head_hash,
            validated_scenario,
            #[cfg(test)]
            fail_at: None,
        })
    }

    /// Materialize the parent at `cut_year` and create a child with a new
    /// scenario revision. The child starts exactly at the cutoff; it never
    /// receives the parent's already-simulated future.
    pub fn fork_branch(
        &self,
        world_id: &str,
        parent_branch_id: &str,
        child_branch_id: &str,
        cut_year: Year,
        scenario_json: &str,
    ) -> StoreResult<State> {
        let cutoff = year_value(cut_year)?;
        let fork_state = self.load_state_at(world_id, parent_branch_id, cut_year)?;
        let revised_scenario = parse_and_validate_scenario(scenario_json, &fork_state)?;
        let parent = get_branch(&open_ro(&self.path)?, world_id, parent_branch_id)?;
        if parent.head_year.is_none_or(|head| cutoff > head) {
            return Err(StoreError::YearUnavailable {
                branch_id: parent_branch_id.to_owned(),
                requested: cutoff,
            });
        }
        if fork_state.current_year.map(year_value).transpose()? != Some(cutoff) {
            return Err(StoreError::Integrity(
                "materialized fork state does not end at requested cutoff".into(),
            ));
        }
        let parent_scenario: Scenario =
            serde_json::from_str(&self.scenario_json(world_id, parent_branch_id)?)?;
        ensure_only_policies_changed(&parent_scenario, &revised_scenario)?;
        let core = state_core_value(&fork_state)?;
        let core_json = canonical_json(&core)?;
        let full_json = canonical_json(&core)?;
        let now = utc_timestamp();
        let conn = open_rw(&self.path)?;
        let tx = conn.unchecked_transaction()?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM branches WHERE world_id=?1 AND branch_id=?2)",
            params![world_id, child_branch_id],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(StoreError::Invalid(format!(
                "branch '{child_branch_id}' already exists"
            )));
        }
        let revision_id =
            insert_revision(&tx, world_id, child_branch_id, Some(cutoff), scenario_json)?;
        let branch_hash = effective_hash_at(&tx, world_id, parent_branch_id, cutoff)?;
        tx.execute(
            "INSERT INTO branches(world_id,branch_id,parent_branch_id,fork_year,base_year,base_state_json,base_state_hash,base_chain_hash,head_year,head_hash,revision_id,created_at) VALUES(?1,?2,?3,?4,?4,?5,?6,?7,?4,?7,?8,?9)",
            params![world_id, child_branch_id, parent_branch_id, cutoff, full_json, fnv_hex(core_json.as_bytes()), branch_hash, revision_id, now],
        )?;
        tx.commit()?;
        Ok(fork_state)
    }

    /// Fork before the first processed year. The child materializes a fresh
    /// initial state from its scenario, so initial-population edits take effect
    /// without changing the parent or inheriting its future.
    pub fn fork_initial_branch(
        &self,
        world_id: &str,
        parent_branch_id: &str,
        child_branch_id: &str,
        scenario_json: &str,
    ) -> StoreResult<State> {
        let conn = open_ro(&self.path)?;
        let _parent = get_branch(&conn, world_id, parent_branch_id)?;
        let scenario = parse_validated_scenario(scenario_json)?;
        if json_id(&scenario.scenario().world_id)? != world_id {
            return Err(StoreError::ScenarioInvalid(
                "scenario world_id does not match requested world".into(),
            ));
        }
        let initial_state = generate_initial_state(&scenario).map_err(|error| {
            StoreError::ScenarioInvalid(format!(
                "cannot materialize edited initial scenario: {error}"
            ))
        })?;
        let core = state_core_value(&initial_state)?;
        let core_json = canonical_json(&core)?;
        let base_hash = fnv_hex(core_json.as_bytes());
        drop(conn);
        let now = utc_timestamp();
        let conn = open_rw(&self.path)?;
        let tx = conn.unchecked_transaction()?;
        if tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM branches WHERE world_id=?1 AND branch_id=?2)",
            params![world_id, child_branch_id],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(StoreError::Invalid(format!(
                "branch '{child_branch_id}' already exists"
            )));
        }
        let revision_id = insert_revision(&tx, world_id, child_branch_id, None, scenario_json)?;
        let base_chain_hash = base_hash.clone();
        tx.execute(
            "INSERT INTO branches(world_id,branch_id,parent_branch_id,fork_year,base_year,base_state_json,base_state_hash,base_chain_hash,head_year,head_hash,revision_id,created_at) VALUES(?1,?2,?3,NULL,NULL,?4,?5,?6,NULL,?6,?7,?8)",
            params![world_id, child_branch_id, parent_branch_id, core_json, base_hash, base_chain_hash, revision_id, now],
        )?;
        tx.commit()?;
        Ok(initial_state)
    }

    /// Resume from the last confirmed close. The returned state is an owned
    /// value and can be passed directly to `Engine::new`.
    pub fn load_state(&self, world_id: &str, branch_id: &str) -> StoreResult<State> {
        let branch = get_branch(&open_ro(&self.path)?, world_id, branch_id)?;
        if let Some(year) = branch.head_year {
            self.load_state_at_i32(world_id, branch_id, year)
        } else {
            Ok(serde_json::from_str(&branch.base_state_json)?)
        }
    }

    pub fn load_state_at(&self, world_id: &str, branch_id: &str, year: Year) -> StoreResult<State> {
        self.load_state_at_i32(world_id, branch_id, year_value(year)?)
    }

    fn load_state_at_i32(&self, world_id: &str, branch_id: &str, year: i32) -> StoreResult<State> {
        let conn = open_ro(&self.path)?;
        let branch = get_branch(&conn, world_id, branch_id)?;
        if branch.base_year.is_some_and(|base| year <= base) {
            if let Some(parent) = branch.parent_branch_id.as_deref() {
                return self.load_state_at_i32(world_id, parent, year);
            }
        }
        if branch.head_year.is_none_or(|head| year > head) {
            return Err(StoreError::YearUnavailable {
                branch_id: branch_id.to_owned(),
                requested: year,
            });
        }
        let mut base_state: State = serde_json::from_str(&branch.base_state_json)?;
        let start_year = year_value(base_state.start_year)?;
        if year < start_year && branch.base_year.is_none() {
            return Err(StoreError::YearUnavailable {
                branch_id: branch_id.to_owned(),
                requested: year,
            });
        }
        let mut core = state_core_value(&base_state)?;
        if fnv_hex(canonical_json(&core)?.as_bytes()) != branch.base_hash {
            return Err(StoreError::Integrity(
                "branch base-state checksum mismatch".into(),
            ));
        }
        let mut chain_hash = branch.base_chain_hash.clone();
        let snapshot = conn.query_row(
            "SELECT year,state_json,state_hash,head_hash FROM snapshots WHERE world_id=?1 AND branch_id=?2 AND year<=?3 AND (?4 IS NULL OR year>?4) ORDER BY year DESC LIMIT 1",
            params![world_id, branch_id, year, branch.base_year],
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)),
        ).optional()?;
        let mut after_year = branch.base_year;
        if let Some((snapshot_year, snapshot_json, expected_state_hash, snapshot_chain_hash)) =
            snapshot
        {
            if fnv_hex(snapshot_json.as_bytes()) != expected_state_hash {
                return Err(StoreError::Integrity(format!(
                    "snapshot checksum mismatch at year {snapshot_year}"
                )));
            }
            core = serde_json::from_str(&snapshot_json)?;
            chain_hash = snapshot_chain_hash;
            let snapshot_commit: (String, String, String, String, i64) = conn.query_row(
                "SELECT state_hash,commit_hash,previous_hash,events_hash,event_count FROM annual_commits WHERE world_id=?1 AND branch_id=?2 AND year=?3",
                params![world_id, branch_id, snapshot_year],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )?;
            if snapshot_commit.0 != expected_state_hash || snapshot_commit.1 != chain_hash {
                return Err(StoreError::Integrity(format!(
                    "snapshot does not match annual close {snapshot_year}"
                )));
            }
            let event_payloads = read_event_payloads(&conn, world_id, branch_id, snapshot_year)?;
            if event_payloads.len() as i64 != snapshot_commit.4
                || events_hash_value(&event_payloads) != snapshot_commit.3
                || commit_hash_value(
                    &snapshot_commit.2,
                    snapshot_year,
                    &snapshot_commit.0,
                    &snapshot_commit.3,
                ) != snapshot_commit.1
            {
                return Err(StoreError::Integrity(format!(
                    "snapshot close event or commit checksum mismatch at year {snapshot_year}"
                )));
            }
            after_year = Some(snapshot_year);
        }
        let mut stmt = conn.prepare(
            "SELECT year,delta_json,previous_hash,commit_hash,state_hash,events_hash,event_count FROM annual_commits WHERE world_id=?1 AND branch_id=?2 AND (?3 IS NULL OR year>?3) AND year<=?4 ORDER BY year",
        )?;
        let mut rows = stmt.query(params![world_id, branch_id, after_year, year])?;
        let mut expected_year = after_year
            .and_then(|y| y.checked_add(1))
            .unwrap_or(start_year);
        while let Some(row) = rows.next()? {
            let commit_year: i32 = row.get(0)?;
            let delta_json: String = row.get(1)?;
            let previous_hash: String = row.get(2)?;
            let commit_hash: String = row.get(3)?;
            let expected_state_hash: String = row.get(4)?;
            if previous_hash != chain_hash {
                return Err(StoreError::Integrity(format!(
                    "broken commit chain at year {commit_year}"
                )));
            }
            let patch: JsonPatch = serde_json::from_str(&delta_json)?;
            patch.apply(&mut core)?;
            let core_json = canonical_json(&core)?;
            if fnv_hex(core_json.as_bytes()) != expected_state_hash {
                return Err(StoreError::Integrity(format!(
                    "state checksum mismatch at year {commit_year}"
                )));
            }
            let events_hash: String = row.get(5)?;
            let expected_event_count: i64 = row.get(6)?;
            if commit_year != expected_year {
                return Err(StoreError::Integrity(format!(
                    "annual close sequence skips from {} to {commit_year}",
                    expected_year.saturating_sub(1)
                )));
            }
            let event_payloads = read_event_payloads(&conn, world_id, branch_id, commit_year)?;
            let actual_event_count = event_payloads.len() as u64;
            if actual_event_count != expected_event_count as u64 {
                return Err(StoreError::Integrity(format!(
                    "event count mismatch at year {commit_year}"
                )));
            }
            if events_hash_value(&event_payloads) != events_hash {
                return Err(StoreError::Integrity(format!(
                    "event payload checksum mismatch at year {commit_year}"
                )));
            }
            let computed = commit_hash_value(
                &previous_hash,
                commit_year,
                &expected_state_hash,
                &events_hash,
            );
            if computed != commit_hash {
                return Err(StoreError::Integrity(format!(
                    "event/commit checksum mismatch at year {commit_year}"
                )));
            }
            chain_hash = commit_hash;
            expected_year = commit_year.checked_add(1).ok_or_else(|| {
                StoreError::Integrity("year overflow while reading commits".into())
            })?;
        }
        let actual_year = value_year(&core)?;
        if actual_year != Some(year) {
            return Err(StoreError::YearUnavailable {
                branch_id: branch_id.to_owned(),
                requested: year,
            });
        }
        let full_json = canonical_json(&core)?;
        base_state = serde_json::from_str(&full_json)?;
        Ok(base_state)
    }

    pub fn head(&self, world_id: &str, branch_id: &str) -> StoreResult<Option<Year>> {
        get_branch(&open_ro(&self.path)?, world_id, branch_id)?
            .head_year
            .map(year_from_i32)
            .transpose()
    }

    pub fn scenario_json(&self, world_id: &str, branch_id: &str) -> StoreResult<String> {
        let conn = open_ro(&self.path)?;
        let branch = get_branch(&conn, world_id, branch_id)?;
        Ok(conn.query_row(
            "SELECT scenario_json FROM scenario_revisions WHERE revision_id=?1",
            [branch.revision_id],
            |r| r.get(0),
        )?)
    }

    /// Read semantic events involving an entity through the selected branch
    /// and year. Ancestors are restricted to the branch's stored cutoff.
    pub fn events_for_entity(
        &self,
        world_id: &str,
        branch_id: &str,
        year: Year,
        entity_type: &str,
        entity_id: &str,
    ) -> StoreResult<Vec<HistoricalEvent>> {
        let conn = open_ro(&self.path)?;
        let mut events = Vec::new();
        collect_entity_events(
            &conn,
            world_id,
            branch_id,
            year_value(year)?,
            entity_type,
            entity_id,
            &mut events,
        )?;
        events.sort_by_key(|event| (event.year.0, event.order));
        Ok(events)
    }

    pub fn list_worlds(&self) -> StoreResult<Vec<WorldSummary>> {
        let conn = open_ro(&self.path)?;
        let mut stmt = conn.prepare(
            "SELECT w.world_id,w.created_at,w.root_branch_id,(SELECT COUNT(*) FROM branches b WHERE b.world_id=w.world_id),(SELECT MAX(head_year) FROM branches b WHERE b.world_id=w.world_id) FROM worlds w ORDER BY w.created_at,w.world_id",
        )?;
        let mut rows = stmt.query([])?;
        let mut worlds = Vec::new();
        while let Some(row) = rows.next()? {
            worlds.push(WorldSummary {
                world_id: row.get(0)?,
                created_at: row.get(1)?,
                root_branch_id: row.get(2)?,
                branch_count: row.get(3)?,
                latest_year: row.get(4)?,
            });
        }
        Ok(worlds)
    }

    pub fn list_branches(&self, world_id: &str) -> StoreResult<Vec<BranchSummary>> {
        let conn = open_ro(&self.path)?;
        let mut stmt = conn.prepare("SELECT world_id,branch_id,parent_branch_id,fork_year,head_year,revision_id FROM branches WHERE world_id=?1 ORDER BY created_at,branch_id")?;
        let mut rows = stmt.query([world_id])?;
        let mut branches = Vec::new();
        while let Some(row) = rows.next()? {
            branches.push(BranchSummary {
                world_id: row.get(0)?,
                branch_id: row.get(1)?,
                parent_branch_id: row.get(2)?,
                fork_year: row.get(3)?,
                head_year: row.get(4)?,
                revision_id: row.get(5)?,
            });
        }
        Ok(branches)
    }

    /// Persist an explicit pause/recovery point at the current confirmed head.
    pub fn checkpoint(&self, world_id: &str, branch_id: &str) -> StoreResult<()> {
        let mut writer = self.acquire_writer(world_id, branch_id)?;
        writer.checkpoint()
    }

    /// Export the full world catalog coherently. Use `export_single_world`
    /// when the caller selected one world.
    pub fn export_world(&self, destination: impl AsRef<Path>) -> StoreResult<ExportManifest> {
        self.export_to(destination.as_ref(), None)
    }

    /// Export one world, including all of its branches and shared prefixes,
    /// without copying other worlds in the same catalog.
    pub fn export_single_world(
        &self,
        world_id: &str,
        destination: impl AsRef<Path>,
    ) -> StoreResult<ExportManifest> {
        self.export_to(destination.as_ref(), Some(world_id))
    }

    /// Copy a coherent SQLite snapshot and write a sidecar manifest. The
    /// backup API includes committed WAL content and omits uncommitted work.
    fn export_to(
        &self,
        destination: &Path,
        only_world: Option<&str>,
    ) -> StoreResult<ExportManifest> {
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        if destination.exists() || manifest_path(destination).exists() {
            return Err(StoreError::Invalid(format!(
                "export destination or manifest already exists: {}",
                destination.display()
            )));
        }
        if same_file_path(&self.path, destination) {
            return Err(StoreError::Invalid(
                "export destination must differ from the open database".into(),
            ));
        }
        let (temp_file, temp_path) = create_temp_sibling(destination, "export")?;
        drop(temp_file);
        let mut target = Connection::open(&temp_path)?;
        let backup_result = (|| -> StoreResult<()> {
            let source = open_ro(&self.path)?;
            let backup = Backup::new(&source, &mut target)?;
            backup.run_to_completion(128, Duration::from_millis(10), None)?;
            drop(backup);
            if let Some(world_id) = only_world {
                retain_only_world(&target, world_id)?;
            }
            Ok(())
        })();
        if let Err(error) = backup_result {
            drop(target);
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        if let Err(error) = target.execute_batch("PRAGMA journal_mode=DELETE;") {
            drop(target);
            let _ = fs::remove_file(&temp_path);
            return Err(StoreError::Sql(error));
        }
        if let Err(error) = verify_database(&target) {
            drop(target);
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        drop(target);
        let (byte_length, checksum) = match file_checksum(&temp_path) {
            Ok(checksum) => checksum,
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                return Err(error);
            }
        };
        if let Err(error) = fs::rename(&temp_path, destination) {
            let _ = fs::remove_file(&temp_path);
            return Err(StoreError::Io(error));
        }
        let exported = open_ro(destination)?;
        let world_ids = query_world_ids(&exported)?;
        let manifest = ExportManifest {
            format_version: 1,
            schema_version: CURRENT_SCHEMA_VERSION,
            world_ids,
            database_file: destination
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("worlds.sqlite3")
                .to_owned(),
            byte_length,
            checksum_fnv1a64: checksum,
        };
        if let Err(error) = write_manifest(destination, &manifest) {
            let _ = fs::remove_file(destination);
            return Err(error);
        }
        Ok(manifest)
    }

    /// Restore an exported database after checking its manifest, bytes, schema,
    /// SQLite integrity and foreign keys. The destination is installed atomically.
    pub fn restore_world(
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> StoreResult<Self> {
        let source = source.as_ref();
        let destination = destination.as_ref();
        let manifest = read_manifest(source)?;
        if manifest.format_version != 1 || manifest.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(StoreError::Invalid(format!(
                "unsupported export manifest versions: format {}, schema {}",
                manifest.format_version, manifest.schema_version
            )));
        }
        let (length, checksum) = file_checksum(source)?;
        if length != manifest.byte_length || checksum != manifest.checksum_fnv1a64 {
            return Err(StoreError::Integrity(
                "export file checksum does not match its manifest".into(),
            ));
        }
        let source_conn = open_ro(source)?;
        verify_database(&source_conn)?;
        let world_ids = query_world_ids(&source_conn)?;
        if world_ids != manifest.world_ids {
            return Err(StoreError::Integrity(
                "manifest world list does not match database".into(),
            ));
        }
        if destination.exists() {
            return Err(StoreError::Invalid(format!(
                "restore destination already exists: {}",
                destination.display()
            )));
        }
        if let Some(parent) = destination.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let (mut temporary_file, temp) = create_temp_sibling(destination, "restore")?;
        let copy_result = (|| -> StoreResult<()> {
            let mut source_file = File::open(source)?;
            std::io::copy(&mut source_file, &mut temporary_file)?;
            temporary_file.sync_all()?;
            Ok(())
        })();
        drop(temporary_file);
        if let Err(error) = copy_result {
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        if let Err(error) = fs::rename(&temp, destination) {
            let _ = fs::remove_file(&temp);
            return Err(StoreError::Io(error));
        }
        Self::open(destination)
    }
}

impl WorldWriter {
    pub fn world_id(&self) -> &str {
        &self.world_id
    }
    pub fn branch_id(&self) -> &str {
        &self.branch_id
    }

    /// Commit an annual engine result as one atomic batch. A failed commit
    /// leaves head, events, deltas and the recoverable state at the prior year.
    pub fn commit_year(&mut self, commit: &YearCommit) -> StoreResult<()> {
        #[cfg(test)]
        let fail_at = self.fail_at;
        let year = year_value(commit.year)?;
        validate_state_identity(&commit.state, &self.world_id)?;
        self.validated_scenario
            .validate_state(&commit.state)
            .map_err(|issues| {
                StoreError::Invalid(format!(
                    "invalid state at annual close: {}",
                    issues
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ))
            })?;
        if commit.state.current_year.map(year_value).transpose()? != Some(year) {
            return Err(StoreError::Invalid(
                "YearCommit state.current_year must equal YearCommit.year".into(),
            ));
        }
        let mut last_order = None;
        for event in &commit.events {
            if year_value(event.year)? != year {
                return Err(StoreError::Invalid(
                    "all events in a year commit must have the committed year".into(),
                ));
            }
            if last_order.is_some_and(|order| event.order <= order) {
                return Err(StoreError::Invalid(
                    "event order values must be strictly increasing".into(),
                ));
            }
            last_order = Some(event.order);
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let branch = get_branch(&tx, &self.world_id, &self.branch_id)?;
        if branch.head_year != self.cached_head_year || branch.head_hash != self.cached_head_hash {
            return Err(StoreError::Integrity(
                "writer's cached head no longer matches the durable branch head".into(),
            ));
        }
        let expected = match branch.head_year.or(branch.base_year) {
            Some(head) => head
                .checked_add(1)
                .ok_or_else(|| StoreError::Invalid("year overflow".into()))?,
            None => year_value(commit.state.start_year)?,
        };
        if year != expected {
            return Err(StoreError::Invalid(format!(
                "expected next year {expected}, received {year}"
            )));
        }
        let next_core = state_core_value(&commit.state)?;
        let previous_core = state_core_value(&self.cached_state)?;
        let patch = JsonPatch::between(&previous_core, &next_core);
        let delta_json = canonical_json(&serde_json::to_value(&patch)?)?;
        let next_core_json = canonical_json(&next_core)?;
        let state_hash = fnv_hex(next_core_json.as_bytes());
        let event_jsons = commit
            .events
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()?
            .iter()
            .map(canonical_json)
            .collect::<Result<Vec<_>, _>>()?;
        let events_hash = events_hash_value(&event_jsons);
        let commit_hash = commit_hash_value(&branch.head_hash, year, &state_hash, &events_hash);
        let revision_id = branch.revision_id;
        tx.execute(
            "INSERT INTO annual_commits(world_id,branch_id,year,delta_json,previous_hash,commit_hash,state_hash,events_hash,revision_id,event_count) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![self.world_id, self.branch_id, year, delta_json, branch.head_hash, commit_hash, state_hash, events_hash, revision_id, commit.events.len() as u64],
        )?;
        #[cfg(test)]
        injected_failure(fail_at, CommitFailurePoint::AfterCommitRow)?;
        for (ordinal, (event, payload)) in commit.events.iter().zip(event_jsons.iter()).enumerate()
        {
            let event_value: Value = serde_json::from_str(payload)?;
            let event_id = event_id_string(event)?;
            let event_kind = format!("{:?}", event.kind);
            tx.execute(
                "INSERT INTO events(world_id,branch_id,year,ordinal,event_id,event_kind,payload_json) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![self.world_id, self.branch_id, year, ordinal as u32, event_id, event_kind, payload],
            )?;
            index_event_entities(
                &tx,
                &self.world_id,
                &self.branch_id,
                year,
                ordinal as u32,
                &event_value,
            )?;
            #[cfg(test)]
            injected_failure(fail_at, CommitFailurePoint::AfterEventInsert)?;
        }
        if self.snapshot_interval > 0 && year.rem_euclid(self.snapshot_interval) == 0 {
            insert_snapshot(
                &tx,
                &self.world_id,
                &self.branch_id,
                year,
                &next_core_json,
                &state_hash,
                &commit_hash,
            )?;
        }
        #[cfg(test)]
        injected_failure(fail_at, CommitFailurePoint::BeforeHeadUpdate)?;
        tx.execute(
            "UPDATE branches SET head_year=?3,head_hash=?4 WHERE world_id=?1 AND branch_id=?2",
            params![self.world_id, self.branch_id, year, commit_hash],
        )?;
        tx.commit()?;
        self.cached_state = commit.state.clone();
        self.cached_head_year = Some(year);
        self.cached_head_hash = commit_hash;
        Ok(())
    }

    pub fn checkpoint(&mut self) -> StoreResult<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let branch = get_branch(&tx, &self.world_id, &self.branch_id)?;
        let year = branch.head_year.ok_or_else(|| {
            StoreError::Invalid("cannot checkpoint a branch before its first close".into())
        })?;
        if branch.head_year != self.cached_head_year || branch.head_hash != self.cached_head_hash {
            return Err(StoreError::Integrity(
                "writer's cached head no longer matches the durable branch head".into(),
            ));
        }
        let core = state_core_value(&self.cached_state)?;
        let json = canonical_json(&core)?;
        insert_snapshot(
            &tx,
            &self.world_id,
            &self.branch_id,
            year,
            &json,
            &fnv_hex(json.as_bytes()),
            &branch.head_hash,
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
fn injected_failure(
    fail_at: Option<CommitFailurePoint>,
    point: CommitFailurePoint,
) -> StoreResult<()> {
    if fail_at == Some(point) {
        return Err(StoreError::Invalid(format!(
            "injected test failure at {point:?}"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
enum JsonPatch {
    Set(Value),
    Remove,
    Merge(BTreeMap<String, JsonPatch>),
}

impl JsonPatch {
    fn between(old: &Value, new: &Value) -> Self {
        match (old, new) {
            (Value::Object(old), Value::Object(new)) => {
                let mut changes = BTreeMap::new();
                for key in old.keys() {
                    if !new.contains_key(key) {
                        changes.insert(key.clone(), Self::Remove);
                    }
                }
                for (key, value) in new {
                    match old.get(key) {
                        Some(old_value) if old_value == value => {}
                        Some(old_value) => {
                            changes.insert(key.clone(), Self::between(old_value, value));
                        }
                        None => {
                            changes.insert(key.clone(), Self::Set(value.clone()));
                        }
                    }
                }
                Self::Merge(changes)
            }
            _ if old == new => Self::Merge(BTreeMap::new()),
            _ => Self::Set(new.clone()),
        }
    }

    fn apply(self, target: &mut Value) -> StoreResult<()> {
        match self {
            Self::Set(value) => *target = value,
            Self::Remove => {
                return Err(StoreError::Integrity(
                    "remove operation cannot replace the state root".into(),
                ))
            }
            Self::Merge(changes) => {
                let object = target
                    .as_object_mut()
                    .ok_or_else(|| StoreError::Integrity("delta expected state object".into()))?;
                for (key, change) in changes {
                    match change {
                        Self::Remove => {
                            object.remove(&key);
                        }
                        Self::Set(value) => {
                            object.insert(key, value);
                        }
                        nested => {
                            let value = object.get_mut(&key).ok_or_else(|| {
                                StoreError::Integrity(format!("delta missing base key '{key}'"))
                            })?;
                            nested.apply(value)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct BranchRecord {
    parent_branch_id: Option<String>,
    fork_year: Option<i32>,
    base_year: Option<i32>,
    base_state_json: String,
    base_hash: String,
    base_chain_hash: String,
    head_year: Option<i32>,
    head_hash: String,
    revision_id: i64,
}

fn get_branch(conn: &Connection, world_id: &str, branch_id: &str) -> StoreResult<BranchRecord> {
    conn.query_row(
        "SELECT parent_branch_id,fork_year,base_year,base_state_json,base_state_hash,base_chain_hash,head_year,head_hash,revision_id FROM branches WHERE world_id=?1 AND branch_id=?2",
        params![world_id, branch_id],
        |row| Ok(BranchRecord { parent_branch_id: row.get(0)?, fork_year: row.get(1)?, base_year: row.get(2)?, base_state_json: row.get(3)?, base_hash: row.get(4)?, base_chain_hash: row.get(5)?, head_year: row.get(6)?, head_hash: row.get(7)?, revision_id: row.get(8)? }),
    ).optional()?.ok_or_else(|| StoreError::BranchNotFound { world_id: world_id.to_owned(), branch_id: branch_id.to_owned() })
}

fn effective_hash_at(
    conn: &Connection,
    world_id: &str,
    branch_id: &str,
    year: i32,
) -> StoreResult<String> {
    let branch = get_branch(conn, world_id, branch_id)?;
    if let Some(base_year) = branch.base_year {
        if year <= base_year {
            if let Some(parent) = branch.parent_branch_id.as_deref() {
                return effective_hash_at(conn, world_id, parent, year);
            }
            if year == base_year {
                return Ok(branch.base_chain_hash);
            }
        }
    }
    if branch.head_year.is_none_or(|head| year > head) {
        return Err(StoreError::YearUnavailable {
            branch_id: branch_id.to_owned(),
            requested: year,
        });
    }
    conn.query_row(
        "SELECT commit_hash FROM annual_commits WHERE world_id=?1 AND branch_id=?2 AND year=?3",
        params![world_id, branch_id, year],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(|| StoreError::YearUnavailable {
        branch_id: branch_id.to_owned(),
        requested: year,
    })
}

fn parse_and_validate_scenario(scenario_json: &str, state: &State) -> StoreResult<Scenario> {
    let scenario = parse_validated_scenario(scenario_json)?;
    scenario.validate_state(state).map_err(|issues| {
        StoreError::ScenarioInvalid(format!(
            "scenario is incompatible with the saved prefix: {}",
            issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        ))
    })?;
    Ok(scenario.into_scenario())
}

fn parse_validated_scenario(scenario_json: &str) -> StoreResult<ValidatedScenario> {
    let scenario: Scenario = serde_json::from_str(scenario_json)
        .map_err(|error| StoreError::ScenarioInvalid(error.to_string()))?;
    scenario.validate().map_err(|issues| {
        StoreError::ScenarioInvalid(
            issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        )
    })
}

fn ensure_only_policies_changed(original: &Scenario, revised: &Scenario) -> StoreResult<()> {
    let mut comparable_revision = revised.clone();
    comparable_revision.policies = original.policies.clone();
    if comparable_revision != *original {
        return Err(StoreError::ScenarioInvalid(
            "a branch with a committed prefix may change policies only; initial entities, identities, catalogs, genetics, profiles and start metadata must remain fixed".into(),
        ));
    }
    Ok(())
}

fn read_event_payloads(
    conn: &Connection,
    world_id: &str,
    branch_id: &str,
    year: i32,
) -> StoreResult<Vec<String>> {
    let mut statement = conn.prepare(
        "SELECT ordinal,payload_json FROM events WHERE world_id=?1 AND branch_id=?2 AND year=?3 ORDER BY ordinal",
    )?;
    let rows = statement.query_map(params![world_id, branch_id, year], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut payloads = Vec::new();
    for (expected_ordinal, row) in rows.enumerate() {
        let (ordinal, payload) = row?;
        if ordinal != expected_ordinal as i64 {
            return Err(StoreError::Integrity(format!(
                "event sequence has a missing or reordered ordinal at year {year}"
            )));
        }
        payloads.push(payload);
    }
    Ok(payloads)
}

fn collect_entity_events(
    conn: &Connection,
    world_id: &str,
    branch_id: &str,
    year: i32,
    entity_type: &str,
    entity_id: &str,
    output: &mut Vec<HistoricalEvent>,
) -> StoreResult<()> {
    let branch = get_branch(conn, world_id, branch_id)?;
    if branch.base_year.is_some_and(|base| year <= base) {
        if let Some(parent) = branch.parent_branch_id.as_deref() {
            return collect_entity_events(
                conn,
                world_id,
                parent,
                year,
                entity_type,
                entity_id,
                output,
            );
        }
    }
    if branch.head_year.is_none_or(|head| year > head) {
        return Err(StoreError::YearUnavailable {
            branch_id: branch_id.to_owned(),
            requested: year,
        });
    }
    let lower = branch.base_year;
    let mut statement = conn.prepare(
        "SELECT e.payload_json FROM event_entities ref JOIN events e USING(world_id,branch_id,year,ordinal) WHERE ref.world_id=?1 AND ref.branch_id=?2 AND ref.entity_type=?3 AND ref.entity_id=?4 AND (?5 IS NULL OR ref.year>?5) AND ref.year<=?6 ORDER BY ref.year,ref.ordinal",
    )?;
    let rows = statement.query_map(
        params![world_id, branch_id, entity_type, entity_id, lower, year],
        |row| row.get::<_, String>(0),
    )?;
    for row in rows {
        let event: HistoricalEvent = serde_json::from_str(&row?)?;
        if !event_references_entity(&event, entity_type, entity_id)? {
            return Err(StoreError::Integrity(
                "historical event entity index does not match its payload".into(),
            ));
        }
        output.push(event);
    }
    if let (Some(parent), Some(cutoff)) = (branch.parent_branch_id.as_deref(), branch.fork_year) {
        if year > cutoff {
            collect_entity_events(
                conn,
                world_id,
                parent,
                cutoff,
                entity_type,
                entity_id,
                output,
            )?;
        }
    }
    Ok(())
}

fn event_references_entity(
    event: &HistoricalEvent,
    entity_type: &str,
    entity_id: &str,
) -> StoreResult<bool> {
    for entity in &event.affected_entities {
        let (kind, id) = entity_ref_parts(&serde_json::to_value(entity)?);
        if kind.as_deref() == Some(entity_type) && id.as_deref() == Some(entity_id) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn insert_revision(
    tx: &Transaction<'_>,
    world_id: &str,
    branch_id: &str,
    effective_after: Option<i32>,
    json: &str,
) -> StoreResult<i64> {
    tx.execute("INSERT INTO scenario_revisions(world_id,branch_id,effective_after_year,scenario_json,scenario_hash,created_at) VALUES(?1,?2,?3,?4,?5,?6)", params![world_id, branch_id, effective_after, json, fnv_hex(json.as_bytes()), utc_timestamp()])?;
    Ok(tx.last_insert_rowid())
}

fn retain_only_world(conn: &Connection, world_id: &str) -> StoreResult<()> {
    let exists = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM worlds WHERE world_id=?1)",
        [world_id],
        |row| row.get::<_, bool>(0),
    )?;
    if !exists {
        return Err(StoreError::WorldNotFound(world_id.to_owned()));
    }
    // This runs only on a temporary backup. Disable FK enforcement while
    // removing entire unrelated world subgraphs, then check it after restore.
    conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "DROP TRIGGER IF EXISTS annual_commits_immutable_delete;
         DROP TRIGGER IF EXISTS events_immutable_delete;
         DROP TRIGGER IF EXISTS event_entities_immutable_delete;",
    )?;
    tx.execute("DELETE FROM event_entities WHERE world_id<>?1", [world_id])?;
    tx.execute("DELETE FROM events WHERE world_id<>?1", [world_id])?;
    tx.execute("DELETE FROM snapshots WHERE world_id<>?1", [world_id])?;
    tx.execute("DELETE FROM annual_commits WHERE world_id<>?1", [world_id])?;
    tx.execute("DELETE FROM branches WHERE world_id<>?1", [world_id])?;
    tx.execute(
        "DELETE FROM scenario_revisions WHERE world_id<>?1",
        [world_id],
    )?;
    tx.execute("DELETE FROM worlds WHERE world_id<>?1", [world_id])?;
    tx.execute_batch(
        "CREATE TRIGGER annual_commits_immutable_delete BEFORE DELETE ON annual_commits BEGIN SELECT RAISE(ABORT, 'annual commits are immutable'); END;
         CREATE TRIGGER events_immutable_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT, 'events are immutable'); END;
         CREATE TRIGGER event_entities_immutable_delete BEFORE DELETE ON event_entities BEGIN SELECT RAISE(ABORT, 'event entity indexes are immutable'); END;",
    )?;
    tx.commit()?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    Ok(())
}

fn state_core_value(state: &State) -> StoreResult<Value> {
    Ok(strip_events(serde_json::to_value(state)?))
}

fn strip_events(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("events");
    }
    value
}

fn validate_state_identity(state: &State, world_id: &str) -> StoreResult<()> {
    if json_id(&state.world_id)? != world_id {
        return Err(StoreError::Invalid(
            "state world_id does not match target world".into(),
        ));
    }
    Ok(())
}

fn same_file_path(left: &Path, right: &Path) -> bool {
    let left = fs::canonicalize(left).ok();
    let right = if right.exists() {
        fs::canonicalize(right).ok()
    } else {
        right
            .parent()
            .and_then(|parent| fs::canonicalize(parent).ok())
            .map(|parent| parent.join(right.file_name().unwrap_or_default()))
    };
    left.is_some() && left == right
}

fn create_temp_sibling(path: &Path, purpose: &str) -> StoreResult<(File, PathBuf)> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".{}.{}.{purpose}.tmp", std::process::id(), nonce));
    let temp_path = PathBuf::from(name);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&temp_path)?;
    Ok((file, temp_path))
}

fn json_id<T: Serialize>(value: &T) -> StoreResult<String> {
    let value = serde_json::to_value(value)?;
    Ok(match value {
        Value::String(s) => s,
        other => canonical_json(&other)?,
    })
}

fn year_value(year: Year) -> StoreResult<i32> {
    let value = serde_json::to_value(year)?;
    value
        .as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| StoreError::Invalid("Year must serialize as an i32 number".into()))
}

fn year_from_i32(value: i32) -> StoreResult<Year> {
    serde_json::from_value(Value::Number(value.into())).map_err(StoreError::Json)
}

fn value_year(value: &Value) -> StoreResult<Option<i32>> {
    let year = value.get("current_year").cloned().unwrap_or(Value::Null);
    if year.is_null() {
        return Ok(None);
    }
    year.as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .map(Some)
        .ok_or_else(|| StoreError::Integrity("current_year is not a valid i32".into()))
}

fn event_id_string(event: &HistoricalEvent) -> StoreResult<String> {
    json_id(&event.id)
}

fn open_rw(path: &Path) -> StoreResult<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA temp_store=MEMORY;")?;
    Ok(conn)
}

fn open_ro(path: &Path) -> StoreResult<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA query_only=ON;")?;
    Ok(conn)
}

fn lock_path(db_path: &Path, world_id: &str) -> PathBuf {
    let mut name: OsString = db_path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(
        ".world-{}.writer.lock",
        fnv_hex(world_id.as_bytes())
    ));
    db_path.with_file_name(name)
}

fn insert_snapshot(
    tx: &Transaction<'_>,
    world_id: &str,
    branch_id: &str,
    year: i32,
    state_json: &str,
    state_hash: &str,
    head_hash: &str,
) -> StoreResult<()> {
    tx.execute("INSERT INTO snapshots(world_id,branch_id,year,state_json,state_hash,head_hash,format_version) VALUES(?1,?2,?3,?4,?5,?6,1) ON CONFLICT(world_id,branch_id,year) DO UPDATE SET state_json=excluded.state_json,state_hash=excluded.state_hash,head_hash=excluded.head_hash,format_version=excluded.format_version", params![world_id, branch_id, year, state_json, state_hash, head_hash])?;
    Ok(())
}

fn index_event_entities(
    tx: &Transaction<'_>,
    world_id: &str,
    branch_id: &str,
    year: i32,
    ordinal: u32,
    event: &Value,
) -> StoreResult<()> {
    let Some(refs) = event.get("affected_entities").and_then(Value::as_array) else {
        return Ok(());
    };
    for reference in refs {
        let (entity_type, entity_id) = entity_ref_parts(reference);
        if let (Some(entity_type), Some(entity_id)) = (entity_type, entity_id) {
            tx.execute("INSERT OR IGNORE INTO event_entities(world_id,branch_id,year,ordinal,entity_type,entity_id) VALUES(?1,?2,?3,?4,?5,?6)", params![world_id, branch_id, year, ordinal, entity_type, entity_id])?;
        }
    }
    Ok(())
}

fn entity_ref_parts(value: &Value) -> (Option<String>, Option<String>) {
    match value {
        Value::Object(object) if object.len() == 1 => {
            let (kind, id) = object.iter().next().expect("length checked");
            (
                Some(kind.to_ascii_lowercase()),
                Some(match id {
                    Value::String(s) => s.clone(),
                    _ => id.to_string(),
                }),
            )
        }
        Value::Object(object) => {
            let kind = object
                .get("kind")
                .or_else(|| object.get("entity_type"))
                .and_then(Value::as_str)
                .map(str::to_ascii_lowercase);
            let id = object
                .get("id")
                .or_else(|| object.get("entity_id"))
                .map(|id| match id {
                    Value::String(s) => s.clone(),
                    _ => id.to_string(),
                });
            (kind, id)
        }
        _ => (None, None),
    }
}

fn canonical_json(value: &Value) -> Result<String, serde_json::Error> {
    serde_json::to_string(value)
}

fn fnv_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn events_hash_value(events: &[String]) -> String {
    let mut bytes = Vec::new();
    for event in events {
        bytes.extend_from_slice(&(event.len() as u64).to_le_bytes());
        bytes.extend_from_slice(event.as_bytes());
    }
    fnv_hex(&bytes)
}

fn commit_hash_value(
    previous_hash: &str,
    year: i32,
    state_hash: &str,
    events_hash: &str,
) -> String {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(previous_hash.as_bytes());
    bytes.extend_from_slice(&year.to_le_bytes());
    bytes.extend_from_slice(state_hash.as_bytes());
    bytes.extend_from_slice(events_hash.as_bytes());
    fnv_hex(&bytes)
}

fn utc_timestamp() -> String {
    // UTC timestamp for ordering only. Simulated decisions never depend on it.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos())
}

fn verify_database(conn: &Connection) -> StoreResult<()> {
    let check: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if check != "ok" {
        return Err(StoreError::Integrity(format!(
            "SQLite quick_check: {check}"
        )));
    }
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let mut rows = stmt.query([])?;
    if rows.next()?.is_some() {
        return Err(StoreError::Integrity(
            "SQLite foreign_key_check reported a violation".into(),
        ));
    }
    let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(StoreError::NewerSchema {
            found: version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    if version != CURRENT_SCHEMA_VERSION {
        return Err(StoreError::Integrity(format!(
            "unsupported database schema version {version}; expected {CURRENT_SCHEMA_VERSION}"
        )));
    }
    Ok(())
}

fn query_world_ids(conn: &Connection) -> StoreResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT world_id FROM worlds ORDER BY world_id")?;
    let world_ids = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(world_ids)
}

fn file_checksum(path: &Path) -> StoreResult<(u64, String)> {
    let mut file = File::open(path)?;
    let mut buffer = [0u8; 64 * 1024];
    let mut length = 0u64;
    let mut hash: u64 = 0xcbf29ce484222325;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        length += read as u64;
        for byte in &buffer[..read] {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    Ok((length, format!("{hash:016x}")))
}

fn manifest_path(database_path: &Path) -> PathBuf {
    database_path.with_extension("manifest.json")
}

fn write_manifest(database_path: &Path, manifest: &ExportManifest) -> StoreResult<()> {
    let path = manifest_path(database_path);
    let bytes = serde_json::to_vec_pretty(manifest)?;
    let (mut file, temp_path) = create_temp_sibling(&path, "manifest")?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp_path, path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        StoreError::Io(error)
    })?;
    Ok(())
}

fn read_manifest(database_path: &Path) -> StoreResult<ExportManifest> {
    let path = manifest_path(database_path);
    if !path.exists() {
        return Err(StoreError::Integrity(format!(
            "missing export manifest: {}",
            path.display()
        )));
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

#[cfg(test)]
mod tests;
