//! Read-only projections pinned to one world, branch and confirmed year.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chargen_domain::{
    FamilyLink, FamilyLinkKind, HistoricalEvent, Person, PersonId, SettlementId, State, Year,
};
use chargen_store::{HistoryStore, StoreError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueryError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("person {0} does not exist in the selected historical state")]
    PersonNotFound(u64),
}

pub type QueryResult<T> = Result<T, QueryError>;

#[derive(Clone, Debug)]
pub struct HistoryQuery {
    world_id: String,
    branch_id: String,
    year: Year,
    state: State,
    store: HistoryStore,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonRecord {
    pub person: Person,
    pub age_at_close: Option<u32>,
    pub alive_at_close: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettlementCensus {
    pub settlement_id: SettlementId,
    pub settlement_name: String,
    pub population: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Census {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub total_population: u64,
    pub settlements: Vec<SettlementCensus>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenealogyRelation {
    pub relative: PersonRecord,
    pub link: FamilyLink,
    /// Number of family links between this person and the subject.
    pub distance: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Genealogy {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub subject: PersonRecord,
    pub ancestors: Vec<GenealogyRelation>,
    pub descendants: Vec<GenealogyRelation>,
    /// True when the requested depth or result cap omitted potentially relevant links.
    pub truncated: bool,
}

impl HistoryQuery {
    /// Construct an immutable historical view. The requested year must be a
    /// confirmed close visible in this branch's effective lineage.
    pub fn open(
        path: impl AsRef<Path>,
        world_id: impl Into<String>,
        branch_id: impl Into<String>,
        year: Year,
    ) -> QueryResult<Self> {
        let store = HistoryStore::open_existing(path)?;
        let world_id = world_id.into();
        let branch_id = branch_id.into();
        let state = store.load_state_at(&world_id, &branch_id, year)?;
        Ok(Self {
            world_id,
            branch_id,
            year,
            state,
            store,
        })
    }

    pub fn world_id(&self) -> &str {
        &self.world_id
    }
    pub fn branch_id(&self) -> &str {
        &self.branch_id
    }
    pub fn year(&self) -> Year {
        self.year
    }

    pub fn census(&self, settlement_id: Option<SettlementId>) -> Census {
        let mut populations = BTreeMap::<SettlementId, u64>::new();
        for person in self
            .state
            .persons
            .values()
            .filter(|person| person.alive_at_close(self.year))
        {
            if settlement_id.is_none_or(|requested| requested == person.settlement_id) {
                *populations.entry(person.settlement_id).or_default() += 1;
            }
        }
        let mut settlements = Vec::new();
        for settlement in self.state.settlements.values() {
            if settlement_id.is_some_and(|requested| requested != settlement.id) {
                continue;
            }
            let population = populations.get(&settlement.id).copied().unwrap_or_default();
            settlements.push(SettlementCensus {
                settlement_id: settlement.id,
                settlement_name: settlement.name.clone(),
                population,
            });
        }
        let total_population = settlements.iter().map(|row| row.population).sum();
        Census {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            total_population,
            settlements,
        }
    }

    /// List people born by the selected year, optionally scoped to a settlement
    /// and optionally restricted to people alive at that year's close.
    pub fn people(
        &self,
        settlement_id: Option<SettlementId>,
        living_only: bool,
        offset: usize,
        limit: usize,
    ) -> Vec<PersonRecord> {
        if limit == 0 {
            return Vec::new();
        }
        self.state
            .persons
            .values()
            .filter(|person| person.birth_year <= self.year)
            .filter(|person| {
                settlement_id.is_none_or(|settlement| person.settlement_id == settlement)
            })
            .filter(|person| !living_only || person.alive_at_close(self.year))
            .skip(offset)
            .take(limit)
            .map(|person| person_record(person, self.year))
            .collect()
    }

    pub fn person(&self, person_id: PersonId) -> Option<PersonRecord> {
        self.state
            .persons
            .get(&person_id)
            .filter(|person| person.birth_year <= self.year)
            .map(|person| person_record(person, self.year))
    }

    /// Return known parents/caregivers and descendants to a bounded depth.
    /// Unknown relatives remain absent; this query never invents ancestors.
    pub fn genealogy(&self, person_id: PersonId, maximum_depth: usize) -> QueryResult<Genealogy> {
        let subject = self
            .person(person_id)
            .ok_or(QueryError::PersonNotFound(person_id.0))?;
        const MAX_DEPTH: usize = 64;
        let truncated_by_depth = maximum_depth > MAX_DEPTH;
        let depth = maximum_depth.min(MAX_DEPTH);
        let links: Vec<&FamilyLink> = self
            .state
            .family_links
            .values()
            .filter(|link| link.active_at_close(self.year))
            .filter(|link| {
                self.state
                    .persons
                    .get(&link.parent)
                    .is_some_and(|p| p.birth_year <= self.year)
            })
            .filter(|link| {
                self.state
                    .persons
                    .get(&link.child)
                    .is_some_and(|p| p.birth_year <= self.year)
            })
            .collect();
        let (ancestors, ancestors_truncated) =
            traverse_relations(&self.state, self.year, person_id, depth, &links, true);
        let (descendants, descendants_truncated) =
            traverse_relations(&self.state, self.year, person_id, depth, &links, false);
        Ok(Genealogy {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            subject,
            ancestors,
            descendants,
            truncated: truncated_by_depth || ancestors_truncated || descendants_truncated,
        })
    }

    /// Historical semantic events involving a person, restricted to this view.
    /// Event ancestry/cutoff filtering is performed by the store index.
    pub fn events_for_person(&self, person_id: PersonId) -> QueryResult<Vec<HistoricalEvent>> {
        Ok(self.store.events_for_entity(
            &self.world_id,
            &self.branch_id,
            self.year,
            "person",
            &person_id.0.to_string(),
        )?)
    }
}

fn person_record(person: &Person, year: Year) -> PersonRecord {
    PersonRecord {
        person: person.clone(),
        age_at_close: person.age_at_close(year),
        alive_at_close: person.alive_at_close(year),
    }
}

fn traverse_relations(
    state: &State,
    year: Year,
    subject: PersonId,
    maximum_depth: usize,
    links: &[&FamilyLink],
    upward: bool,
) -> (Vec<GenealogyRelation>, bool) {
    const MAX_NODES: usize = 10_000;
    let mut output = Vec::new();
    let mut seen_people = BTreeSet::from([subject]);
    let mut seen_links = BTreeSet::new();
    let mut frontier = vec![subject];
    let mut adjacency = BTreeMap::<PersonId, Vec<&FamilyLink>>::new();
    for link in links {
        let source = if upward { link.child } else { link.parent };
        adjacency.entry(source).or_default().push(*link);
    }
    for distance in 1..=maximum_depth {
        let mut next = Vec::new();
        for current in frontier {
            for link in adjacency.get(&current).into_iter().flatten() {
                let (from, to) = if upward {
                    (link.child, link.parent)
                } else {
                    (link.parent, link.child)
                };
                if from != current || to == subject || !seen_links.insert(link.id) {
                    continue;
                }
                if let Some(person) = state.persons.get(&to) {
                    if output.len() >= MAX_NODES {
                        return (output, true);
                    }
                    output.push(GenealogyRelation {
                        relative: person_record(person, year),
                        link: (*link).clone(),
                        distance,
                    });
                    if seen_people.insert(to) {
                        next.push(to);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    (output, false)
}

#[allow(dead_code)]
fn _link_kind_is_explicit(_kind: FamilyLinkKind) {}

#[cfg(test)]
mod tests {
    use super::*;
    use chargen_domain::{
        DemographicPolicies, EntityRef, FamilyLinkId, FamilyLinkKind, GeneticCatalog, Genome,
        HistoricalEventKind, LogicalId, Person, ReproductiveProfile, Scenario, Settlement, WorldId,
    };
    use chargen_engine::Engine;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(1);

    struct TempWorkspace(PathBuf);

    impl TempWorkspace {
        fn new() -> Self {
            let id = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("chargen-query-test-{}-{id}", std::process::id()));
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

    fn scenario() -> Scenario {
        let settlement_id = SettlementId(1);
        let parent = Person {
            id: PersonId(1),
            logical_id: LogicalId(101),
            birth_year: Year(-40),
            death_year: None,
            settlement_id,
            household_id: None,
            reproductive: ReproductiveProfile::default(),
            traits: BTreeMap::new(),
            genome: Genome::default(),
            last_widowed_year: None,
        };
        let child = Person {
            id: PersonId(2),
            logical_id: LogicalId(102),
            birth_year: Year(-20),
            death_year: None,
            settlement_id,
            household_id: None,
            reproductive: ReproductiveProfile::default(),
            traits: BTreeMap::new(),
            genome: Genome::default(),
            last_widowed_year: None,
        };
        let link = |id, logical_id, kind| FamilyLink {
            id: FamilyLinkId(id),
            logical_id: LogicalId(logical_id),
            parent: PersonId(1),
            child: PersonId(2),
            kind,
            validity: chargen_domain::YearInterval {
                start: Year(-20),
                end_exclusive: None,
            },
        };
        Scenario {
            world_id: WorldId(201),
            schema_version: Scenario::SUPPORTED_SCHEMA_VERSION,
            rules_version: Scenario::SUPPORTED_RULES_VERSION,
            seed: 19,
            start_year: Year(0),
            settlements: vec![Settlement {
                id: settlement_id,
                logical_id: LogicalId(11),
                name: "Harbor".into(),
                culture_values: BTreeMap::new(),
            }],
            initial_people: vec![parent, child],
            initial_households: Vec::new(),
            initial_unions: Vec::new(),
            initial_family_links: vec![
                link(1, 301, FamilyLinkKind::BiologicalParent),
                link(2, 302, FamilyLinkKind::AdoptiveParent),
            ],
            trait_catalog: Vec::new(),
            initial_population_profiles: Vec::new(),
            policies: DemographicPolicies::default(),
            genetics: GeneticCatalog::default(),
        }
    }

    #[test]
    fn genealogy_preserves_distinct_links_to_the_same_relative() {
        let temp = TempWorkspace::new();
        let scenario = scenario();
        let state = Engine::from_scenario(scenario.validate().unwrap())
            .unwrap()
            .state()
            .clone();
        let store = HistoryStore::open(temp.db()).unwrap();
        store
            .create_world(
                "201",
                "main",
                &serde_json::to_string(&scenario).unwrap(),
                &state,
            )
            .unwrap();
        let mut engine = Engine::new(scenario.validate().unwrap(), state).unwrap();
        let commit = engine.advance_year(None).unwrap();
        store
            .acquire_writer("201", "main")
            .unwrap()
            .commit_year(&commit)
            .unwrap();

        let query = HistoryQuery::open(temp.db(), "201", "main", Year(0)).unwrap();
        let family = query.genealogy(PersonId(2), 4).unwrap();
        assert_eq!(family.ancestors.len(), 2);
        assert!(family
            .ancestors
            .iter()
            .all(|relation| relation.relative.person.id == PersonId(1)));
        assert_eq!(
            family
                .ancestors
                .iter()
                .map(|relation| relation.link.kind)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                FamilyLinkKind::BiologicalParent,
                FamilyLinkKind::AdoptiveParent,
            ])
        );
    }

    #[test]
    fn child_year_view_and_event_index_exclude_mothers_post_cut_future() {
        let temp = TempWorkspace::new();
        let scenario = scenario();
        let initial = Engine::from_scenario(scenario.validate().unwrap())
            .unwrap()
            .state()
            .clone();
        let store = HistoryStore::open(temp.db()).unwrap();
        let scenario_json = serde_json::to_string(&scenario).unwrap();
        store
            .create_world("201", "main", &scenario_json, &initial)
            .unwrap();
        let mut mother_engine = Engine::new(scenario.validate().unwrap(), initial).unwrap();
        let year_zero = mother_engine.advance_year(None).unwrap();
        store
            .acquire_writer("201", "main")
            .unwrap()
            .commit_year(&year_zero)
            .unwrap();
        let cut_state = store
            .fork_branch("201", "main", "child", Year(0), &scenario_json)
            .unwrap();

        let mut mother_year_one = mother_engine.advance_year(None).unwrap();
        let newborn = Person {
            id: PersonId(3),
            logical_id: LogicalId(103),
            birth_year: Year(1),
            death_year: None,
            settlement_id: SettlementId(1),
            household_id: None,
            reproductive: ReproductiveProfile::default(),
            traits: BTreeMap::new(),
            genome: Genome::default(),
            last_widowed_year: None,
        };
        mother_year_one.state.persons.insert(newborn.id, newborn);
        mother_year_one.state.next_person_id = 4;
        let new_link = FamilyLink {
            id: FamilyLinkId(3),
            logical_id: LogicalId(303),
            parent: PersonId(2),
            child: PersonId(3),
            kind: FamilyLinkKind::BiologicalParent,
            validity: chargen_domain::YearInterval {
                start: Year(1),
                end_exclusive: None,
            },
        };
        mother_year_one
            .state
            .family_links
            .insert(new_link.id, new_link);
        mother_year_one.state.next_family_link_id = 4;
        mother_year_one.events.push(HistoricalEvent {
            id: LogicalId(701),
            year: Year(1),
            order: 0,
            affected_entities: vec![
                EntityRef::Person(PersonId(2)),
                EntityRef::Person(PersonId(3)),
                EntityRef::FamilyLink(FamilyLinkId(3)),
            ],
            kind: HistoricalEventKind::FamilyLinkEstablished {
                link_id: FamilyLinkId(3),
                parent: PersonId(2),
                child: PersonId(3),
                kind: FamilyLinkKind::BiologicalParent,
            },
        });
        store
            .acquire_writer("201", "main")
            .unwrap()
            .commit_year(&mother_year_one)
            .unwrap();

        let mut child_engine = Engine::new(scenario.validate().unwrap(), cut_state).unwrap();
        let child_year_one = child_engine.advance_year(None).unwrap();
        store
            .acquire_writer("201", "child")
            .unwrap()
            .commit_year(&child_year_one)
            .unwrap();

        let mother_view = HistoryQuery::open(temp.db(), "201", "main", Year(1)).unwrap();
        let child_view = HistoryQuery::open(temp.db(), "201", "child", Year(1)).unwrap();
        assert_eq!(
            mother_view.person(PersonId(3)).unwrap().person.birth_year,
            Year(1)
        );
        assert!(child_view.person(PersonId(3)).is_none());
        assert_eq!(
            mother_view
                .genealogy(PersonId(2), 4)
                .unwrap()
                .descendants
                .len(),
            1
        );
        assert!(child_view
            .genealogy(PersonId(2), 4)
            .unwrap()
            .descendants
            .is_empty());
        assert_eq!(mother_view.events_for_person(PersonId(2)).unwrap().len(), 1);
        assert!(child_view
            .events_for_person(PersonId(2))
            .unwrap()
            .is_empty());
    }
}
