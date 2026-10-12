use chargen_domain::{Scenario, Year};
use chargen_engine::Engine;
use chargen_store::HistoryStore;
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(1);

struct TempDb(PathBuf);

impl TempDb {
    fn from_e2_dump() -> Self {
        let serial = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "chargen-e2-fixture-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("e2.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        connection
            .execute_batch(include_str!("fixtures/e2-b0.sql"))
            .unwrap();
        drop(connection);
        Self(path)
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        if let Some(parent) = self.0.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}

/// This fixture is an SQL dump of the E2 B0 database produced by commit
/// 5b7039a. Its stored commit hashes and event/state JSON predate the E3 fields.
#[test]
fn opens_replays_and_resumes_a_real_e2_database_without_rewriting_history() {
    const WORLD_ID: &str = "421440";
    const BRANCH_ID: &str = "main";
    let temp = TempDb::from_e2_dump();
    let before = fs::read(&temp.0).unwrap();
    let store = HistoryStore::open_existing(&temp.0).unwrap();

    let worlds = store.list_worlds().unwrap();
    assert_eq!(worlds.len(), 1);
    assert_eq!(worlds[0].world_id, WORLD_ID);
    let branches = store.list_branches(WORLD_ID).unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].branch_id, BRANCH_ID);
    assert_eq!(branches[0].head_year, Some(9));

    let scenario_json = store.scenario_json(WORLD_ID, BRANCH_ID).unwrap();
    assert!(!scenario_json.contains("\"economy\""));
    assert!(!scenario_json.contains("\"scheduled_events\""));
    let scenario: Scenario = serde_json::from_str(&scenario_json).unwrap();
    assert_eq!(scenario.rules_version, 2);
    let validated = scenario.validate().unwrap();
    let mut replay = Engine::from_scenario(validated.clone()).unwrap();

    let expected_state_hashes = [
        "a7e9d7bf9b9993e6",
        "1f856916aa3333ac",
        "3e87a041ed2322f6",
        "caa11d5bc8c55ac0",
        "04876da30ee30999",
        "a1da6b051737c201",
        "dd3ae73fa17f891e",
        "40c760bb6bfeee02",
        "00ee5fb1ae78a79e",
        "76731ceb5290bfdf",
    ];
    let expected_event_hashes = [
        "79e6ce3f9433072d",
        "9223c0ce11034783",
        "cbf29ce484222325",
        "a9e261e90fe66055",
        "d2db8c1ea811dcc9",
        "d45acdac0f6f89c4",
        "dbb1ceb99aac7b6b",
        "cbf29ce484222325",
        "c56dfb0507117708",
        "cbf29ce484222325",
    ];
    let expected_commit_hashes = [
        "a11a6dddd89ef982",
        "c00881b505638d23",
        "6951551028bae932",
        "7a398ce09a2ce2bf",
        "e90ef1319057609f",
        "4bd8cdb7de709a25",
        "a5f80a9931aeb498",
        "14ecd288d3c36ae1",
        "47a4f0df762a2bb9",
        "4764ae8bc9b74aff",
    ];
    let connection = Connection::open(&temp.0).unwrap();
    for year in 0..=9 {
        let stored_hashes: (String, String, String) = connection
            .query_row(
                "SELECT state_hash,events_hash,commit_hash FROM annual_commits WHERE world_id=?1 AND branch_id=?2 AND year=?3",
                (WORLD_ID, BRANCH_ID, year),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(stored_hashes.0, expected_state_hashes[year as usize]);
        assert_eq!(stored_hashes.1, expected_event_hashes[year as usize]);
        assert_eq!(stored_hashes.2, expected_commit_hashes[year as usize]);

        let state = store
            .load_state_at(WORLD_ID, BRANCH_ID, Year(year))
            .unwrap();
        assert_eq!(state.current_year, Some(Year(year)));
        let serialized_state = serde_json::to_value(&state).unwrap();
        assert!(serialized_state.get("economy").is_none());
        assert!(serialized_state.get("event_effects").is_none());

        let replayed = replay.advance_year(None).unwrap();
        assert_eq!(replayed.year, Year(year));
        assert_eq!(replayed.state, state, "E2 replay state at year {year}");
        let replayed_events = replayed
            .events
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let stored_events = {
            let mut statement = connection
                .prepare(
                    "SELECT payload_json FROM events WHERE world_id=?1 AND branch_id=?2 AND year=?3 ORDER BY ordinal",
                )
                .unwrap();
            let rows = statement
                .query_map((WORLD_ID, BRANCH_ID, year), |row| row.get::<_, String>(0))
                .unwrap();
            let mut payloads = Vec::new();
            for row in rows {
                payloads.push(serde_json::from_str::<serde_json::Value>(&row.unwrap()).unwrap());
            }
            payloads
        };
        assert_eq!(
            replayed_events, stored_events,
            "E2 replay events at year {year}"
        );
    }
    drop(connection);

    let all_world_events = store
        .events_for_entity(WORLD_ID, BRANCH_ID, Year(9), "world", WORLD_ID)
        .unwrap();
    let mut paged_world_events = Vec::new();
    for offset in (0..all_world_events.len()).step_by(4) {
        let page = store
            .events_for_entity_page(WORLD_ID, BRANCH_ID, Year(9), "world", WORLD_ID, offset, 4)
            .unwrap();
        assert_eq!(page.total, all_world_events.len());
        paged_world_events.extend(page.events);
    }
    assert_eq!(paged_world_events, all_world_events);

    let head_state = store.load_state(WORLD_ID, BRANCH_ID).unwrap();
    assert_eq!(head_state.current_year, Some(Year(9)));
    let mut resumed = Engine::new(validated, head_state).unwrap();
    let next = resumed.advance_year(None).unwrap();
    assert_eq!(next.year, Year(10));
    assert_eq!(next.state.living_people_at(Year(10)).count(), 40);

    // Loading, replaying and exporting preserve the original E2 database.
    assert_eq!(fs::read(&temp.0).unwrap(), before);
    let export_path = temp.0.with_file_name("e2-export.sqlite3");
    let export = store.export_single_world(WORLD_ID, &export_path).unwrap();
    assert_eq!(export.world_ids, vec![WORLD_ID]);
    let exported = HistoryStore::open_existing(&export_path).unwrap();
    assert_eq!(exported.head(WORLD_ID, BRANCH_ID).unwrap(), Some(Year(9)));
    assert_eq!(
        exported
            .load_state_at(WORLD_ID, BRANCH_ID, Year(9))
            .unwrap()
            .current_year,
        Some(Year(9))
    );
}
