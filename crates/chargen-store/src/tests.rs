use super::*;
use chargen_domain::{
    DemographicPolicies, EntityRef, GeneticCatalog, Genome, HistoricalEventKind, LogicalId, Person,
    PersonId, ReproductiveProfile, Scenario, Settlement, SettlementId, WorldId,
};
use chargen_engine::{Engine, EngineError};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(1);

struct TempWorkspace(PathBuf);

impl TempWorkspace {
    fn new() -> Self {
        let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("chargen-store-test-{}-{id}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn db(&self) -> PathBuf {
        self.0.join("worlds.sqlite3")
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn tiny_scenario(world: u64, with_person: bool) -> Scenario {
    let settlement_id = SettlementId(1);
    let mut scenario = Scenario {
        world_id: WorldId(world),
        schema_version: Scenario::SUPPORTED_SCHEMA_VERSION,
        rules_version: Scenario::SUPPORTED_RULES_VERSION,
        seed: 73,
        start_year: Year(0),
        settlements: vec![Settlement {
            id: settlement_id,
            logical_id: LogicalId(101),
            name: "Aster".into(),
            culture_values: BTreeMap::new(),
        }],
        initial_people: Vec::new(),
        initial_households: Vec::new(),
        initial_unions: Vec::new(),
        initial_family_links: Vec::new(),
        trait_catalog: Vec::new(),
        initial_population_profiles: Vec::new(),
        policies: DemographicPolicies::default(),
        genetics: GeneticCatalog::default(),
        territory: None,
        culture: None,
        economy: None,
        scheduled_events: None,
    };
    if with_person {
        scenario.initial_people.push(Person {
            id: PersonId(1),
            logical_id: LogicalId(201),
            birth_year: Year(0),
            death_year: None,
            settlement_id,
            household_id: None,
            reproductive: ReproductiveProfile::default(),
            traits: BTreeMap::new(),
            genome: Genome::default(),
            last_widowed_year: None,
            profession: None,
        });
    }
    scenario
}

fn initial_state(scenario: &Scenario) -> State {
    Engine::from_scenario(scenario.validate().unwrap())
        .unwrap()
        .state()
        .clone()
}

fn engine_for(scenario: &Scenario, state: State) -> Engine {
    Engine::new(scenario.validate().unwrap(), state).unwrap()
}

fn create_store(
    workspace: &TempWorkspace,
    scenario: &Scenario,
    branch_id: &str,
) -> (HistoryStore, State) {
    let store = HistoryStore::open(workspace.db()).unwrap();
    let state = initial_state(scenario);
    let json = serde_json::to_string(scenario).unwrap();
    store
        .create_world(&scenario.world_id.0.to_string(), branch_id, &json, &state)
        .unwrap();
    (store, state)
}

fn next_year(engine: &mut Engine) -> Result<YearCommit, EngineError> {
    engine.advance_year(None)
}

fn add_test_event(commit: &mut YearCommit) {
    commit.events.push(HistoricalEvent {
        id: LogicalId(99_001),
        year: commit.year,
        order: 0,
        affected_entities: vec![EntityRef::Person(PersonId(1))],
        kind: HistoricalEventKind::HouseholdChanged {
            person_id: PersonId(1),
            from: None,
            to: None,
        },
    });
}

#[test]
fn annual_commits_resume_and_catalog_are_durable() {
    let workspace = TempWorkspace::new();
    let scenario = tiny_scenario(101, true);
    let (store, state) = create_store(&workspace, &scenario, "main");
    let store = store.with_snapshot_interval(2);
    let mut engine = engine_for(&scenario, state.clone());
    let mut writer = store.acquire_writer("101", "main").unwrap();
    assert!(matches!(
        store.acquire_writer("101", "main"),
        Err(StoreError::WriterAlreadyActive(_))
    ));

    let mut closed_states = Vec::new();
    for expected_year in 0..3 {
        let commit = next_year(&mut engine).unwrap();
        assert_eq!(commit.year, Year(expected_year));
        closed_states.push(commit.state.clone());
        writer.commit_year(&commit).unwrap();
    }
    drop(writer);

    assert_eq!(store.head("101", "main").unwrap(), Some(Year(2)));
    assert_eq!(store.load_state("101", "main").unwrap(), *engine.state());
    for (year, expected) in closed_states.iter().enumerate() {
        assert_eq!(
            store
                .load_state_at("101", "main", Year(year as i32))
                .unwrap(),
            *expected
        );
    }
    store.checkpoint("101", "main").unwrap();
    assert_eq!(store.list_worlds().unwrap()[0].branch_count, 1);
    assert_eq!(store.list_branches("101").unwrap()[0].head_year, Some(2));
    assert_eq!(
        store.scenario_json("101", "main").unwrap(),
        serde_json::to_string(&scenario).unwrap()
    );

    let mut restored_engine = engine_for(&scenario, store.load_state("101", "main").unwrap());
    let mut continued = engine;
    let expected = next_year(&mut continued).unwrap();
    let actual = next_year(&mut restored_engine).unwrap();
    assert_eq!(expected, actual);
}

#[test]
fn historical_fork_shares_only_its_prefix_and_allows_policy_edits() {
    let workspace = TempWorkspace::new();
    let mut scenario = tiny_scenario(102, true);
    let (store, state) = create_store(&workspace, &scenario, "main");
    let mut engine = engine_for(&scenario, state);
    {
        let mut writer = store.acquire_writer("102", "main").unwrap();
        for _ in 0..3 {
            writer
                .commit_year(&next_year(&mut engine).unwrap())
                .unwrap();
        }
    }

    scenario.policies.mortality.default_probability_ppm = 1;
    let scenario_json = serde_json::to_string(&scenario).unwrap();
    let parent_revision = store.revision_id_at("102", "main", Year(0)).unwrap();
    let cut_state = store
        .fork_branch("102", "main", "edited", Year(0), &scenario_json)
        .unwrap();
    assert_eq!(
        store.revision_id_at("102", "edited", Year(0)).unwrap(),
        parent_revision
    );
    assert_eq!(
        store.scenario_json_at("102", "edited", Year(0)).unwrap(),
        store.scenario_json_at("102", "main", Year(0)).unwrap()
    );
    assert_eq!(
        cut_state,
        store.load_state_at("102", "main", Year(0)).unwrap()
    );
    assert!(matches!(
        store.load_state_at("102", "edited", Year(1)),
        Err(StoreError::YearUnavailable { .. })
    ));

    let mut child_engine = engine_for(&scenario, cut_state);
    let mut writer = store.acquire_writer("102", "edited").unwrap();
    let child_year_one = next_year(&mut child_engine).unwrap();
    writer.commit_year(&child_year_one).unwrap();
    drop(writer);
    assert_eq!(
        store.revision_id_at("102", "edited", Year(1)).unwrap(),
        store
            .list_branches("102")
            .unwrap()
            .into_iter()
            .find(|branch| branch.branch_id == "edited")
            .unwrap()
            .revision_id
    );
    assert_eq!(
        store.load_state_at("102", "edited", Year(1)).unwrap(),
        child_year_one.state
    );
    assert_eq!(
        store.load_state_at("102", "main", Year(2)).unwrap(),
        engine.state().clone()
    );
}

#[test]
fn unchanged_fork_replays_the_same_closes_as_its_parent() {
    let workspace = TempWorkspace::new();
    let scenario = tiny_scenario(110, true);
    let (store, state) = create_store(&workspace, &scenario, "main");
    let mut parent_engine = engine_for(&scenario, state);
    let mut parent_states = BTreeMap::new();
    {
        let mut writer = store.acquire_writer("110", "main").unwrap();
        for _ in 0..3 {
            let commit = next_year(&mut parent_engine).unwrap();
            parent_states.insert(commit.year, commit.state.clone());
            writer.commit_year(&commit).unwrap();
        }
    }

    let child_initial = store
        .fork_branch(
            "110",
            "main",
            "unchanged",
            Year(0),
            &serde_json::to_string(&scenario).unwrap(),
        )
        .unwrap();
    let mut child_engine = engine_for(&scenario, child_initial);
    let mut writer = store.acquire_writer("110", "unchanged").unwrap();
    for year in [Year(1), Year(2)] {
        let commit = next_year(&mut child_engine).unwrap();
        assert_eq!(commit.year, year);
        assert_eq!(commit.state, parent_states[&year]);
        writer.commit_year(&commit).unwrap();
        assert_eq!(
            store.load_state_at("110", "unchanged", year).unwrap(),
            parent_states[&year]
        );
    }
}

#[test]
fn historical_fork_rejects_changes_to_the_initial_prefix() {
    let workspace = TempWorkspace::new();
    let scenario = tiny_scenario(103, true);
    let (store, state) = create_store(&workspace, &scenario, "main");
    let mut engine = engine_for(&scenario, state);
    {
        let mut writer = store.acquire_writer("103", "main").unwrap();
        writer
            .commit_year(&next_year(&mut engine).unwrap())
            .unwrap();
    }

    let mut edited = scenario.clone();
    edited.initial_people[0].birth_year = Year(-1);
    let error = store
        .fork_branch(
            "103",
            "main",
            "invalid-prefix",
            Year(0),
            &serde_json::to_string(&edited).unwrap(),
        )
        .unwrap_err();
    assert!(matches!(error, StoreError::ScenarioInvalid(_)));
    assert_eq!(store.list_branches("103").unwrap().len(), 1);
}

#[test]
fn e2_branch_revision_keeps_cutoff_displacements_and_referenced_institutions() {
    let workspace = TempWorkspace::new();
    let scenario: Scenario = serde_json::from_str(include_str!(
        "../../../scenarios/demos/e2-territory-culture.json"
    ))
    .unwrap();
    let (store, state) = create_store(&workspace, &scenario, "main");
    let mut engine = engine_for(&scenario, state);
    {
        let mut writer = store.acquire_writer("2", "main").unwrap();
        for _ in 0..=1 {
            writer
                .commit_year(&next_year(&mut engine).unwrap())
                .unwrap();
        }
    }

    let mut revised = scenario.clone();
    revised
        .territory
        .as_mut()
        .unwrap()
        .scheduled_displacements
        .clear();
    let error = store
        .fork_branch(
            "2",
            "main",
            "removed-command",
            Year(1),
            &serde_json::to_string(&revised).unwrap(),
        )
        .unwrap_err();
    assert!(matches!(error, StoreError::ScenarioInvalid(_)));

    let mut revised = scenario.clone();
    revised.culture.as_mut().unwrap().institutions.clear();
    let error = store
        .fork_branch(
            "2",
            "main",
            "removed-institution",
            Year(0),
            &serde_json::to_string(&revised).unwrap(),
        )
        .unwrap_err();
    assert!(matches!(error, StoreError::ScenarioInvalid(_)));

    let mut revised = scenario.clone();
    revised
        .culture
        .as_mut()
        .unwrap()
        .organizations
        .formation_opportunity_ppm = 500_000;
    store
        .fork_branch(
            "2",
            "main",
            "future-policy",
            Year(0),
            &serde_json::to_string(&revised).unwrap(),
        )
        .unwrap();
    assert_eq!(store.head("2", "future-policy").unwrap(), Some(Year(0)));
    assert_eq!(store.list_branches("2").unwrap().len(), 2);
}

#[test]
fn e3_branch_revision_allows_future_economic_and_event_policies_only() {
    let workspace = TempWorkspace::new();
    let scenario: Scenario = serde_json::from_str(include_str!(
        "../../../scenarios/demos/e3-economy-events.json"
    ))
    .unwrap();
    let (store, state) = create_store(&workspace, &scenario, "main");
    let mut engine = engine_for(&scenario, state);
    store
        .acquire_writer("3", "main")
        .unwrap()
        .commit_year(&next_year(&mut engine).unwrap())
        .unwrap();

    let mut future_revision = scenario.clone();
    future_revision.scheduled_events.as_mut().unwrap().clear();
    future_revision
        .economy
        .as_mut()
        .unwrap()
        .price_policy
        .adjustment_step_ppm = 100_000;
    store
        .fork_branch(
            "3",
            "main",
            "future-policy",
            Year(0),
            &serde_json::to_string(&future_revision).unwrap(),
        )
        .unwrap();

    let mut changed_endowment = future_revision.clone();
    changed_endowment
        .economy
        .as_mut()
        .unwrap()
        .initial_group_endowments[0]
        .endowment
        .money
        .0 += 1;
    assert!(matches!(
        store.fork_branch(
            "3",
            "main",
            "changed-endowment",
            Year(0),
            &serde_json::to_string(&changed_endowment).unwrap(),
        ),
        Err(StoreError::ScenarioInvalid(_))
    ));

    let mut writer = store.acquire_writer("3", "main").unwrap();
    writer
        .commit_year(&next_year(&mut engine).unwrap())
        .unwrap();
    drop(writer);
    let mut changed_past_event = scenario.clone();
    changed_past_event
        .scheduled_events
        .as_mut()
        .unwrap()
        .clear();
    assert!(matches!(
        store.fork_branch(
            "3",
            "main",
            "changed-past-event",
            Year(1),
            &serde_json::to_string(&changed_past_event).unwrap(),
        ),
        Err(StoreError::ScenarioInvalid(_))
    ));
}

#[test]
fn initial_fork_materializes_edited_population_without_mutating_parent() {
    let workspace = TempWorkspace::new();
    let scenario = tiny_scenario(104, true);
    let (store, state) = create_store(&workspace, &scenario, "main");
    let mut engine = engine_for(&scenario, state.clone());
    {
        let mut writer = store.acquire_writer("104", "main").unwrap();
        writer
            .commit_year(&next_year(&mut engine).unwrap())
            .unwrap();
    }

    let mut edited = scenario.clone();
    edited.initial_people[0].birth_year = Year(-10);
    let child_state = store
        .fork_initial_branch(
            "104",
            "main",
            "redesigned-start",
            &serde_json::to_string(&edited).unwrap(),
        )
        .unwrap();
    assert_eq!(child_state.current_year, None);
    assert_eq!(child_state.persons[&PersonId(1)].birth_year, Year(-10));
    assert_eq!(
        store.load_state("104", "main").unwrap(),
        engine.state().clone()
    );
    assert_eq!(
        store.load_state("104", "redesigned-start").unwrap(),
        child_state
    );

    let mut child_engine = engine_for(&edited, child_state);
    let mut writer = store.acquire_writer("104", "redesigned-start").unwrap();
    writer
        .commit_year(&next_year(&mut child_engine).unwrap())
        .unwrap();
    assert_eq!(
        store.head("104", "redesigned-start").unwrap(),
        Some(Year(0))
    );
}

#[test]
fn injected_failures_roll_back_the_entire_annual_transaction() {
    for (world, failure_point) in [
        (105, CommitFailurePoint::AfterCommitRow),
        (106, CommitFailurePoint::AfterEventInsert),
        (107, CommitFailurePoint::BeforeHeadUpdate),
    ] {
        let workspace = TempWorkspace::new();
        let scenario = tiny_scenario(world, true);
        let (store, initial) = create_store(&workspace, &scenario, "main");
        let mut engine = engine_for(&scenario, initial.clone());
        let mut commit = next_year(&mut engine).unwrap();
        add_test_event(&mut commit);

        let mut writer = store.acquire_writer(&world.to_string(), "main").unwrap();
        writer.snapshot_interval = 1;
        writer.fail_at = Some(failure_point);
        assert!(writer.commit_year(&commit).is_err());
        writer.fail_at = None;

        assert_eq!(store.head(&world.to_string(), "main").unwrap(), None);
        assert_eq!(
            store.load_state(&world.to_string(), "main").unwrap(),
            initial
        );
        let annual_rows: i64 = writer
            .conn
            .query_row("SELECT COUNT(*) FROM annual_commits", [], |row| row.get(0))
            .unwrap();
        let event_rows: i64 = writer
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        let snapshot_rows: i64 = writer
            .conn
            .query_row("SELECT COUNT(*) FROM snapshots", [], |row| row.get(0))
            .unwrap();
        assert_eq!((annual_rows, event_rows, snapshot_rows), (0, 0, 0));

        writer.commit_year(&commit).unwrap();
        assert_eq!(
            store.head(&world.to_string(), "main").unwrap(),
            Some(Year(0))
        );
    }
}

#[test]
fn export_restore_round_trip_and_manifest_validation() {
    let workspace = TempWorkspace::new();
    let scenario = tiny_scenario(108, true);
    let (store, state) = create_store(&workspace, &scenario, "main");
    let mut engine = engine_for(&scenario, state);
    {
        let mut writer = store.acquire_writer("108", "main").unwrap();
        writer
            .commit_year(&next_year(&mut engine).unwrap())
            .unwrap();
    }

    let exported = workspace.0.join("export.sqlite3");
    let manifest = store.export_world(&exported).unwrap();
    assert_eq!(manifest.schema_version, CURRENT_SCHEMA_VERSION);
    let restored_path = workspace.0.join("restored.sqlite3");
    let restored = HistoryStore::restore_world(&exported, &restored_path).unwrap();
    assert_eq!(
        restored.load_state("108", "main").unwrap(),
        engine.state().clone()
    );
    assert!(matches!(
        store.export_world(&exported),
        Err(StoreError::Invalid(_))
    ));

    let tampered_path = workspace.0.join("tampered.sqlite3");
    fs::copy(&exported, &tampered_path).unwrap();
    fs::copy(manifest_path(&exported), manifest_path(&tampered_path)).unwrap();
    let mut bytes = fs::read(&tampered_path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(&tampered_path, bytes).unwrap();
    assert!(matches!(
        HistoryStore::restore_world(&tampered_path, workspace.0.join("bad-restore.sqlite3")),
        Err(StoreError::Integrity(_))
    ));
}

#[test]
fn selected_world_export_does_not_mix_the_rest_of_the_catalog() {
    let workspace = TempWorkspace::new();
    let store = HistoryStore::open(workspace.db()).unwrap();
    for world in [111, 112] {
        let scenario = tiny_scenario(world, true);
        let initial = initial_state(&scenario);
        store
            .create_world(
                &world.to_string(),
                "main",
                &serde_json::to_string(&scenario).unwrap(),
                &initial,
            )
            .unwrap();
        let mut engine = engine_for(&scenario, initial);
        store
            .acquire_writer(&world.to_string(), "main")
            .unwrap()
            .commit_year(&next_year(&mut engine).unwrap())
            .unwrap();
    }

    let exported = workspace.0.join("one-world.sqlite3");
    let manifest = store.export_single_world("111", &exported).unwrap();
    assert_eq!(manifest.world_ids, vec!["111"]);
    let restored_path = workspace.0.join("one-world-restored.sqlite3");
    let restored = HistoryStore::restore_world(&exported, restored_path).unwrap();
    assert_eq!(restored.list_worlds().unwrap().len(), 1);
    assert_eq!(restored.list_worlds().unwrap()[0].world_id, "111");
    assert_eq!(store.list_worlds().unwrap().len(), 2);
    assert!(matches!(
        store.export_single_world("missing", workspace.0.join("missing.sqlite3")),
        Err(StoreError::WorldNotFound(_))
    ));
}

#[test]
fn event_payload_tampering_is_detected_on_historical_load() {
    let workspace = TempWorkspace::new();
    let scenario = tiny_scenario(109, true);
    let (store, state) = create_store(&workspace, &scenario, "main");
    let mut engine = engine_for(&scenario, state);
    let mut commit = next_year(&mut engine).unwrap();
    add_test_event(&mut commit);
    {
        let mut writer = store.acquire_writer("109", "main").unwrap();
        writer.commit_year(&commit).unwrap();
    }
    let writable = open_rw(&workspace.db()).unwrap();
    writable
        .execute_batch("DROP TRIGGER events_immutable_update;")
        .unwrap();
    writable
        .execute("UPDATE events SET payload_json='{}' WHERE year=0", [])
        .unwrap();
    let error = store.load_state_at("109", "main", Year(0)).unwrap_err();
    assert!(
        matches!(&error, StoreError::Integrity(message) if message.contains("event or commit checksum")),
        "unexpected load error: {error:?}"
    );
}
