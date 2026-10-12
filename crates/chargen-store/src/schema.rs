use rusqlite::Connection;

use crate::{StoreError, StoreResult};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

pub fn migrate(conn: &Connection) -> StoreResult<()> {
    let found: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if found > CURRENT_SCHEMA_VERSION {
        return Err(StoreError::NewerSchema {
            found,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    if found == CURRENT_SCHEMA_VERSION {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "CREATE TABLE schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );
        CREATE TABLE worlds (
            world_id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            format_version INTEGER NOT NULL,
            schema_version INTEGER NOT NULL,
            rules_version INTEGER NOT NULL,
            seed TEXT NOT NULL,
            root_branch_id TEXT NOT NULL
        );
        CREATE TABLE scenario_revisions (
            revision_id INTEGER PRIMARY KEY AUTOINCREMENT,
            world_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            effective_after_year INTEGER,
            scenario_json TEXT NOT NULL,
            scenario_hash TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (world_id) REFERENCES worlds(world_id) ON DELETE RESTRICT
        );
        CREATE INDEX scenario_revision_branch ON scenario_revisions(world_id, branch_id, revision_id);
        CREATE TABLE branches (
            world_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            parent_branch_id TEXT,
            fork_year INTEGER,
            base_year INTEGER,
            base_state_json TEXT NOT NULL,
            base_state_hash TEXT NOT NULL,
            base_chain_hash TEXT NOT NULL,
            head_year INTEGER,
            head_hash TEXT NOT NULL,
            revision_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (world_id, branch_id),
            FOREIGN KEY (world_id) REFERENCES worlds(world_id) ON DELETE RESTRICT,
            FOREIGN KEY (world_id, parent_branch_id) REFERENCES branches(world_id, branch_id) ON DELETE RESTRICT,
            FOREIGN KEY (revision_id) REFERENCES scenario_revisions(revision_id) ON DELETE RESTRICT,
            CHECK ((parent_branch_id IS NULL AND fork_year IS NULL AND base_year IS NULL) OR
                   (parent_branch_id IS NOT NULL AND fork_year IS NULL AND base_year IS NULL) OR
                   (parent_branch_id IS NOT NULL AND fork_year IS NOT NULL AND base_year = fork_year))
        );
        CREATE INDEX branches_by_parent ON branches(world_id, parent_branch_id, fork_year);
        CREATE TABLE annual_commits (
            world_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            year INTEGER NOT NULL,
            delta_json TEXT NOT NULL,
            previous_hash TEXT NOT NULL,
            commit_hash TEXT NOT NULL,
            state_hash TEXT NOT NULL,
            events_hash TEXT NOT NULL,
            revision_id INTEGER NOT NULL,
            event_count INTEGER NOT NULL,
            PRIMARY KEY (world_id, branch_id, year),
            FOREIGN KEY (world_id, branch_id) REFERENCES branches(world_id, branch_id) ON DELETE RESTRICT,
            FOREIGN KEY (revision_id) REFERENCES scenario_revisions(revision_id) ON DELETE RESTRICT
        );
        CREATE INDEX annual_commits_by_year ON annual_commits(world_id, branch_id, year);
        CREATE TABLE events (
            world_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            year INTEGER NOT NULL,
            ordinal INTEGER NOT NULL,
            event_id TEXT NOT NULL,
            event_kind TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            PRIMARY KEY (world_id, branch_id, year, ordinal),
            UNIQUE (world_id, branch_id, event_id),
            FOREIGN KEY (world_id, branch_id, year) REFERENCES annual_commits(world_id, branch_id, year) ON DELETE RESTRICT
        );
        CREATE INDEX events_by_kind_year ON events(world_id, branch_id, event_kind, year, ordinal);
        CREATE TABLE event_entities (
            world_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            year INTEGER NOT NULL,
            ordinal INTEGER NOT NULL,
            entity_type TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            PRIMARY KEY (world_id, branch_id, year, ordinal, entity_type, entity_id),
            FOREIGN KEY (world_id, branch_id, year, ordinal) REFERENCES events(world_id, branch_id, year, ordinal) ON DELETE RESTRICT
        );
        CREATE INDEX event_entities_by_entity_year ON event_entities(world_id, branch_id, entity_type, entity_id, year, ordinal);
        CREATE TABLE snapshots (
            world_id TEXT NOT NULL,
            branch_id TEXT NOT NULL,
            year INTEGER NOT NULL,
            state_json TEXT NOT NULL,
            state_hash TEXT NOT NULL,
            head_hash TEXT NOT NULL,
            format_version INTEGER NOT NULL,
            PRIMARY KEY (world_id, branch_id, year),
            FOREIGN KEY (world_id, branch_id, year) REFERENCES annual_commits(world_id, branch_id, year) ON DELETE RESTRICT
        );
        CREATE INDEX snapshots_nearest ON snapshots(world_id, branch_id, year DESC);
        CREATE TRIGGER annual_commits_immutable_update BEFORE UPDATE ON annual_commits BEGIN SELECT RAISE(ABORT, 'annual commits are immutable'); END;
        CREATE TRIGGER annual_commits_immutable_delete BEFORE DELETE ON annual_commits BEGIN SELECT RAISE(ABORT, 'annual commits are immutable'); END;
        CREATE TRIGGER events_immutable_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT, 'events are immutable'); END;
        CREATE TRIGGER events_immutable_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT, 'events are immutable'); END;
        CREATE TRIGGER event_entities_immutable_update BEFORE UPDATE ON event_entities BEGIN SELECT RAISE(ABORT, 'event entity indexes are immutable'); END;
        CREATE TRIGGER event_entities_immutable_delete BEFORE DELETE ON event_entities BEGIN SELECT RAISE(ABORT, 'event entity indexes are immutable'); END;
        INSERT INTO schema_migrations(version,applied_at) VALUES(1,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
        PRAGMA user_version=1;"
    )?;
    tx.commit()?;
    Ok(())
}
