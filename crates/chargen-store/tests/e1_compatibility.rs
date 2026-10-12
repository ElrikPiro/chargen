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
    fn from_e1_dump() -> Self {
        let serial = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "chargen-e1-fixture-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("e1.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(include_str!("fixtures/e1-b0-final.sql"))
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

/// This SQL fixture was dumped from the E1 B0 database produced before E2's
/// territory/culture fields existed. Stored hashes are fixed here so the test
/// cannot silently replace it with a database serialized by the new code.
#[test]
fn opens_queries_and_resumes_a_real_e1_database_without_rewriting_history() {
    let temp = TempDb::from_e1_dump();
    let before = fs::read(&temp.0).unwrap();
    let store = HistoryStore::open_existing(&temp.0).unwrap();

    let worlds = store.list_worlds().unwrap();
    assert_eq!(worlds.len(), 1);
    assert_eq!(worlds[0].world_id, "421337");
    let branches = store.list_branches("421337").unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].branch_id, "main");
    assert_eq!(branches[0].head_year, Some(9));

    let scenario_json = store.scenario_json("421337", "main").unwrap();
    let scenario: Scenario = serde_json::from_str(&scenario_json).unwrap();
    assert_eq!(scenario.rules_version, 1);
    let validated = scenario.validate().unwrap();
    let mut replay = Engine::from_scenario(validated.clone()).unwrap();

    let expected_state_hashes = [
        "a28b944ebb24c594",
        "4e043dc45bc8af8f",
        "bad76ff51eb0bd4e",
        "cfabc43e4a95c633",
        "04887abc8bd53d33",
        "40add6c2b0284eb4",
        "bd9efca0e2ddc7e5",
        "54e64496004c8ee1",
        "b4f086fcd541ea4a",
        "9a1a9ee835aee05b",
    ];
    let connection = Connection::open(&temp.0).unwrap();
    for (year, expected_hash) in expected_state_hashes.iter().enumerate() {
        let stored_hash: String = connection
            .query_row(
                "SELECT state_hash FROM annual_commits WHERE world_id='421337' AND branch_id='main' AND year=?1",
                [year as i32],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            &stored_hash, expected_hash,
            "legacy stored hash at year {year}"
        );
        let state = store
            .load_state_at("421337", "main", Year(year as i32))
            .unwrap();
        assert_eq!(state.current_year, Some(Year(year as i32)));
        assert_eq!(state.living_people_at(Year(year as i32)).count(), 40);

        let replayed = replay.advance_year(None).unwrap();
        assert_eq!(replayed.year, Year(year as i32));
        assert_eq!(replayed.state, state, "legacy replay state at year {year}");
        let replayed_events = replayed
            .events
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let stored_events = {
            let mut statement = connection
                .prepare(
                    "SELECT payload_json FROM events WHERE world_id='421337' AND branch_id='main' AND year=?1 ORDER BY ordinal",
                )
                .unwrap();
            let rows = statement
                .query_map([year as i32], |row| row.get::<_, String>(0))
                .unwrap();
            let mut payloads = Vec::new();
            for row in rows {
                payloads.push(serde_json::from_str::<serde_json::Value>(&row.unwrap()).unwrap());
            }
            payloads
        };
        assert_eq!(
            replayed_events, stored_events,
            "legacy replay events at year {year}"
        );
    }
    drop(connection);

    let head_state = store.load_state("421337", "main").unwrap();
    assert_eq!(head_state.current_year, Some(Year(9)));
    let mut engine = Engine::new(validated, head_state).unwrap();
    let next = engine.advance_year(None).unwrap();
    assert_eq!(next.year, Year(10));
    assert_eq!(next.state.living_people_at(Year(10)).count(), 40);

    // `open_existing` and reconstruction are read-only. The parent history is
    // byte-for-byte unchanged and remains exportable as its original world.
    assert_eq!(fs::read(&temp.0).unwrap(), before);
    let export_path = temp.0.with_file_name("e1-export.sqlite3");
    let export = store.export_single_world("421337", &export_path).unwrap();
    assert_eq!(export.world_ids, vec!["421337"]);
    let exported = HistoryStore::open_existing(&export_path).unwrap();
    assert_eq!(exported.head("421337", "main").unwrap(), Some(Year(9)));
    assert_eq!(
        exported
            .load_state_at("421337", "main", Year(9))
            .unwrap()
            .current_year,
        Some(Year(9))
    );
}
