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
    fn from_e3_dump() -> Self {
        let serial = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "chargen-e3-fixture-{}-{serial}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("e3.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        connection
            .execute_batch(include_str!("fixtures/e3-b0.sql"))
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

/// The SQL fixture is a read-only dump of the E3 B0 database produced by
/// engine commit 5b11a50. The hash goldens below were captured independently
/// from that pre-E4 database's annual_commits rows.
#[test]
fn opens_replays_and_resumes_a_real_e3_database_without_rewriting_history() {
    const WORLD_ID: &str = "421450";
    const BRANCH_ID: &str = "main";

    let temp = TempDb::from_e3_dump();
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
    let scenario: Scenario = serde_json::from_str(&scenario_json).unwrap();
    assert_eq!(scenario.schema_version, 3);
    assert_eq!(scenario.rules_version, 3);
    assert!(scenario
        .scheduled_events
        .as_ref()
        .is_some_and(|events| !events.is_empty()));
    let validated = scenario.validate().unwrap();
    let mut replay = Engine::from_scenario(validated.clone()).unwrap();

    let expected_state_hashes = [
        "27dad7f9bf2fb718",
        "84bf2bae7389f6e5",
        "2816e6f54b4f1d85",
        "e6e8d5465935cfb0",
        "a8f589a76614452a",
        "a4c90e09e726d86f",
        "6d91727dc46adb2a",
        "a80542bc601c4460",
        "1ba85d6c74bf0fae",
        "3267f3fa656ad7e5",
    ];
    let expected_event_hashes = [
        "be5c244e099a499a",
        "71660be3b365a5b2",
        "53e69cd62ad7ec2f",
        "d419e9fa72444187",
        "3187e68094d3024e",
        "938ea952242bf664",
        "7e142d10bf51c127",
        "128515948dc6c06a",
        "291f37bf35b006d3",
        "5d22f5afb8df54cb",
    ];
    let expected_commit_hashes = [
        "6df17bcff21dcf2a",
        "1c6dfd84c185a839",
        "67a00eda4afbe4e5",
        "c5a948f7c537632c",
        "de3c753d1488465d",
        "40a89b719b1991bf",
        "6f4494d1a51ccffe",
        "188a6f8b6f1933c1",
        "6eec34cb23544293",
        "900172a9b7a1f62f",
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
        assert_eq!(state.rules_version, 3);
        assert!(serde_json::to_value(&state)
            .unwrap()
            .get("economy")
            .is_some());

        let replayed = replay.advance_year(None).unwrap();
        assert_eq!(replayed.year, Year(year));
        assert_eq!(replayed.state, state, "E3 replay state at year {year}");
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
            "E3 replay events at year {year}"
        );
    }
    drop(connection);

    let head_state = store.load_state(WORLD_ID, BRANCH_ID).unwrap();
    assert_eq!(head_state.current_year, Some(Year(9)));
    let mut resumed = Engine::new(validated, head_state).unwrap();
    let next = resumed.advance_year(None).unwrap();
    assert_eq!(next.year, Year(10));
    assert_eq!(next.state.current_year, Some(Year(10)));

    // Read, replay and resume leave all pre-E4 bytes and committed hashes intact.
    assert_eq!(fs::read(&temp.0).unwrap(), before);
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
    }
}
