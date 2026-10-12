//! Deterministic annual simulation with no filesystem, database, clock, or network access.

use chargen_domain::{
    AdoptionPolicy, AgeMortalityBand, Allele, BirthPolicy, DeathCause, EntityRef, FamilyLink,
    FamilyLinkEndReason, FamilyLinkId, FamilyLinkKind, GeneticCatalog, HistoricalEvent,
    HistoricalEventKind, Household, HouseholdId, LogicalId, Person, PersonId, PhaseDisposition,
    PhaseRecord, ReproductiveProfile, ResidenceInterval, SettlementId, SettlementYearBalance,
    State, TraitKey, Union, UnionEndReason, UnionId, ValidatedScenario, ValidationIssue, Year,
    YearInterval, YearPhase,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use thiserror::Error;

pub mod culture;
pub mod economy;
mod events;
mod migration;
pub use culture::*;
pub use economy::*;

/// Keyed RNG contract. Changing its mixing or domain numbers requires a new version.
pub const RNG_VERSION: u32 = 1;
const RNG_ALGORITHM: &str = "chargen-splitmix-keyed-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub enum DecisionDomain {
    Mortality = 1,
    UnionSeparation = 2,
    UnionFormation = 3,
    UnionCandidate = 4,
    UnionIdentity = 5,
    BirthChance = 6,
    BirthContributor = 7,
    PersonIdentity = 8,
    FounderGenome = 9,
    AlleleInheritance = 10,
    InfantMortality = 11,
    AdoptionChance = 12,
    AdoptionCandidate = 13,
    HouseholdIdentity = 14,
    FamilyLinkIdentity = 15,
    TraitVariation = 16,
    ReproductiveProfile = 17,
    InitialAge = 18,
    InitialTrait = 19,
    CultureVariation = 20,
    InstitutionAffiliation = 21,
    OrganizationCandidate = 22,
    OrganizationIdentity = 23,
    CultureEvent = 24,
    MigrationOpportunity = 25,
    MigrationDestination = 26,
    MigrationPriority = 27,
    ForcedDisplacementPriority = 28,
    EventSelection = 29,
    EventProbability = 30,
    InitialProfession = 31,
    EconomicMarketPriority = 32,
    EconomicProfessionChange = 33,
}

/// A cancellation request is observed between annual phases; the pending year is discarded.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct YearCommit {
    pub year: Year,
    pub state: State,
    /// Only events created during this year; full history belongs to the repository.
    pub events: Vec<HistoricalEvent>,
}

/// Injectable monotonic clock used only for process metrics; never enters state or RNG keys.
pub trait MetricsClock: Send + Sync {
    fn now_nanos(&self) -> u128;
}

struct MonotonicClock(Instant);

impl Default for MonotonicClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}

impl MetricsClock for MonotonicClock {
    fn now_nanos(&self) -> u128 {
        self.0.elapsed().as_nanos()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhaseMetric {
    pub phase: YearPhase,
    pub duration_nanos: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DemographicBalance {
    pub settlement_id: SettlementId,
    pub before: u64,
    pub births: u64,
    pub deaths: u64,
    pub incoming: u64,
    pub outgoing: u64,
    pub after: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EngineMetrics {
    pub year: Year,
    pub phases: Vec<PhaseMetric>,
    pub total_duration_nanos: u128,
    pub census_balance: Vec<DemographicBalance>,
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("simulation cancelled before annual commit")]
    Cancelled,
    #[error("year counter overflow")]
    YearOverflow,
    #[error("storage ID allocator exhausted for {0}")]
    IdExhausted(&'static str),
    #[error("invalid state: {0:?}")]
    InvalidState(Vec<ValidationIssue>),
    #[error("invalid generated population: {0}")]
    InvalidPopulation(String),
    #[error("invalid migration: {0}")]
    InvalidMigration(String),
    #[error("culture processing failed: {0}")]
    Culture(String),
    #[error("scheduled event processing failed: {0}")]
    ScheduledEvent(String),
    #[error("economic processing failed: {0}")]
    Economy(String),
    #[error(
        "demographic balance failed for settlement {settlement_id:?}: expected {expected}, found {actual}"
    )]
    DemographicBalance {
        settlement_id: SettlementId,
        expected: u64,
        actual: u64,
    },
    #[error("global demographic balance failed: expected {expected}, found {actual}")]
    GlobalDemographicBalance { expected: u64, actual: u64 },
}

pub struct Engine {
    scenario: ValidatedScenario,
    state: State,
    clock: Arc<dyn MetricsClock>,
    last_metrics: Option<EngineMetrics>,
    route_cache: migration::RouteCache,
    rebase_culture_pending: bool,
}

/// Per-year indexes prevent each person from rescanning every relationship or household.
#[derive(Default)]
pub(crate) struct AnnualIndexes {
    settlement_people: BTreeMap<SettlementId, Vec<PersonId>>,
    settlement_households: BTreeMap<SettlementId, Vec<HouseholdId>>,
    active_unions_by_person: BTreeMap<PersonId, BTreeSet<UnionId>>,
    active_union_ids_by_person: BTreeMap<PersonId, Vec<UnionId>>,
    birth_candidates_by_settlement: BTreeMap<SettlementId, Vec<PersonId>>,
    birth_candidates_by_settlement_gamete:
        BTreeMap<(SettlementId, chargen_domain::Gamete), Vec<PersonId>>,
    birth_candidates_by_union: BTreeMap<(UnionId, Option<chargen_domain::Gamete>), Vec<PersonId>>,
    kinship_neighbors: BTreeMap<PersonId, BTreeSet<PersonId>>,
    children_by_parent: BTreeMap<PersonId, BTreeSet<PersonId>>,
    family_links_by_child: BTreeMap<PersonId, Vec<FamilyLinkId>>,
    caregivers_by_child: BTreeMap<PersonId, BTreeSet<PersonId>>,
    caregiver_links_by_parent: BTreeMap<PersonId, Vec<FamilyLinkId>>,
    adoption_households_by_settlement: BTreeMap<SettlementId, BTreeSet<HouseholdId>>,
    household_adults: BTreeMap<HouseholdId, Vec<PersonId>>,
    dependent_count_by_household: BTreeMap<HouseholdId, u32>,
}

impl AnnualIndexes {
    pub(crate) fn rebuild(
        state: &State,
        year: Year,
        adoption: &AdoptionPolicy,
        births: &BirthPolicy,
    ) -> Self {
        let mut indexes = Self::default();
        for person in state
            .persons
            .values()
            .filter(|person| person.alive_at_close(year))
        {
            indexes
                .settlement_people
                .entry(person.settlement_id)
                .or_default()
                .push(person.id);
            if eligible_birth_contributor(state, person.id, year, births) {
                indexes
                    .birth_candidates_by_settlement
                    .entry(person.settlement_id)
                    .or_default()
                    .push(person.id);
                if let Some(gamete) = births.required_contributor_gamete {
                    if person.reproductive.gametes.contains(&gamete) {
                        indexes
                            .birth_candidates_by_settlement_gamete
                            .entry((person.settlement_id, gamete))
                            .or_default()
                            .push(person.id);
                    }
                }
            }
        }
        for ids in indexes.settlement_people.values_mut() {
            ids.sort_by_key(|id| state.persons[id].logical_id);
        }
        for ids in indexes.birth_candidates_by_settlement.values_mut() {
            ids.sort_by_key(|id| state.persons[id].logical_id);
        }
        for ids in indexes.birth_candidates_by_settlement_gamete.values_mut() {
            ids.sort_by_key(|id| state.persons[id].logical_id);
        }
        for union in state
            .unions
            .values()
            .filter(|union| union.active_at_close(year))
        {
            let mut participants: Vec<_> = union.participants.iter().copied().collect();
            participants.sort_by_key(|id| state.persons[id].logical_id);
            for participant in &participants {
                let unions = indexes
                    .active_union_ids_by_person
                    .entry(*participant)
                    .or_default();
                unions.push(union.id);
            }
            let eligible = participants
                .into_iter()
                .filter(|id| eligible_birth_contributor(state, *id, year, births))
                .collect::<Vec<_>>();
            let key = births.required_contributor_gamete;
            let eligible = if let Some(gamete) = key {
                eligible
                    .into_iter()
                    .filter(|id| state.persons[id].reproductive.gametes.contains(&gamete))
                    .collect()
            } else {
                eligible
            };
            indexes
                .birth_candidates_by_union
                .insert((union.id, key), eligible);
            for participant in &union.participants {
                indexes
                    .active_unions_by_person
                    .entry(*participant)
                    .or_default()
                    .insert(union.id);
            }
        }
        for unions in indexes.active_union_ids_by_person.values_mut() {
            unions.sort_by_key(|id| state.unions[id].logical_id);
        }
        for link in state.family_links.values() {
            indexes
                .kinship_neighbors
                .entry(link.parent)
                .or_default()
                .insert(link.child);
            indexes
                .kinship_neighbors
                .entry(link.child)
                .or_default()
                .insert(link.parent);
            indexes
                .children_by_parent
                .entry(link.parent)
                .or_default()
                .insert(link.child);
            if link.active_at_close(year) {
                indexes
                    .family_links_by_child
                    .entry(link.child)
                    .or_default()
                    .push(link.id);
                if link.kind == FamilyLinkKind::Caregiver {
                    indexes
                        .caregivers_by_child
                        .entry(link.child)
                        .or_default()
                        .insert(link.parent);
                    indexes
                        .caregiver_links_by_parent
                        .entry(link.parent)
                        .or_default()
                        .push(link.id);
                }
            }
        }
        for links in indexes.family_links_by_child.values_mut() {
            links.sort_by_key(|id| state.family_links[id].logical_id);
        }
        for household in state
            .households
            .values()
            .filter(|household| household.closed_year.is_none())
        {
            indexes
                .settlement_households
                .entry(household.settlement_id)
                .or_default()
                .push(household.id);
            for person_id in &household.members {
                let Some(person) = state
                    .persons
                    .get(person_id)
                    .filter(|person| person.alive_at_close(year))
                else {
                    continue;
                };
                let age = year.age_of(person.birth_year).unwrap_or(0);
                if age >= u32::from(adoption.minimum_caregiver_age) {
                    indexes
                        .household_adults
                        .entry(household.id)
                        .or_default()
                        .push(*person_id);
                }
                if age < u32::from(adoption.minor_age) {
                    *indexes
                        .dependent_count_by_household
                        .entry(household.id)
                        .or_default() += 1;
                }
            }
        }
        for ids in indexes.settlement_households.values_mut() {
            ids.sort_by_key(|id| state.households[id].logical_id);
        }
        for adults in indexes.household_adults.values_mut() {
            adults.sort_by_key(|id| state.persons[id].logical_id);
        }
        for household in state
            .households
            .values()
            .filter(|household| household.closed_year.is_none())
        {
            let has_capacity = adoption.maximum_children_per_household.is_none_or(|limit| {
                indexes
                    .dependent_count_by_household
                    .get(&household.id)
                    .copied()
                    .unwrap_or(0)
                    < limit
            });
            if has_capacity
                && indexes
                    .household_adults
                    .get(&household.id)
                    .is_some_and(|adults| !adults.is_empty())
            {
                indexes
                    .adoption_households_by_settlement
                    .entry(household.settlement_id)
                    .or_default()
                    .insert(household.id);
            }
        }
        indexes
    }

    fn add_union(&mut self, union: &Union, state: &State, year: Year, births: &BirthPolicy) {
        let mut participants: Vec<_> = union.participants.iter().copied().collect();
        participants.sort_by_key(|id| state.persons[id].logical_id);
        let mut eligible = participants
            .clone()
            .into_iter()
            .filter(|id| eligible_birth_contributor(state, *id, year, births))
            .collect::<Vec<_>>();
        if let Some(gamete) = births.required_contributor_gamete {
            eligible.retain(|id| state.persons[id].reproductive.gametes.contains(&gamete));
        }
        self.birth_candidates_by_union
            .insert((union.id, births.required_contributor_gamete), eligible);
        for participant in &union.participants {
            self.active_unions_by_person
                .entry(*participant)
                .or_default()
                .insert(union.id);
            let unions = self
                .active_union_ids_by_person
                .entry(*participant)
                .or_default();
            unions.push(union.id);
            unions.sort_by_key(|id| state.unions[id].logical_id);
        }
    }

    fn close_union(&mut self, union: &Union) {
        for participant in &union.participants {
            if let Some(unions) = self.active_unions_by_person.get_mut(participant) {
                unions.remove(&union.id);
                if unions.is_empty() {
                    self.active_unions_by_person.remove(participant);
                }
            }
            if let Some(unions) = self.active_union_ids_by_person.get_mut(participant) {
                unions.retain(|id| *id != union.id);
                if unions.is_empty() {
                    self.active_union_ids_by_person.remove(participant);
                }
            }
        }
        self.birth_candidates_by_union
            .retain(|(union_id, _), _| *union_id != union.id);
    }

    fn add_family_link(&mut self, link: &FamilyLink) {
        self.kinship_neighbors
            .entry(link.parent)
            .or_default()
            .insert(link.child);
        self.kinship_neighbors
            .entry(link.child)
            .or_default()
            .insert(link.parent);
        self.children_by_parent
            .entry(link.parent)
            .or_default()
            .insert(link.child);
        self.family_links_by_child
            .entry(link.child)
            .or_default()
            .push(link.id);
        if link.kind == FamilyLinkKind::Caregiver {
            self.caregivers_by_child
                .entry(link.child)
                .or_default()
                .insert(link.parent);
            self.caregiver_links_by_parent
                .entry(link.parent)
                .or_default()
                .push(link.id);
        }
    }
}

impl Engine {
    /// Resume from a validated, persisted state. Storage IDs are never RNG inputs.
    pub fn new(scenario: ValidatedScenario, state: State) -> Result<Self, EngineError> {
        Self::new_with_clock(scenario, state, Arc::new(MonotonicClock::default()))
    }

    pub fn new_with_clock(
        scenario: ValidatedScenario,
        state: State,
        clock: Arc<dyn MetricsClock>,
    ) -> Result<Self, EngineError> {
        scenario
            .validate_state(&state)
            .map_err(EngineError::InvalidState)?;
        Ok(Self {
            scenario,
            state,
            clock,
            last_metrics: None,
            route_cache: migration::RouteCache::default(),
            rebase_culture_pending: false,
        })
    }

    /// Build the initial state, materialize founder genotypes, and generate configured households.
    pub fn from_scenario(scenario: ValidatedScenario) -> Result<Self, EngineError> {
        let state = generate_initial_state(&scenario)?;
        Ok(Self {
            scenario,
            state,
            clock: Arc::new(MonotonicClock::default()),
            last_metrics: None,
            route_cache: migration::RouteCache::default(),
            rebase_culture_pending: false,
        })
    }

    /// Resume a child branch at its inclusive cut while applying a revised ruleset only to
    /// subsequent annual closes. The parent revision must validate the cut before this call.
    pub fn new_for_branch_revision(
        scenario: ValidatedScenario,
        state: State,
    ) -> Result<Self, EngineError> {
        state.validate().map_err(EngineError::InvalidState)?;
        if state.world_id != scenario.scenario().world_id
            || state.rules_version != scenario.scenario().rules_version
            || state.seed != scenario.scenario().seed
            || state.start_year != scenario.scenario().start_year
        {
            return Err(EngineError::InvalidState(vec![ValidationIssue {
                path: "state".into(),
                code: "scenario_mismatch".into(),
                message: "branch cut state identity must match the revised scenario".into(),
            }]));
        }
        if state.rules_version < chargen_domain::Scenario::TERRITORIAL_RULES_VERSION
            || state.rules_version > chargen_domain::Scenario::EVENT_RULES_VERSION
        {
            return Err(EngineError::InvalidMigration(
                "policy revision constructor is only valid for rules versions 2 and 3".into(),
            ));
        }
        Ok(Self {
            scenario,
            state,
            clock: Arc::new(MonotonicClock::default()),
            last_metrics: None,
            route_cache: migration::RouteCache::default(),
            rebase_culture_pending: true,
        })
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn last_metrics(&self) -> Option<&EngineMetrics> {
        self.last_metrics.as_ref()
    }

    pub fn rng_algorithm(&self) -> &'static str {
        RNG_ALGORITHM
    }

    /// Advances one complete year. A cancelled or invalid pending year never replaces `self.state`.
    pub fn advance_year(
        &mut self,
        cancellation: Option<&CancellationToken>,
    ) -> Result<YearCommit, EngineError> {
        self.advance_year_with_commands(cancellation, &[])
    }

    /// Advances one year with additional A1 displacement commands. Commands are validated as a
    /// complete batch before the pending annual state is created.
    pub fn advance_year_with_commands(
        &mut self,
        cancellation: Option<&CancellationToken>,
        commands: &[chargen_domain::ForcedDisplacementCommand],
    ) -> Result<YearCommit, EngineError> {
        ensure_not_cancelled(cancellation)?;
        let year = match self.state.current_year {
            Some(last) => last.checked_add(1).ok_or(EngineError::YearOverflow)?,
            None => self.state.start_year,
        };
        if self.scenario.scenario().rules_version
            < chargen_domain::Scenario::TERRITORIAL_RULES_VERSION
            && !commands.is_empty()
        {
            return Err(EngineError::InvalidMigration(
                "forced displacement commands require rules version 2".into(),
            ));
        }
        let (forced_moves, mut movement_usage) =
            if let Some(territory) = &self.scenario.scenario().territory {
                migration::prepare_forced_displacements(
                    &self.scenario,
                    &self.state,
                    territory,
                    year,
                    commands,
                    &mut self.route_cache,
                )?
            } else {
                (Vec::new(), migration::MigrationUsage::default())
            };
        let total_started = self.clock.now_nanos();
        let mut pending = self.state.clone();
        let mut events = Vec::new();
        let mut phases = Vec::with_capacity(YearPhase::ALL.len());
        let mut phase_metrics = Vec::with_capacity(YearPhase::ALL.len());
        let before_census = census_at_start(&self.state, year);
        let policies = &self.scenario.scenario().policies;
        let mut indexes =
            AnnualIndexes::rebuild(&pending, year, &policies.adoption, &policies.births);

        for phase in YearPhase::ALL {
            ensure_not_cancelled(cancellation)?;
            let phase_started = self.clock.now_nanos();
            let record = match phase {
                YearPhase::A0 => {
                    if self.rebase_culture_pending {
                        if let Some(territory) = &self.scenario.scenario().territory {
                            reconcile_territorial_professions(
                                territory,
                                &mut pending,
                                year,
                                &mut events,
                            );
                        }
                        if let Some(culture_config) = &self.scenario.scenario().culture {
                            let current_year =
                                self.state.current_year.unwrap_or(self.state.start_year);
                            culture::reconcile_culture_config(
                                culture_config,
                                &mut pending,
                                current_year,
                            )
                            .map_err(|error| EngineError::Culture(error.to_string()))?;
                        }
                    }
                    if let (Some(culture), Some(previous_year)) =
                        (&self.scenario.scenario().culture, self.state.current_year)
                    {
                        culture::apply_culture_age_transitions(
                            culture,
                            &mut pending,
                            previous_year,
                            year,
                        )
                        .map_err(|error| EngineError::Culture(error.to_string()))?;
                    }
                    events::expire_event_effects(&mut pending, year);
                    if let Some(economic_config) = &self.scenario.scenario().economy {
                        economy::begin_economy_year(
                            economic_config,
                            &mut pending,
                            self.scenario.scenario().territory.as_ref(),
                            year,
                            &mut events,
                        )
                        .map_err(|error| EngineError::Economy(error.to_string()))?;
                    }
                    match self.scenario.scenario().rules_version {
                        chargen_domain::Scenario::EVENT_RULES_VERSION => implemented(
                            phase,
                            "efectos vencidos, mercado pendiente y transiciones culturales",
                        ),
                        chargen_domain::Scenario::TERRITORIAL_RULES_VERSION => {
                            implemented(phase, "transiciones anuales de elegibilidad cultural")
                        }
                        _ => deferred(phase, "sin comandos de año configurados en este hito"),
                    }
                }
                YearPhase::A1 => {
                    if let Some(territory) = &self.scenario.scenario().territory {
                        migration::apply_forced_displacements(
                            &self.scenario,
                            &mut pending,
                            territory,
                            year,
                            &forced_moves,
                            &mut events,
                        )?;
                        if !forced_moves.is_empty() {
                            indexes = AnnualIndexes::rebuild(
                                &pending,
                                year,
                                &policies.adoption,
                                &policies.births,
                            );
                        }
                        if pending.rules_version == chargen_domain::Scenario::EVENT_RULES_VERSION {
                            events::apply_scheduled_events(
                                &self.scenario,
                                &mut pending,
                                &mut indexes,
                                year,
                                &mut events,
                                &mut movement_usage,
                                &mut self.route_cache,
                            )?;
                            indexes = AnnualIndexes::rebuild(
                                &pending,
                                year,
                                &policies.adoption,
                                &policies.births,
                            );
                            implemented(
                                phase,
                                "desplazamientos y shocks programados resueltos en orden estable",
                            )
                        } else {
                            implemented(
                                phase,
                                "desplazamientos forzados validados y aplicados en lote",
                            )
                        }
                    } else {
                        deferred(phase, "agenda de eventos programados pendiente")
                    }
                }
                YearPhase::A2 => {
                    apply_ordinary_mortality(
                        &self.scenario,
                        &mut pending,
                        &mut indexes,
                        year,
                        &mut events,
                        self.scenario.scenario().economy.as_ref(),
                    )?;
                    indexes = AnnualIndexes::rebuild(
                        &pending,
                        year,
                        &policies.adoption,
                        &policies.births,
                    );
                    implemented(phase, "mortalidad ordinaria y viudedad")
                }
                YearPhase::A3 => {
                    if pending.rules_version >= chargen_domain::Scenario::TERRITORIAL_RULES_VERSION
                    {
                        reconcile_territorial_state(&mut pending, year, &mut events)?;
                        if let Some(economic_config) = &self.scenario.scenario().economy {
                            economy::reconcile_economic_groups(economic_config, &mut pending, year)
                                .map_err(|error| EngineError::Economy(error.to_string()))?;
                        }
                        implemented(
                            phase,
                            "hogares, ocupación y residencias reconciliados tras mortalidad",
                        )
                    } else {
                        deferred(
                            phase,
                            "reconciliación agregada de hogares y empleo pendiente",
                        )
                    }
                }
                YearPhase::A4 => {
                    if let Some(economic_config) = &self.scenario.scenario().economy {
                        let modifiers = events::production_modifiers(&pending, year);
                        economy::produce_economy(economic_config, &mut pending, year, &modifiers)
                            .map_err(|error| EngineError::Economy(error.to_string()))?;
                        implemented(
                            phase,
                            "producción agrupada con pérdidas e incapacidades activas",
                        )
                    } else {
                        deferred(phase, "producción pendiente")
                    }
                }
                YearPhase::A5 => {
                    if let (Some(economic_config), Some(territory)) = (
                        &self.scenario.scenario().economy,
                        &self.scenario.scenario().territory,
                    ) {
                        economy::trade_economy(
                            economic_config,
                            &mut pending,
                            Some(territory),
                            year,
                        )
                        .map_err(|error| EngineError::Economy(error.to_string()))?;
                        implemented(phase, "mercados locales y comercio por rutas")
                    } else {
                        deferred(phase, "mercados pendientes")
                    }
                }
                YearPhase::A6 => {
                    if let (Some(economic_config), Some(territory)) = (
                        &self.scenario.scenario().economy,
                        &self.scenario.scenario().territory,
                    ) {
                        economy::consume_and_adjust_economy(
                            economic_config,
                            &mut pending,
                            Some(territory),
                            year,
                        )
                        .map_err(|error| EngineError::Economy(error.to_string()))?;
                        implemented(phase, "consumo, carencias y ajustes del siguiente año")
                    } else {
                        deferred(phase, "consumo y necesidades pendientes")
                    }
                }
                YearPhase::A7 => {
                    if let Some(territory) = &self.scenario.scenario().territory {
                        let mut adjusted_territory = territory.clone();
                        if let Some(economic_config) = &self.scenario.scenario().economy {
                            for settlement_id in pending.settlements.keys().copied() {
                                let adjustment = economy::current_migration_scarcity_adjustment(
                                    economic_config,
                                    &pending,
                                    settlement_id,
                                );
                                let opportunity = adjusted_territory
                                    .opportunity_scores
                                    .entry(settlement_id)
                                    .or_default();
                                *opportunity = opportunity.saturating_add(adjustment);
                            }
                        }
                        migration::apply_voluntary_migration(
                            &self.scenario,
                            &mut pending,
                            &adjusted_territory,
                            year,
                            &mut self.route_cache,
                            &mut movement_usage,
                            &mut events,
                        )?;
                        indexes = AnnualIndexes::rebuild(
                            &pending,
                            year,
                            &policies.adoption,
                            &policies.births,
                        );
                        if let Some(economic_config) = &self.scenario.scenario().economy {
                            economy::reconcile_economic_groups(economic_config, &mut pending, year)
                                .map_err(|error| EngineError::Economy(error.to_string()))?;
                        }
                        if territory.migration.enabled {
                            implemented(phase, "propuestas voluntarias resueltas en lote estable")
                        } else {
                            implemented(phase, "no hay política de migración voluntaria activa")
                        }
                    } else {
                        deferred(phase, "migración pendiente")
                    }
                }
                YearPhase::A8 => {
                    apply_union_and_care_policies(
                        &self.scenario,
                        &mut pending,
                        &mut indexes,
                        year,
                        &mut events,
                    )?;
                    implemented(phase, "uniones, separaciones, cuidado y adopción")
                }
                YearPhase::A9 => {
                    apply_births(
                        &self.scenario,
                        &mut pending,
                        &mut indexes,
                        year,
                        &mut events,
                        self.scenario.scenario().economy.as_ref(),
                    )?;
                    if let Some(culture_config) = &self.scenario.scenario().culture {
                        let birth_inputs: Vec<_> = events
                            .iter()
                            .filter_map(|event| match &event.kind {
                                HistoricalEventKind::PersonBorn {
                                    person_id,
                                    genetic_parents,
                                    ..
                                } => pending.persons.get(person_id).map(|person| {
                                    culture::CultureBirthInput {
                                        person_id: *person_id,
                                        logical_id: person.logical_id,
                                        settlement_id: person.settlement_id,
                                        genetic_parents: genetic_parents.clone(),
                                    }
                                }),
                                _ => None,
                            })
                            .collect();
                        let assignments = culture::prepare_birth_culture_batch(
                            culture_config,
                            &pending,
                            year,
                            &birth_inputs,
                            |logical_id, trait_key, variation_max| {
                                let span = u64::from(variation_max) * 2 + 1;
                                let draw = random_below(
                                    pending.seed,
                                    pending.rules_version,
                                    DecisionDomain::CultureVariation,
                                    year,
                                    logical_id,
                                    stable_trait_ordinal(trait_key),
                                    span,
                                ) as i32;
                                (draw - i32::from(variation_max)) as i16
                            },
                        )
                        .map_err(|error| EngineError::Culture(error.to_string()))?;
                        culture::apply_birth_culture_batch(
                            culture_config,
                            &mut pending,
                            year,
                            &assignments,
                        )
                        .map_err(|error| EngineError::Culture(error.to_string()))?;
                    }
                    if let Some(economic_config) = &self.scenario.scenario().economy {
                        economy::reconcile_economic_groups(economic_config, &mut pending, year)
                            .map_err(|error| EngineError::Economy(error.to_string()))?;
                    }
                    implemented(
                        phase,
                        "nacimientos, alelos heredados y mortalidad infantil configurada",
                    )
                }
                YearPhase::A10 => {
                    if let Some(culture_config) = &self.scenario.scenario().culture {
                        let seed = pending.seed;
                        let rules_version = pending.rules_version;
                        culture::apply_a10_culture(
                            culture_config,
                            &mut pending,
                            year,
                            seed,
                            rules_version,
                            &mut events,
                        )
                        .map_err(|error| EngineError::Culture(error.to_string()))?;
                        implemented(phase, "afiliaciones e instituciones procesadas en lote")
                    } else {
                        deferred(phase, "organizaciones e instituciones pendientes")
                    }
                }
                YearPhase::A11 => {
                    if pending.rules_version == chargen_domain::Scenario::EVENT_RULES_VERSION {
                        for event in &mut events {
                            event
                                .affected_entities
                                .push(EntityRef::World(pending.world_id));
                            event.affected_entities.sort();
                            event.affected_entities.dedup();
                        }
                    }
                    if let Some(economic_config) = &self.scenario.scenario().economy {
                        economy::reconcile_economic_groups(economic_config, &mut pending, year)
                            .map_err(|error| EngineError::Economy(error.to_string()))?;
                        let report =
                            economy::finalize_economy_year(economic_config, &mut pending, year)
                                .map_err(|error| EngineError::Economy(error.to_string()))?;
                        let summary = chargen_domain::EconomicYearClosedSummary::from(&report);
                        let mut affected_entities = vec![EntityRef::World(pending.world_id)];
                        affected_entities.extend(
                            summary
                                .settlement_ids
                                .iter()
                                .copied()
                                .map(EntityRef::Settlement),
                        );
                        affected_entities.extend(
                            summary
                                .group_ids
                                .iter()
                                .cloned()
                                .map(EntityRef::EconomicGroup),
                        );
                        affected_entities.sort();
                        affected_entities.dedup();
                        emit_event(
                            &pending,
                            &mut events,
                            year,
                            LogicalId(pending.world_id.0),
                            affected_entities,
                            HistoricalEventKind::EconomicYearClosed { summary },
                        );
                    }
                    pending.current_year = Some(year);
                    pending.last_year_phases = phases
                        .iter()
                        .cloned()
                        .chain(std::iter::once(implemented(
                            phase,
                            "validación de cierre anual",
                        )))
                        .collect();
                    if let Some(culture_config) = &self.scenario.scenario().culture {
                        culture::validate_culture_state(culture_config, &pending, year)
                            .map_err(|error| EngineError::Culture(error.to_string()))?;
                    }
                    validate_census_balance(&pending, year, &before_census, &events)?;
                    if pending.rules_version >= chargen_domain::Scenario::TERRITORIAL_RULES_VERSION
                    {
                        pending.last_year_settlement_balances =
                            census_balance(&pending, year, &before_census, &events)
                                .into_iter()
                                .map(|balance| {
                                    (
                                        balance.settlement_id,
                                        SettlementYearBalance {
                                            settlement_id: balance.settlement_id,
                                            before: balance.before,
                                            births: balance.births,
                                            deaths: balance.deaths,
                                            incoming: balance.incoming,
                                            outgoing: balance.outgoing,
                                            after: balance.after,
                                        },
                                    )
                                })
                                .collect();
                    }
                    self.scenario
                        .validate_state(&pending)
                        .map_err(EngineError::InvalidState)?;
                    implemented(phase, "validación de cierre anual")
                }
            };
            phases.push(record);
            phase_metrics.push(PhaseMetric {
                phase,
                duration_nanos: self.clock.now_nanos().saturating_sub(phase_started),
            });
        }

        ensure_not_cancelled(cancellation)?;
        pending.current_year = Some(year);
        pending.last_year_phases = phases;
        let census_balance = census_balance(&pending, year, &before_census, &events);
        let total_duration_nanos = self.clock.now_nanos().saturating_sub(total_started);
        self.state = pending.clone();
        self.rebase_culture_pending = false;
        self.last_metrics = Some(EngineMetrics {
            year,
            phases: phase_metrics,
            total_duration_nanos,
            census_balance,
        });
        Ok(YearCommit {
            year,
            state: pending,
            events,
        })
    }
}

/// Public deterministic scenario initializer for demos, benchmarks, and application setup.
pub fn generate_initial_state(scenario: &ValidatedScenario) -> Result<State, EngineError> {
    let mut state = scenario.initial_state();
    generate_initial_population(scenario, &mut state)?;
    materialize_founder_genomes(scenario, &mut state)?;
    if scenario.scenario().rules_version >= chargen_domain::Scenario::TERRITORIAL_RULES_VERSION {
        let initial_residents: Vec<_> = state
            .living_people_at(scenario.scenario().start_year)
            .map(|person| (person.id, person.settlement_id, person.profession.clone()))
            .collect();
        for (person_id, settlement_id, profession_id) in initial_residents {
            state.residences.entry(person_id).or_insert_with(|| {
                vec![ResidenceInterval {
                    settlement_id,
                    validity: YearInterval {
                        start: scenario.scenario().start_year,
                        end_exclusive: None,
                    },
                }]
            });
            if let Some(profession_id) = profession_id {
                state
                    .profession_intervals
                    .entry(person_id)
                    .or_insert_with(|| {
                        vec![chargen_domain::ProfessionInterval {
                            profession_id,
                            validity: YearInterval {
                                start: scenario.scenario().start_year,
                                end_exclusive: None,
                            },
                        }]
                    });
            }
        }
        if let Some(culture_config) = &scenario.scenario().culture {
            culture::initialize_culture(culture_config, &mut state, scenario.scenario().start_year)
                .map_err(|error| EngineError::Culture(error.to_string()))?;
        }
    }
    if let Some(economic_config) = &scenario.scenario().economy {
        economy::initialize_economy(economic_config, &mut state, scenario.scenario().start_year)
            .map_err(|error| EngineError::Economy(error.to_string()))?;
    }
    scenario
        .validate_state(&state)
        .map_err(EngineError::InvalidState)?;
    Ok(state)
}

fn ensure_not_cancelled(token: Option<&CancellationToken>) -> Result<(), EngineError> {
    if token.is_some_and(CancellationToken::is_cancelled) {
        Err(EngineError::Cancelled)
    } else {
        Ok(())
    }
}

fn implemented(phase: YearPhase, description: &str) -> PhaseRecord {
    PhaseRecord {
        phase,
        disposition: PhaseDisposition::Implemented,
        description: description.to_owned(),
    }
}

fn deferred(phase: YearPhase, description: &str) -> PhaseRecord {
    PhaseRecord {
        phase,
        disposition: PhaseDisposition::DeferredNoop,
        description: description.to_owned(),
    }
}

fn census_at_start(state: &State, year: Year) -> BTreeMap<SettlementId, u64> {
    let mut counts: BTreeMap<_, _> = state
        .settlements
        .keys()
        .copied()
        .map(|id| (id, 0))
        .collect();
    for person in state
        .persons
        .values()
        .filter(|person| person.alive_at_start(year))
    {
        *counts.entry(person.settlement_id).or_default() += 1;
    }
    counts
}

fn census_balance(
    state: &State,
    year: Year,
    before: &BTreeMap<SettlementId, u64>,
    events: &[HistoricalEvent],
) -> Vec<DemographicBalance> {
    let mut births: BTreeMap<SettlementId, u64> = BTreeMap::new();
    let mut deaths: BTreeMap<SettlementId, u64> = BTreeMap::new();
    let mut incoming: BTreeMap<SettlementId, u64> = BTreeMap::new();
    let mut outgoing: BTreeMap<SettlementId, u64> = BTreeMap::new();
    for event in events {
        match &event.kind {
            HistoricalEventKind::PersonBorn { person_id, .. } => {
                if let Some(person) = state.persons.get(person_id) {
                    *births.entry(person.settlement_id).or_default() += 1;
                }
            }
            HistoricalEventKind::PersonDied { person_id, .. } => {
                if let Some(person) = state.persons.get(person_id) {
                    *deaths.entry(person.settlement_id).or_default() += 1;
                }
            }
            HistoricalEventKind::ResidentsMoved {
                person_ids,
                from,
                to,
                ..
            } => {
                let count = person_ids.len() as u64;
                *outgoing.entry(*from).or_default() += count;
                *incoming.entry(*to).or_default() += count;
            }
            _ => {}
        }
    }
    let mut after: BTreeMap<_, _> = state
        .settlements
        .keys()
        .copied()
        .map(|id| (id, 0))
        .collect();
    for person in state.living_people_at(year) {
        *after.entry(person.settlement_id).or_default() += 1;
    }
    state
        .settlements
        .keys()
        .copied()
        .map(|settlement_id| DemographicBalance {
            settlement_id,
            before: before.get(&settlement_id).copied().unwrap_or(0),
            births: births.get(&settlement_id).copied().unwrap_or(0),
            deaths: deaths.get(&settlement_id).copied().unwrap_or(0),
            incoming: incoming.get(&settlement_id).copied().unwrap_or(0),
            outgoing: outgoing.get(&settlement_id).copied().unwrap_or(0),
            after: after.get(&settlement_id).copied().unwrap_or(0),
        })
        .collect()
}

fn validate_census_balance(
    state: &State,
    year: Year,
    before: &BTreeMap<SettlementId, u64>,
    events: &[HistoricalEvent],
) -> Result<(), EngineError> {
    let balances = census_balance(state, year, before, events);
    let mut global_before = 0_u64;
    let mut global_births = 0_u64;
    let mut global_deaths = 0_u64;
    let mut global_after = 0_u64;
    let mut global_incoming = 0_u64;
    let mut global_outgoing = 0_u64;
    for balance in balances {
        let expected = balance
            .before
            .checked_add(balance.births)
            .and_then(|total| total.checked_sub(balance.deaths))
            .and_then(|total| total.checked_add(balance.incoming))
            .and_then(|total| total.checked_sub(balance.outgoing));
        if expected != Some(balance.after) {
            return Err(EngineError::DemographicBalance {
                settlement_id: balance.settlement_id,
                expected: expected.unwrap_or(u64::MAX),
                actual: balance.after,
            });
        }
        global_before = global_before.checked_add(balance.before).ok_or(
            EngineError::GlobalDemographicBalance {
                expected: u64::MAX,
                actual: u64::MAX,
            },
        )?;
        global_births = global_births.checked_add(balance.births).ok_or(
            EngineError::GlobalDemographicBalance {
                expected: u64::MAX,
                actual: u64::MAX,
            },
        )?;
        global_deaths = global_deaths.checked_add(balance.deaths).ok_or(
            EngineError::GlobalDemographicBalance {
                expected: u64::MAX,
                actual: u64::MAX,
            },
        )?;
        global_after = global_after.checked_add(balance.after).ok_or(
            EngineError::GlobalDemographicBalance {
                expected: u64::MAX,
                actual: u64::MAX,
            },
        )?;
        global_incoming = global_incoming.checked_add(balance.incoming).ok_or(
            EngineError::GlobalDemographicBalance {
                expected: u64::MAX,
                actual: u64::MAX,
            },
        )?;
        global_outgoing = global_outgoing.checked_add(balance.outgoing).ok_or(
            EngineError::GlobalDemographicBalance {
                expected: u64::MAX,
                actual: u64::MAX,
            },
        )?;
    }
    let expected_global = global_before
        .checked_add(global_births)
        .and_then(|total| total.checked_sub(global_deaths))
        .and_then(|total| total.checked_add(global_incoming))
        .and_then(|total| total.checked_sub(global_outgoing));
    if expected_global != Some(global_after) {
        return Err(EngineError::GlobalDemographicBalance {
            expected: expected_global.unwrap_or(u64::MAX),
            actual: global_after,
        });
    }
    Ok(())
}

fn reconcile_territorial_state(
    state: &mut State,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    let active_households: Vec<_> = state
        .households
        .values()
        .filter(|household| household.closed_year.is_none())
        .map(|household| household.id)
        .collect();
    for household_id in active_households {
        let household = state.households[&household_id].clone();
        let deceased_members: Vec<_> = household
            .members
            .iter()
            .copied()
            .filter(|person_id| !state.alive_at_close(*person_id, year))
            .collect();
        for person_id in deceased_members {
            state
                .households
                .get_mut(&household_id)
                .expect("active household exists")
                .members
                .remove(&person_id);
            let detached_logical_id = state.persons.get_mut(&person_id).and_then(|person| {
                if person.household_id == Some(household_id) {
                    person.household_id = None;
                    Some(person.logical_id)
                } else {
                    None
                }
            });
            if let Some(logical_id) = detached_logical_id {
                emit_event(
                    state,
                    events,
                    year,
                    logical_id,
                    vec![
                        EntityRef::Person(person_id),
                        EntityRef::Household(household_id),
                    ],
                    HistoricalEventKind::HouseholdChanged {
                        person_id,
                        from: Some(household_id),
                        to: None,
                    },
                );
            }
        }
        let (members, settlement_id) = {
            let household = state
                .households
                .get(&household_id)
                .expect("household exists");
            (household.members.clone(), household.settlement_id)
        };
        if members.is_empty() {
            state
                .households
                .get_mut(&household_id)
                .expect("household exists")
                .closed_year = Some(year);
            continue;
        }
        for person_id in &members {
            let person = state.persons.get(person_id).ok_or_else(|| {
                EngineError::InvalidPopulation("household has missing member".into())
            })?;
            if person.household_id != Some(household_id) || person.settlement_id != settlement_id {
                return Err(EngineError::InvalidPopulation(
                    "household membership or residence is inconsistent after mortality".into(),
                ));
            }
        }
    }
    for person in state.living_people_at(year) {
        let active_residence_count = state
            .residences
            .get(&person.id)
            .into_iter()
            .flatten()
            .filter(|interval| interval.validity.contains(year))
            .count();
        if active_residence_count != 1
            || state
                .residences
                .get(&person.id)
                .into_iter()
                .flatten()
                .find(|interval| interval.validity.contains(year))
                .is_none_or(|interval| interval.settlement_id != person.settlement_id)
        {
            return Err(EngineError::InvalidPopulation(
                "living resident has inconsistent territorial history".into(),
            ));
        }
    }
    Ok(())
}

fn reconcile_territorial_professions(
    territory: &chargen_domain::TerritoryConfig,
    state: &mut State,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
) {
    let unavailable: Vec<_> = state
        .living_people_at(year)
        .filter_map(|person| {
            let profession = person.profession.as_ref()?;
            let offered = territory
                .profession_options_by_settlement
                .get(&person.settlement_id)
                .is_some_and(|options| options.contains(profession));
            let defined = territory
                .professions
                .iter()
                .any(|definition| &definition.id == profession);
            (!offered || !defined).then_some((
                person.id,
                person.logical_id,
                person.settlement_id,
                profession.clone(),
            ))
        })
        .collect();
    for (person_id, logical_id, settlement_id, profession_id) in unavailable {
        if let Some(intervals) = state.profession_intervals.get_mut(&person_id) {
            if let Some(active) = intervals
                .iter_mut()
                .rev()
                .find(|interval| interval.validity.end_exclusive.is_none())
            {
                active.validity.end_exclusive = Some(year);
            }
        }
        state
            .persons
            .get_mut(&person_id)
            .expect("living person was collected")
            .profession = None;
        emit_event(
            state,
            events,
            year,
            logical_id,
            vec![
                EntityRef::World(state.world_id),
                EntityRef::Person(person_id),
                EntityRef::Settlement(settlement_id),
            ],
            HistoricalEventKind::ProfessionChanged {
                person_id,
                from: Some(profession_id),
                to: None,
                reason: chargen_domain::ProfessionChangeReason::DestinationDoesNotOfferProfession,
            },
        );
    }
}

/// Derives a portable pseudorandom word from stable logical inputs. No branch/storage ID is included.
pub fn keyed_random_u64(
    seed: u64,
    rules_version: u32,
    domain: DecisionDomain,
    year: Year,
    logical_id: LogicalId,
    ordinal: u64,
) -> u64 {
    let mut value = seed ^ 0x4348_4152_4745_4e01;
    for word in [
        u64::from(RNG_VERSION),
        u64::from(rules_version),
        domain as u64,
        year.0 as u64,
        logical_id.0,
        ordinal,
    ] {
        value = splitmix64(value ^ splitmix64(word.wrapping_add(0x9e37_79b9_7f4a_7c15)));
    }
    splitmix64(value)
}

fn stable_trait_ordinal(key: &TraitKey) -> u64 {
    format!("{}::{}", key.namespace, key.name)
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub(crate) fn random_below(
    seed: u64,
    rules: u32,
    domain: DecisionDomain,
    year: Year,
    logical: LogicalId,
    ordinal: u64,
    bound: u64,
) -> u64 {
    if bound <= 1 {
        return 0;
    }
    let threshold = bound.wrapping_neg() % bound;
    let mut attempt = 0_u64;
    loop {
        let raw = keyed_random_u64(
            seed,
            rules,
            domain,
            year,
            logical,
            ordinal.wrapping_add(attempt),
        );
        if raw >= threshold {
            return raw % bound;
        }
        attempt = attempt.wrapping_add(1);
    }
}

pub(crate) fn occurs(
    seed: u64,
    rules: u32,
    domain: DecisionDomain,
    year: Year,
    logical: LogicalId,
    ordinal: u64,
    probability_ppm: u32,
) -> bool {
    probability_ppm >= 1_000_000
        || (probability_ppm > 0
            && random_below(seed, rules, domain, year, logical, ordinal, 1_000_000)
                < u64::from(probability_ppm))
}

// Keep the versioned RNG inputs explicit to make decision keys auditable.
#[allow(clippy::too_many_arguments)]
fn choose_weighted<T>(
    entries: &[T],
    weight: impl Fn(&T) -> u32,
    seed: u64,
    rules: u32,
    domain: DecisionDomain,
    year: Year,
    logical: LogicalId,
    ordinal: u64,
) -> Option<&T> {
    let total = entries
        .iter()
        .map(|entry| u64::from(weight(entry)))
        .sum::<u64>();
    if total == 0 {
        return None;
    }
    let mut draw = random_below(seed, rules, domain, year, logical, ordinal, total);
    for entry in entries {
        let item_weight = u64::from(weight(entry));
        if draw < item_weight {
            return Some(entry);
        }
        draw -= item_weight;
    }
    None
}

pub(crate) fn derive_logical_id(
    seed: u64,
    rules: u32,
    domain: DecisionDomain,
    year: Year,
    parent: LogicalId,
    ordinal: u64,
) -> LogicalId {
    LogicalId(keyed_random_u64(seed, rules, domain, year, parent, ordinal))
}

fn generate_initial_population(
    scenario: &ValidatedScenario,
    state: &mut State,
) -> Result<(), EngineError> {
    let config = scenario.scenario();
    for (profile_index, profile) in config.initial_population_profiles.iter().enumerate() {
        let place = state
            .settlements
            .get(&profile.settlement_id)
            .ok_or_else(|| {
                EngineError::InvalidPopulation("profile references a missing settlement".into())
            })?;
        let place_logical = place.logical_id;
        let mut remaining = profile.population_size;
        let mut household_ordinal = 0_u64;
        let mut person_ordinal = 0_u64;
        while remaining > 0 {
            let room =
                u64::from(profile.household_size_maximum - profile.household_size_minimum) + 1;
            let chosen_size = u64::from(profile.household_size_minimum)
                + random_below(
                    config.seed,
                    config.rules_version,
                    DecisionDomain::HouseholdIdentity,
                    config.start_year,
                    place_logical,
                    (profile_index as u64)
                        .wrapping_mul(1_000_000)
                        .wrapping_add(household_ordinal),
                    room,
                );
            let members = remaining.min(chosen_size as u32);
            let household_id = allocate_household_id(state)?;
            let household_logical = derive_logical_id(
                config.seed,
                config.rules_version,
                DecisionDomain::HouseholdIdentity,
                config.start_year,
                place_logical,
                (profile_index as u64)
                    .wrapping_mul(1_000_000)
                    .wrapping_add(household_ordinal),
            );
            let mut household = Household {
                id: household_id,
                logical_id: household_logical,
                kind: profile.household_kind.clone(),
                settlement_id: profile.settlement_id,
                members: BTreeSet::new(),
                closed_year: None,
            };
            for _ in 0..members {
                let person_logical = derive_logical_id(
                    config.seed,
                    config.rules_version,
                    DecisionDomain::PersonIdentity,
                    config.start_year,
                    place_logical,
                    (profile_index as u64)
                        .wrapping_mul(1_000_000)
                        .wrapping_add(person_ordinal),
                );
                let age_band = choose_weighted(
                    &profile.age_bands,
                    |band| band.weight,
                    config.seed,
                    config.rules_version,
                    DecisionDomain::InitialAge,
                    config.start_year,
                    person_logical,
                    0,
                )
                .ok_or_else(|| {
                    EngineError::InvalidPopulation("age distribution has zero total weight".into())
                })?;
                let age = u32::from(age_band.minimum_age)
                    + random_below(
                        config.seed,
                        config.rules_version,
                        DecisionDomain::InitialAge,
                        config.start_year,
                        person_logical,
                        1,
                        u64::from(age_band.maximum_age - age_band.minimum_age) + 1,
                    ) as u32;
                let birth_year = config
                    .start_year
                    .0
                    .checked_sub(i32::try_from(age).map_err(|_| EngineError::YearOverflow)?)
                    .ok_or(EngineError::YearOverflow)?;
                let reproductive = choose_weighted(
                    &profile.reproductive_profiles,
                    |entry| entry.weight,
                    config.seed,
                    config.rules_version,
                    DecisionDomain::ReproductiveProfile,
                    config.start_year,
                    person_logical,
                    0,
                )
                .map(|entry| entry.profile.clone())
                .unwrap_or_default();
                let mut traits = BTreeMap::new();
                for (trait_ordinal, (trait_key, values)) in
                    profile.trait_distributions.iter().enumerate()
                {
                    if let Some(selected) = choose_weighted(
                        values,
                        |entry| entry.weight,
                        config.seed,
                        config.rules_version,
                        DecisionDomain::InitialTrait,
                        config.start_year,
                        person_logical,
                        trait_ordinal as u64,
                    ) {
                        traits.insert(trait_key.clone(), selected.value);
                    }
                }
                let id = allocate_person_id(state)?;
                let profession =
                    if config.rules_version == chargen_domain::Scenario::EVENT_RULES_VERSION {
                        choose_weighted(
                            &profile.profession_distribution,
                            |entry| entry.weight,
                            config.seed,
                            config.rules_version,
                            DecisionDomain::InitialProfession,
                            config.start_year,
                            person_logical,
                            0,
                        )
                        .and_then(|entry| entry.profession_id.clone())
                    } else {
                        None
                    };
                let person = Person {
                    id,
                    logical_id: person_logical,
                    birth_year: Year(birth_year),
                    death_year: None,
                    settlement_id: profile.settlement_id,
                    household_id: Some(household_id),
                    reproductive,
                    traits,
                    genome: Default::default(),
                    last_widowed_year: None,
                    profession,
                };
                household.members.insert(id);
                state.persons.insert(id, person);
                person_ordinal += 1;
            }
            state.households.insert(household_id, household);
            remaining -= members;
            household_ordinal += 1;
        }
    }
    Ok(())
}

fn materialize_founder_genomes(
    scenario: &ValidatedScenario,
    state: &mut State,
) -> Result<(), EngineError> {
    let config = scenario.scenario();
    for person in state.persons.values_mut() {
        for (locus_index, locus) in config.genetics.loci.iter().enumerate() {
            if person.genome.loci.contains_key(&locus.id) {
                continue;
            }
            let alleles = [
                founder_allele(
                    &config.genetics,
                    locus_index,
                    person.logical_id,
                    0,
                    config.seed,
                    config.rules_version,
                    config.start_year,
                )?,
                founder_allele(
                    &config.genetics,
                    locus_index,
                    person.logical_id,
                    1,
                    config.seed,
                    config.rules_version,
                    config.start_year,
                )?,
            ];
            person.genome.loci.insert(locus.id.clone(), alleles);
        }
    }
    Ok(())
}

fn founder_allele(
    catalog: &GeneticCatalog,
    locus_index: usize,
    logical: LogicalId,
    chromosome: u64,
    seed: u64,
    rules: u32,
    year: Year,
) -> Result<Allele, EngineError> {
    let locus = catalog
        .loci
        .get(locus_index)
        .ok_or_else(|| EngineError::InvalidPopulation("missing genetic locus".into()))?;
    let chosen = choose_weighted(
        &locus.founder_distribution,
        |entry| entry.weight,
        seed,
        rules,
        DecisionDomain::FounderGenome,
        year,
        logical,
        (locus_index as u64)
            .wrapping_mul(2)
            .wrapping_add(chromosome),
    )
    .ok_or_else(|| {
        EngineError::InvalidPopulation(format!(
            "locus {} has an empty founder distribution",
            locus.id.0
        ))
    })?;
    Ok(Allele {
        variant: chosen.variant.clone(),
        lineage: None,
        inherited_from: None,
    })
}

fn apply_ordinary_mortality(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
    economic_config: Option<&chargen_domain::EconomicConfig>,
) -> Result<(), EngineError> {
    let seed = scenario.scenario().seed;
    let rules = scenario.scenario().rules_version;
    let mut candidates: Vec<_> = state
        .persons
        .values()
        .filter(|person| person.alive_at_start(year) && person.death_year.is_none())
        .map(|person| (person.logical_id, person.id))
        .collect();
    candidates.sort_unstable();
    for (logical, person_id) in candidates {
        let person = &state.persons[&person_id];
        let age = year.age_of(person.birth_year).unwrap_or(0);
        let base_probability = mortality_probability(
            &scenario.scenario().policies.mortality.age_bands,
            age,
            scenario
                .scenario()
                .policies
                .mortality
                .default_probability_ppm,
        );
        let probability = economic_config.map_or(base_probability, |config| {
            economy::adjust_mortality_probability(
                config,
                state,
                person.settlement_id,
                base_probability,
            )
        });
        if occurs(
            seed,
            rules,
            DecisionDomain::Mortality,
            year,
            logical,
            0,
            probability,
        ) {
            apply_death(
                scenario,
                state,
                indexes,
                year,
                person_id,
                DeathCause::OrdinaryMortality,
                events,
            )?;
        }
    }
    Ok(())
}

fn mortality_probability(bands: &[AgeMortalityBand], age: u32, fallback: u32) -> u32 {
    bands
        .iter()
        .find(|band| {
            age >= u32::from(band.minimum_age)
                && band.maximum_age.is_none_or(|max| age <= u32::from(max))
        })
        .map_or(fallback, |band| band.probability_ppm)
}

pub(crate) fn apply_death(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    person_id: PersonId,
    cause: DeathCause,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    let Some(person) = state.persons.get(&person_id) else {
        return Ok(());
    };
    if !person.alive_at_start(year)
        || person
            .death_year
            .is_some_and(|death_year| death_year <= year)
    {
        return Ok(());
    }
    let person_logical = person.logical_id;
    let settlement_id = person.settlement_id;
    let old_household = person.household_id;
    if let Some(history) = state.residences.get_mut(&person_id) {
        if let Some(active) = history
            .iter_mut()
            .rev()
            .find(|interval| interval.validity.end_exclusive.is_none())
        {
            active.validity.end_exclusive = Some(year);
        }
    }
    if let Some(intervals) = state.profession_intervals.get_mut(&person_id) {
        if let Some(active) = intervals
            .iter_mut()
            .rev()
            .find(|interval| interval.validity.end_exclusive.is_none())
        {
            active.validity.end_exclusive = Some(year);
        }
    }
    state
        .persons
        .get_mut(&person_id)
        .expect("person checked above")
        .death_year = Some(year);
    emit_event(
        state,
        events,
        year,
        person_logical,
        vec![
            EntityRef::World(state.world_id),
            EntityRef::Person(person_id),
            EntityRef::Settlement(settlement_id),
        ],
        HistoricalEventKind::PersonDied { person_id, cause },
    );
    if let Some(culture_config) = &scenario.scenario().culture {
        culture::apply_population_delta(
            culture_config,
            state,
            year,
            culture::CulturePopulationDelta {
                person_id,
                from: Some(settlement_id),
                to: None,
            },
            events,
        )
        .map_err(|error| EngineError::Culture(error.to_string()))?;
    }

    if let Some(household_id) = old_household {
        if let Some(household) = state.households.get_mut(&household_id) {
            household.members.remove(&person_id);
            if household.members.is_empty() {
                household.closed_year = Some(year);
            }
        }
        state
            .persons
            .get_mut(&person_id)
            .expect("person checked above")
            .household_id = None;
        let household_logical = state
            .households
            .get(&household_id)
            .map_or(person_logical, |household| household.logical_id);
        emit_event(
            state,
            events,
            year,
            household_logical,
            vec![
                EntityRef::Person(person_id),
                EntityRef::Household(household_id),
            ],
            HistoricalEventKind::HouseholdChanged {
                person_id,
                from: Some(household_id),
                to: None,
            },
        );
    }

    let union_ids = indexes
        .active_unions_by_person
        .get(&person_id)
        .cloned()
        .unwrap_or_default();
    for union_id in union_ids {
        let Some(union) = state.unions.get_mut(&union_id) else {
            continue;
        };
        if !union.active_at_close(year) {
            continue;
        }
        union.validity.end_exclusive = Some(year);
        let union = union.clone();
        indexes.close_union(&union);
        let union_id = union.id;
        let union_logical = union.logical_id;
        for surviving_id in union
            .participants
            .iter()
            .copied()
            .filter(|participant| *participant != person_id)
        {
            if state
                .persons
                .get(&surviving_id)
                .is_some_and(|survivor| survivor.alive_at_start(year))
            {
                state
                    .persons
                    .get_mut(&surviving_id)
                    .expect("survivor checked above")
                    .last_widowed_year = Some(year);
            }
        }
        emit_event(
            state,
            events,
            year,
            union_logical,
            vec![EntityRef::Union(union_id), EntityRef::Person(person_id)],
            HistoricalEventKind::UnionEnded {
                union_id,
                reason: UnionEndReason::Widowhood(person_id),
            },
        );
    }
    let mut care_link_ids: BTreeSet<_> = indexes
        .family_links_by_child
        .get(&person_id)
        .into_iter()
        .flatten()
        .copied()
        .collect();
    care_link_ids.extend(
        indexes
            .caregiver_links_by_parent
            .get(&person_id)
            .into_iter()
            .flatten()
            .copied(),
    );
    for link_id in care_link_ids {
        let Some(link) = state.family_links.get_mut(&link_id) else {
            continue;
        };
        if link.kind != FamilyLinkKind::Caregiver || !link.active_at_close(year) {
            continue;
        }
        link.validity.end_exclusive = Some(year);
        let closed = link.clone();
        if let Some(parents) = indexes.caregivers_by_child.get_mut(&closed.child) {
            parents.remove(&closed.parent);
        }
        if let Some(links) = indexes.family_links_by_child.get_mut(&closed.child) {
            links.retain(|id| *id != link_id);
        }
        if let Some(links) = indexes.caregiver_links_by_parent.get_mut(&closed.parent) {
            links.retain(|id| *id != link_id);
        }
        emit_event(
            state,
            events,
            year,
            closed.logical_id,
            vec![
                EntityRef::World(state.world_id),
                EntityRef::Person(closed.parent),
                EntityRef::Person(closed.child),
                EntityRef::FamilyLink(link_id),
            ],
            HistoricalEventKind::FamilyLinkEnded {
                link_id,
                parent: closed.parent,
                child: closed.child,
                kind: closed.kind,
                reason: FamilyLinkEndReason::EndpointDeath(person_id),
            },
        );
    }
    Ok(())
}

fn apply_union_and_care_policies(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    let policy = &scenario.scenario().policies.unions;
    if policy.enabled {
        let seed = scenario.scenario().seed;
        let rules = scenario.scenario().rules_version;
        let mut active: Vec<_> = state
            .unions
            .values()
            .filter(|union| union.active_at_close(year))
            .map(|union| union.logical_id)
            .collect();
        active.sort_unstable();
        let ids_by_logical: BTreeMap<_, _> = state
            .unions
            .values()
            .map(|union| (union.logical_id, union.id))
            .collect();
        for union_logical in active {
            let union_id = ids_by_logical[&union_logical];
            if occurs(
                seed,
                rules,
                DecisionDomain::UnionSeparation,
                year,
                union_logical,
                0,
                policy.separation_probability_ppm,
            ) {
                let union = state
                    .unions
                    .get_mut(&union_id)
                    .expect("union index is current");
                union.validity.end_exclusive = Some(year);
                let closed_union = union.clone();
                indexes.close_union(&closed_union);
                let participants: Vec<_> = union.participants.iter().copied().collect();
                let mut entities = vec![EntityRef::Union(union_id)];
                entities.extend(participants.iter().copied().map(EntityRef::Person));
                emit_event(
                    state,
                    events,
                    year,
                    union_logical,
                    entities,
                    HistoricalEventKind::UnionEnded {
                        union_id,
                        reason: UnionEndReason::Separation,
                    },
                );
            }
        }
        form_unions(scenario, state, indexes, year, events)?;
    }
    apply_care_and_adoption(scenario, state, indexes, year, events)
}

fn form_unions(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    let config = scenario.scenario();
    let policy = &config.policies.unions;
    let mut initiators: Vec<_> = indexes
        .settlement_people
        .values()
        .flatten()
        .map(|id| (state.persons[id].logical_id, *id))
        .collect();
    initiators.sort_unstable();
    let mut unions_created_by_person: BTreeMap<PersonId, u16> = BTreeMap::new();

    for (initiator_logical, initiator_id) in initiators {
        for attempt in 0..policy.formation_attempts_per_person {
            if !eligible_for_union(state, indexes, initiator_id, year, policy) {
                break;
            }
            if unions_created_by_person
                .get(&initiator_id)
                .copied()
                .unwrap_or(0)
                >= policy.formation_attempts_per_person
            {
                break;
            }
            let attempt_key = u64::from(attempt);
            if !occurs(
                config.seed,
                config.rules_version,
                DecisionDomain::UnionFormation,
                year,
                initiator_logical,
                attempt_key,
                policy.formation_probability_ppm,
            ) {
                continue;
            }
            let settlement_id = state.persons[&initiator_id].settlement_id;
            let Some(candidates) = indexes.settlement_people.get(&settlement_id) else {
                continue;
            };
            let mut sample = sample_indices(
                candidates.len(),
                usize::from(policy.candidate_limit),
                config.seed,
                config.rules_version,
                DecisionDomain::UnionCandidate,
                year,
                initiator_logical,
                attempt_key,
            );
            sample.sort_by_key(|index| {
                keyed_random_u64(
                    config.seed,
                    config.rules_version,
                    DecisionDomain::UnionCandidate,
                    year,
                    state.persons[&candidates[*index]].logical_id,
                    attempt_key,
                )
            });
            let mut selected = None;
            for index in sample {
                let candidate_id = candidates[index];
                if candidate_id == initiator_id
                    || !eligible_for_union(state, indexes, candidate_id, year, policy)
                {
                    continue;
                }
                if policy.prohibited_kinship_generations > 0
                    && related_people(
                        indexes,
                        initiator_id,
                        candidate_id,
                        policy.prohibited_kinship_generations,
                    )
                {
                    continue;
                }
                selected = Some(candidate_id);
                break;
            }
            let Some(candidate_id) = selected else {
                continue;
            };
            let union_id = allocate_union_id(state)?;
            let logical_id = derive_logical_id(
                config.seed,
                config.rules_version,
                DecisionDomain::UnionIdentity,
                year,
                initiator_logical,
                attempt_key,
            );
            let participants = [initiator_id, candidate_id]
                .into_iter()
                .collect::<BTreeSet<_>>();
            let mut event_participants = participants.iter().copied().collect::<Vec<_>>();
            event_participants.sort_by_key(|person_id| state.persons[person_id].logical_id);
            let mut entities = vec![EntityRef::Union(union_id)];
            entities.extend(event_participants.iter().copied().map(EntityRef::Person));
            state.unions.insert(
                union_id,
                Union {
                    id: union_id,
                    logical_id,
                    participants,
                    kind: policy.kind.clone(),
                    validity: YearInterval {
                        start: year,
                        end_exclusive: None,
                    },
                },
            );
            indexes.add_union(
                &state.unions[&union_id],
                state,
                year,
                &config.policies.births,
            );
            for person_id in [initiator_id, candidate_id] {
                *unions_created_by_person.entry(person_id).or_default() += 1;
            }
            emit_event(
                state,
                events,
                year,
                logical_id,
                entities,
                HistoricalEventKind::UnionStarted {
                    union_id,
                    participants: event_participants,
                    kind: policy.kind.clone(),
                },
            );
        }
    }
    Ok(())
}

fn eligible_for_union(
    state: &State,
    indexes: &AnnualIndexes,
    person_id: PersonId,
    year: Year,
    policy: &chargen_domain::UnionPolicy,
) -> bool {
    let Some(person) = state.persons.get(&person_id) else {
        return false;
    };
    if !person.alive_at_close(year) {
        return false;
    }
    let Some(age) = year.age_of(person.birth_year) else {
        return false;
    };
    if age < u32::from(policy.minimum_age)
        || policy.maximum_age.is_some_and(|max| age > u32::from(max))
    {
        return false;
    }
    if !policy.allow_widowed_remarriage && person.last_widowed_year.is_some() {
        return false;
    }
    !matches!(
        policy.max_active_unions_per_person,
        Some(limit)
            if indexes
                .active_unions_by_person
                .get(&person_id)
                .map_or(0, BTreeSet::len)
                >= usize::from(limit)
    )
}

fn apply_care_and_adoption(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    let policy = &scenario.scenario().policies.adoption;
    if !policy.enabled {
        return Ok(());
    }
    if policy.add_household_care_links {
        add_household_care_links(scenario, state, indexes, year, events)?;
    }
    if policy.adopt_orphans {
        adopt_orphans(scenario, state, indexes, year, events)?;
    }
    Ok(())
}

fn add_household_care_links(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    let policy = &scenario.scenario().policies.adoption;
    let mut proposals = Vec::new();
    for household in state
        .households
        .values()
        .filter(|household| household.closed_year.is_none())
    {
        let adults = indexes
            .household_adults
            .get(&household.id)
            .cloned()
            .unwrap_or_default();
        for child_id in &household.members {
            let Some(child) = state.persons.get(child_id) else {
                continue;
            };
            let age = year.age_of(child.birth_year).unwrap_or(u32::MAX);
            if !child.alive_at_close(year) || age >= u32::from(policy.minor_age) {
                continue;
            }
            let mut needed = usize::from(policy.maximum_caregivers_per_child).saturating_sub(
                indexes
                    .caregivers_by_child
                    .get(child_id)
                    .map_or(0, BTreeSet::len),
            );
            if needed == 0 {
                continue;
            }
            for adult_id in adults.iter().filter(|adult_id| **adult_id != *child_id) {
                if would_create_family_cycle(indexes, *adult_id, *child_id) {
                    continue;
                }
                if indexes
                    .caregivers_by_child
                    .get(child_id)
                    .is_some_and(|parents| parents.contains(adult_id))
                {
                    continue;
                }
                if proposals
                    .iter()
                    .any(|(parent, child)| *parent == *adult_id && *child == *child_id)
                {
                    continue;
                }
                proposals.push((*adult_id, *child_id));
                needed -= 1;
                if needed == 0 {
                    break;
                }
            }
        }
    }
    proposals.sort_by_key(|(parent, child)| {
        (
            state.persons[parent].logical_id,
            state.persons[child].logical_id,
        )
    });
    for (parent, child) in proposals {
        if would_create_family_cycle(indexes, parent, child) {
            continue;
        }
        if indexes
            .caregivers_by_child
            .get(&child)
            .is_some_and(|parents| parents.contains(&parent))
        {
            continue;
        }
        create_family_link(
            scenario,
            state,
            indexes,
            year,
            parent,
            child,
            FamilyLinkKind::Caregiver,
            events,
        )?;
    }
    Ok(())
}

fn adopt_orphans(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    let config = scenario.scenario();
    let policy = &config.policies.adoption;
    let mut minors: Vec<_> = state
        .persons
        .values()
        .filter(|person| {
            person.alive_at_close(year)
                && year
                    .age_of(person.birth_year)
                    .is_some_and(|age| age < u32::from(policy.minor_age))
                && !indexes
                    .family_links_by_child
                    .get(&person.id)
                    .into_iter()
                    .flatten()
                    .any(|id| {
                        let link = &state.family_links[id];
                        matches!(
                            link.kind,
                            FamilyLinkKind::BiologicalParent | FamilyLinkKind::AdoptiveParent
                        ) && state
                            .persons
                            .get(&link.parent)
                            .is_some_and(|parent| parent.alive_at_close(year))
                    })
        })
        .map(|person| (person.logical_id, person.id))
        .collect();
    minors.sort_unstable();
    let mut adoptions = 0_u32;
    for (child_logical, child_id) in minors {
        if adoptions >= policy.maximum_adoptions_per_year {
            break;
        }
        if !occurs(
            config.seed,
            config.rules_version,
            DecisionDomain::AdoptionChance,
            year,
            child_logical,
            0,
            policy.adoption_probability_ppm,
        ) {
            continue;
        }
        let settlement_id = state.persons[&child_id].settlement_id;
        let mut households: Vec<_> = indexes
            .adoption_households_by_settlement
            .get(&settlement_id)
            .into_iter()
            .flatten()
            .map(|household_id| (state.households[household_id].logical_id, *household_id))
            .collect();
        households.sort_unstable();
        let mut sample = sample_indices(
            households.len(),
            usize::from(policy.candidate_limit),
            config.seed,
            config.rules_version,
            DecisionDomain::AdoptionCandidate,
            year,
            child_logical,
            0,
        );
        sample.sort_by_key(|index| {
            keyed_random_u64(
                config.seed,
                config.rules_version,
                DecisionDomain::AdoptionCandidate,
                year,
                households[*index].0,
                0,
            )
        });
        let Some(household_id) = sample.first().map(|index| households[*index].1) else {
            continue;
        };
        if Some(household_id) == state.persons[&child_id].household_id {
            continue;
        }
        let adult = indexes
            .household_adults
            .get(&household_id)
            .into_iter()
            .flatten()
            .find(|adult_id| !would_create_family_cycle(indexes, **adult_id, child_id))
            .copied();
        let Some(parent_id) = adult else { continue };
        if policy.move_to_adoptive_household {
            move_to_household(state, indexes, year, child_id, household_id, policy, events);
        }
        create_family_link(
            scenario,
            state,
            indexes,
            year,
            parent_id,
            child_id,
            FamilyLinkKind::AdoptiveParent,
            events,
        )?;
        adoptions += 1;
    }
    Ok(())
}

fn move_to_household(
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    person_id: PersonId,
    destination: HouseholdId,
    policy: &AdoptionPolicy,
    events: &mut Vec<HistoricalEvent>,
) {
    let Some(person) = state.persons.get(&person_id) else {
        return;
    };
    let old = person.household_id;
    if old == Some(destination) {
        return;
    }
    let person_logical = person.logical_id;
    let age = year
        .age_of(state.persons[&person_id].birth_year)
        .unwrap_or(0);
    if let Some(old_id) = old {
        if let Some(old_household) = state.households.get_mut(&old_id) {
            old_household.members.remove(&person_id);
            if old_household.members.is_empty() {
                old_household.closed_year = Some(year);
            }
        }
        if age < u32::from(policy.minor_age) {
            if let Some(count) = indexes.dependent_count_by_household.get_mut(&old_id) {
                *count = count.saturating_sub(1);
            }
        }
        if age >= u32::from(policy.minimum_caregiver_age) {
            if let Some(adults) = indexes.household_adults.get_mut(&old_id) {
                adults.retain(|id| *id != person_id);
            }
        }
        if state
            .households
            .get(&old_id)
            .is_some_and(|household| household.closed_year.is_some())
        {
            if let Some(households) = indexes
                .settlement_households
                .get_mut(&state.households[&old_id].settlement_id)
            {
                households.retain(|id| *id != old_id);
            }
        }
    }
    state
        .persons
        .get_mut(&person_id)
        .expect("person exists")
        .household_id = Some(destination);
    if let Some(new_household) = state.households.get_mut(&destination) {
        new_household.members.insert(person_id);
    }
    if age < u32::from(policy.minor_age) {
        *indexes
            .dependent_count_by_household
            .entry(destination)
            .or_default() += 1;
    }
    if age >= u32::from(policy.minimum_caregiver_age) {
        let adults = indexes.household_adults.entry(destination).or_default();
        if !adults.contains(&person_id) {
            adults.push(person_id);
        }
        adults.sort_by_key(|id| state.persons[id].logical_id);
    }
    if let Some(old_id) = old {
        refresh_adoption_household(state, indexes, old_id, policy);
    }
    refresh_adoption_household(state, indexes, destination, policy);
    let mut entities = vec![
        EntityRef::Person(person_id),
        EntityRef::Household(destination),
    ];
    if let Some(old_id) = old {
        entities.push(EntityRef::Household(old_id));
    }
    emit_event(
        state,
        events,
        year,
        person_logical,
        entities,
        HistoricalEventKind::HouseholdChanged {
            person_id,
            from: old,
            to: Some(destination),
        },
    );
}

fn refresh_adoption_household(
    state: &State,
    indexes: &mut AnnualIndexes,
    household_id: HouseholdId,
    policy: &AdoptionPolicy,
) {
    let Some(household) = state.households.get(&household_id) else {
        return;
    };
    let has_adult = indexes
        .household_adults
        .get(&household_id)
        .is_some_and(|adults| !adults.is_empty());
    let has_capacity = policy.maximum_children_per_household.is_none_or(|limit| {
        indexes
            .dependent_count_by_household
            .get(&household_id)
            .copied()
            .unwrap_or(0)
            < limit
    });
    let eligible = household.closed_year.is_none() && has_adult && has_capacity;
    let households = indexes
        .adoption_households_by_settlement
        .entry(household.settlement_id)
        .or_default();
    if eligible {
        households.insert(household_id);
    } else {
        households.remove(&household_id);
    }
}

fn would_create_family_cycle(indexes: &AnnualIndexes, parent: PersonId, child: PersonId) -> bool {
    let mut visited = BTreeSet::from([child]);
    let mut frontier = vec![child];
    while let Some(current) = frontier.pop() {
        if current == parent {
            return true;
        }
        if let Some(children) = indexes.children_by_parent.get(&current) {
            for next in children {
                if visited.insert(*next) {
                    frontier.push(*next);
                }
            }
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
fn create_family_link(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    parent: PersonId,
    child: PersonId,
    kind: FamilyLinkKind,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    let link_id = allocate_family_link_id(state)?;
    let parent_logical = state.persons[&parent].logical_id;
    let child_logical = state.persons[&child].logical_id;
    let logical_id = derive_logical_id(
        scenario.scenario().seed,
        scenario.scenario().rules_version,
        DecisionDomain::FamilyLinkIdentity,
        year,
        parent_logical,
        u64::from(events.len() as u32),
    );
    state.family_links.insert(
        link_id,
        FamilyLink {
            id: link_id,
            logical_id,
            parent,
            child,
            kind,
            validity: YearInterval {
                start: year,
                end_exclusive: None,
            },
        },
    );
    indexes.add_family_link(&state.family_links[&link_id]);
    emit_event(
        state,
        events,
        year,
        parent_logical,
        vec![
            EntityRef::Person(parent),
            EntityRef::Person(child),
            EntityRef::FamilyLink(link_id),
        ],
        HistoricalEventKind::FamilyLinkEstablished {
            link_id,
            parent,
            child,
            kind,
        },
    );
    let _ = child_logical;
    Ok(())
}

fn apply_births(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
    economic_config: Option<&chargen_domain::EconomicConfig>,
) -> Result<(), EngineError> {
    let config = scenario.scenario();
    let policy = &config.policies.births;
    if !policy.enabled {
        return Ok(());
    }
    let mut potential_birth_gestators: Vec<_> = state
        .persons
        .values()
        .filter(|person| {
            person.alive_at_close(year)
                && person.reproductive.fertility_enabled
                && person.reproductive.can_gestate
        })
        .map(|person| (person.logical_id, person.id))
        .collect();
    potential_birth_gestators.sort_unstable();
    for (gestator_logical, gestator_id) in potential_birth_gestators {
        let last_birth_year = state.last_gestational_birth_year.get(&gestator_id).copied();
        if last_birth_year.is_some_and(|last_year| {
            year.0.saturating_sub(last_year.0) < i32::from(policy.minimum_years_between_births)
        }) {
            continue;
        }
        let age = year
            .age_of(state.persons[&gestator_id].birth_year)
            .unwrap_or(0);
        if age < u32::from(policy.minimum_gestational_age)
            || age > u32::from(policy.maximum_gestational_age)
        {
            continue;
        }
        let mut had_birth = false;
        let settlement_id = state.persons[&gestator_id].settlement_id;
        let birth_probability = economic_config.map_or(policy.probability_ppm, |economic| {
            let reduction =
                economy::current_birth_scarcity_reduction(economic, state, settlement_id);
            policy.probability_ppm.saturating_sub(reduction)
        });
        for ordinal in 0..policy.maximum_births_per_person_per_year {
            if !occurs(
                config.seed,
                config.rules_version,
                DecisionDomain::BirthChance,
                year,
                gestator_logical,
                u64::from(ordinal),
                birth_probability,
            ) {
                continue;
            }
            let contributor =
                find_genetic_contributor(scenario, state, indexes, year, gestator_id, ordinal)?;
            if policy.require_active_union
                && contributor.is_none()
                && !policy.allow_unknown_contributor
            {
                continue;
            }
            if contributor.is_none() && !policy.allow_unknown_contributor {
                continue;
            }
            let contributors = contributor.into_iter().collect::<Vec<_>>();
            let person_id = allocate_person_id(state)?;
            let logical_id = derive_logical_id(
                config.seed,
                config.rules_version,
                DecisionDomain::PersonIdentity,
                year,
                gestator_logical,
                u64::from(ordinal),
            );
            let household_id = state.persons[&gestator_id].household_id;
            let settlement_id = state.persons[&gestator_id].settlement_id;
            let reproductive = birth_reproductive_profile(scenario, year, logical_id);
            let genome = inherit_genome(
                scenario,
                state,
                year,
                logical_id,
                gestator_id,
                contributors.first().copied(),
            )?;
            let traits = inherit_traits(
                scenario,
                state,
                year,
                logical_id,
                gestator_id,
                contributors.first().copied(),
                settlement_id,
            );
            let person = Person {
                id: person_id,
                logical_id,
                birth_year: year,
                death_year: None,
                settlement_id,
                household_id,
                reproductive,
                traits,
                genome,
                last_widowed_year: None,
                profession: None,
            };
            state.persons.insert(person_id, person);
            if state.rules_version >= chargen_domain::Scenario::TERRITORIAL_RULES_VERSION {
                state.residences.insert(
                    person_id,
                    vec![ResidenceInterval {
                        settlement_id,
                        validity: YearInterval {
                            start: year,
                            end_exclusive: None,
                        },
                    }],
                );
            }
            indexes
                .settlement_people
                .entry(settlement_id)
                .or_default()
                .push(person_id);
            if let Some(household_id) = household_id {
                if let Some(household) = state.households.get_mut(&household_id) {
                    household.closed_year = None;
                    household.members.insert(person_id);
                }
            }
            let mut genetic_parents = vec![gestator_id];
            if let Some(parent_id) = contributors.first() {
                genetic_parents.push(*parent_id);
            }
            genetic_parents.sort_by_key(|id| state.persons[id].logical_id);
            let mut entities = vec![
                EntityRef::World(state.world_id),
                EntityRef::Person(person_id),
                EntityRef::Person(gestator_id),
                EntityRef::Settlement(settlement_id),
            ];
            if let Some(household_id) = household_id {
                entities.push(EntityRef::Household(household_id));
            }
            if let Some(parent_id) = contributors.first() {
                entities.push(EntityRef::Person(*parent_id));
            }
            emit_event(
                state,
                events,
                year,
                logical_id,
                entities,
                HistoricalEventKind::PersonBorn {
                    person_id,
                    gestational_parent: gestator_id,
                    genetic_parents: genetic_parents.clone(),
                    unknown_genetic_contributors: 2_u8.saturating_sub(genetic_parents.len() as u8),
                    household_id,
                },
            );
            for parent_id in genetic_parents.iter().copied() {
                create_family_link(
                    scenario,
                    state,
                    indexes,
                    year,
                    parent_id,
                    person_id,
                    FamilyLinkKind::BiologicalParent,
                    events,
                )?;
            }
            if occurs(
                config.seed,
                config.rules_version,
                DecisionDomain::InfantMortality,
                year,
                logical_id,
                u64::from(ordinal),
                policy.infant_mortality_probability_ppm,
            ) {
                apply_death(
                    scenario,
                    state,
                    indexes,
                    year,
                    person_id,
                    DeathCause::InfantMortality,
                    events,
                )?;
            }
            had_birth = true;
        }
        if had_birth {
            state.last_gestational_birth_year.insert(gestator_id, year);
        }
    }
    Ok(())
}

fn birth_reproductive_profile(
    scenario: &ValidatedScenario,
    year: Year,
    logical: LogicalId,
) -> ReproductiveProfile {
    let profiles = &scenario
        .scenario()
        .policies
        .births
        .child_reproductive_profiles;
    choose_weighted(
        profiles,
        |entry| entry.weight,
        scenario.scenario().seed,
        scenario.scenario().rules_version,
        DecisionDomain::ReproductiveProfile,
        year,
        logical,
        1,
    )
    .map(|entry| entry.profile.clone())
    .unwrap_or_default()
}

fn find_genetic_contributor(
    scenario: &ValidatedScenario,
    state: &State,
    indexes: &AnnualIndexes,
    year: Year,
    gestator_id: PersonId,
    ordinal: u8,
) -> Result<Option<PersonId>, EngineError> {
    let config = scenario.scenario();
    let policy = &config.policies.births;
    let gestator = &state.persons[&gestator_id];
    let candidate_limit = usize::from(config.policies.unions.candidate_limit.max(1));
    if policy.require_active_union {
        let Some(unions) = indexes.active_union_ids_by_person.get(&gestator_id) else {
            return Ok(None);
        };
        let union_index = random_below(
            config.seed,
            config.rules_version,
            DecisionDomain::BirthContributor,
            year,
            gestator.logical_id,
            u64::from(ordinal),
            unions.len() as u64,
        ) as usize;
        let union_id = unions[union_index];
        let candidates = indexes
            .birth_candidates_by_union
            .get(&(union_id, policy.required_contributor_gamete))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        Ok(select_birth_candidate(
            candidates,
            candidate_limit,
            config.seed,
            config.rules_version,
            year,
            gestator,
            gestator_id,
            ordinal,
            state,
        ))
    } else {
        let candidates = if let Some(gamete) = policy.required_contributor_gamete {
            indexes
                .birth_candidates_by_settlement_gamete
                .get(&(gestator.settlement_id, gamete))
                .map(Vec::as_slice)
                .unwrap_or(&[])
        } else {
            indexes
                .birth_candidates_by_settlement
                .get(&gestator.settlement_id)
                .map(Vec::as_slice)
                .unwrap_or(&[])
        };
        Ok(select_birth_candidate(
            candidates,
            candidate_limit,
            config.seed,
            config.rules_version,
            year,
            gestator,
            gestator_id,
            ordinal,
            state,
        ))
    }
}

fn eligible_birth_contributor(
    state: &State,
    person_id: PersonId,
    year: Year,
    policy: &BirthPolicy,
) -> bool {
    state.persons.get(&person_id).is_some_and(|person| {
        person.alive_at_close(year)
            && person.reproductive.fertility_enabled
            && year
                .age_of(person.birth_year)
                .is_some_and(|age| age >= u32::from(policy.minimum_contributor_age))
    })
}

#[allow(clippy::too_many_arguments)]
fn select_birth_candidate(
    candidates: &[PersonId],
    limit: usize,
    seed: u64,
    rules: u32,
    year: Year,
    gestator: &Person,
    gestator_id: PersonId,
    ordinal: u8,
    state: &State,
) -> Option<PersonId> {
    let mut sample = sample_indices(
        candidates.len(),
        limit.saturating_add(1),
        seed,
        rules,
        DecisionDomain::BirthContributor,
        year,
        gestator.logical_id,
        u64::from(ordinal),
    );
    sample.sort_by_key(|index| {
        keyed_random_u64(
            seed,
            rules,
            DecisionDomain::BirthContributor,
            year,
            state.persons[&candidates[*index]].logical_id,
            u64::from(ordinal),
        )
    });
    sample
        .into_iter()
        .map(|index| candidates[index])
        .find(|id| *id != gestator_id)
}

fn inherit_genome(
    scenario: &ValidatedScenario,
    state: &State,
    year: Year,
    child_logical: LogicalId,
    gestator_id: PersonId,
    contributor_id: Option<PersonId>,
) -> Result<chargen_domain::Genome, EngineError> {
    let config = scenario.scenario();
    let gestator = &state.persons[&gestator_id];
    let contributor = contributor_id.and_then(|id| state.persons.get(&id));
    let mut genome = chargen_domain::Genome::default();
    for (locus_index, locus) in config.genetics.loci.iter().enumerate() {
        let gestator_pair = gestator.genome.loci.get(&locus.id).ok_or_else(|| {
            EngineError::InvalidPopulation(format!("parent genome missing locus {}", locus.id.0))
        })?;
        let gestator_allele_index = random_below(
            config.seed,
            config.rules_version,
            DecisionDomain::AlleleInheritance,
            year,
            child_logical,
            (locus_index as u64).wrapping_mul(2),
            2,
        ) as usize;
        let mut first = gestator_pair[gestator_allele_index].clone();
        first.inherited_from = Some(gestator.logical_id);
        let second = if let Some(contributor) = contributor {
            let parent_pair = contributor.genome.loci.get(&locus.id).ok_or_else(|| {
                EngineError::InvalidPopulation(format!(
                    "parent genome missing locus {}",
                    locus.id.0
                ))
            })?;
            let index = random_below(
                config.seed,
                config.rules_version,
                DecisionDomain::AlleleInheritance,
                year,
                child_logical,
                (locus_index as u64).wrapping_mul(2).wrapping_add(1),
                2,
            ) as usize;
            let mut allele = parent_pair[index].clone();
            allele.inherited_from = Some(contributor.logical_id);
            allele
        } else {
            founder_allele(
                &config.genetics,
                locus_index,
                child_logical,
                1,
                config.seed,
                config.rules_version,
                year,
            )?
        };
        genome.loci.insert(locus.id.clone(), [first, second]);
    }
    Ok(genome)
}

fn inherit_traits(
    scenario: &ValidatedScenario,
    state: &State,
    year: Year,
    child_logical: LogicalId,
    gestator_id: PersonId,
    contributor_id: Option<PersonId>,
    settlement_id: SettlementId,
) -> BTreeMap<TraitKey, i16> {
    let config = scenario.scenario();
    let parent = &state.persons[&gestator_id];
    let contributor = contributor_id.and_then(|id| state.persons.get(&id));
    let Some(settlement) = state.settlements.get(&settlement_id) else {
        return BTreeMap::new();
    };
    let mut values = BTreeMap::new();
    for (index, (key, policy)) in config.policies.traits.iter().enumerate() {
        let mut numerator = 0_i64;
        let mut denominator = 0_i64;
        let parent_values: Vec<_> = [Some(parent), contributor]
            .into_iter()
            .flatten()
            .filter_map(|person| person.traits.get(key))
            .map(|value| i64::from(*value))
            .collect();
        if !parent_values.is_empty() && policy.parent_weight > 0 {
            numerator += (parent_values.iter().sum::<i64>() / parent_values.len() as i64)
                * i64::from(policy.parent_weight);
            denominator += i64::from(policy.parent_weight);
        }
        if let Some(value) = settlement.culture_values.get(key) {
            numerator += i64::from(*value) * i64::from(policy.local_weight);
            denominator += i64::from(policy.local_weight);
        }
        let base = if denominator == 0 {
            0
        } else {
            numerator / denominator
        };
        let variation = if policy.variation_max == 0 {
            0
        } else {
            let width = u64::from(policy.variation_max as u16) * 2 + 1;
            random_below(
                config.seed,
                config.rules_version,
                DecisionDomain::TraitVariation,
                year,
                child_logical,
                index as u64,
                width,
            ) as i64
                - i64::from(policy.variation_max)
        };
        let result =
            (base + variation).clamp(i64::from(policy.minimum), i64::from(policy.maximum)) as i16;
        values.insert(key.clone(), result);
    }
    values
}

fn related_people(indexes: &AnnualIndexes, left: PersonId, right: PersonId, max_depth: u8) -> bool {
    if max_depth == 0 {
        return false;
    }
    let mut distances = BTreeMap::from([(left, 0_u8)]);
    let mut frontier = vec![left];
    while let Some(current) = frontier.pop() {
        let distance = distances[&current];
        if distance >= max_depth {
            continue;
        }
        if let Some(neighbors) = indexes.kinship_neighbors.get(&current) {
            for neighbor in neighbors {
                let next = distance + 1;
                if *neighbor == right {
                    return true;
                }
                if distances.get(neighbor).is_some_and(|known| *known <= next) {
                    continue;
                }
                distances.insert(*neighbor, next);
                frontier.push(*neighbor);
            }
        }
    }
    false
}

/// Floyd's algorithm samples at most K distinct indices in O(K), independent of population size.
// Keep the versioned RNG inputs explicit to make decision keys auditable.
#[allow(clippy::too_many_arguments)]
fn sample_indices(
    count: usize,
    limit: usize,
    seed: u64,
    rules: u32,
    domain: DecisionDomain,
    year: Year,
    logical: LogicalId,
    ordinal: u64,
) -> Vec<usize> {
    let amount = count.min(limit);
    if amount == 0 {
        return Vec::new();
    }
    if amount == count {
        return (0..count).collect();
    }
    let mut selected = BTreeSet::new();
    for offset in 0..amount {
        let j = count - amount + offset;
        let draw = random_below(
            seed,
            rules,
            domain,
            year,
            logical,
            ordinal.wrapping_mul(2048).wrapping_add(offset as u64),
            (j + 1) as u64,
        ) as usize;
        if !selected.insert(draw) {
            selected.insert(j);
        }
    }
    selected.into_iter().collect()
}

pub(crate) fn emit_event(
    state: &State,
    events: &mut Vec<HistoricalEvent>,
    year: Year,
    logical: LogicalId,
    affected_entities: Vec<EntityRef>,
    kind: HistoricalEventKind,
) {
    let order = events.len() as u32;
    let id = derive_logical_id(
        state.seed,
        state.rules_version,
        DecisionDomain::PersonIdentity,
        year,
        logical,
        u64::from(order).wrapping_add(0x4556_454e_5400),
    );
    events.push(HistoricalEvent {
        id,
        year,
        order,
        affected_entities,
        kind,
    });
}

fn allocate_person_id(state: &mut State) -> Result<PersonId, EngineError> {
    let id = state.next_person_id;
    state.next_person_id = id
        .checked_add(1)
        .ok_or(EngineError::IdExhausted("person"))?;
    Ok(PersonId(id))
}
fn allocate_household_id(state: &mut State) -> Result<HouseholdId, EngineError> {
    let id = state.next_household_id;
    state.next_household_id = id
        .checked_add(1)
        .ok_or(EngineError::IdExhausted("household"))?;
    Ok(HouseholdId(id))
}
fn allocate_union_id(state: &mut State) -> Result<UnionId, EngineError> {
    let id = state.next_union_id;
    state.next_union_id = id.checked_add(1).ok_or(EngineError::IdExhausted("union"))?;
    Ok(UnionId(id))
}
fn allocate_family_link_id(state: &mut State) -> Result<FamilyLinkId, EngineError> {
    let id = state.next_family_link_id;
    state.next_family_link_id = id
        .checked_add(1)
        .ok_or(EngineError::IdExhausted("family link"))?;
    Ok(FamilyLinkId(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chargen_domain::{
        AdoptionPolicy, BirthPolicy, CultureConfig, DemographicPolicies, DirectedRoute,
        EconomicConfig, EconomicResponsePolicy, EconomicYearReport, EconomySignals, EventEffect,
        EventPopulationFilter, EventRealization, EventSelection, ExactOverflowPolicy, FamilyLink,
        FamilyLinkKind, ForcedDisplacementCommand, GeneticCatalog, GeneticExpression, GeneticLocus,
        Genome, GoodDefinition, GoodId, HouseholdKind, InitialPopulationProfile, LocusId,
        MigrationPolicy, MigrationUnit, Money, MortalityPolicy, Person, ProductionRecipe,
        ProfessionDefinition, ProfessionEconomicPolicy, ProfessionId, ReproductiveProfile,
        Scenario, ScheduledEvent, Settlement, TerritoryConfig, UnionId, UnionKind, UnionPolicy,
        VariantId, WeightedAgeBand, WeightedReproductiveProfile, WeightedVariant,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn profile(can_gestate: bool, gamete: Option<chargen_domain::Gamete>) -> ReproductiveProfile {
        ReproductiveProfile {
            gametes: gamete.into_iter().collect(),
            can_gestate,
            fertility_enabled: true,
        }
    }

    fn base_scenario() -> Scenario {
        let settlement_id = SettlementId(1);
        let household_id = HouseholdId(1);
        let people = vec![
            Person {
                id: PersonId(1),
                logical_id: LogicalId(101),
                birth_year: Year(0),
                death_year: None,
                settlement_id,
                household_id: Some(household_id),
                reproductive: profile(true, Some(chargen_domain::Gamete::Ovum)),
                traits: BTreeMap::new(),
                genome: Genome::default(),
                last_widowed_year: None,
                profession: None,
            },
            Person {
                id: PersonId(2),
                logical_id: LogicalId(102),
                birth_year: Year(0),
                death_year: None,
                settlement_id,
                household_id: Some(household_id),
                reproductive: profile(false, Some(chargen_domain::Gamete::Sperm)),
                traits: BTreeMap::new(),
                genome: Genome::default(),
                last_widowed_year: None,
                profession: None,
            },
        ];
        Scenario {
            world_id: chargen_domain::WorldId(1),
            schema_version: 1,
            rules_version: 1,
            seed: 91,
            start_year: Year(20),
            settlements: vec![Settlement {
                id: settlement_id,
                logical_id: LogicalId(1),
                name: "Lake".into(),
                culture_values: BTreeMap::new(),
            }],
            initial_people: people,
            initial_households: vec![Household {
                id: household_id,
                logical_id: LogicalId(2),
                kind: HouseholdKind::Family,
                settlement_id,
                members: [PersonId(1), PersonId(2)].into_iter().collect(),
                closed_year: None,
            }],
            initial_unions: vec![Union {
                id: UnionId(1),
                logical_id: LogicalId(50),
                participants: [PersonId(1), PersonId(2)].into_iter().collect(),
                kind: UnionKind::Marriage,
                validity: YearInterval {
                    start: Year(18),
                    end_exclusive: None,
                },
            }],
            initial_family_links: vec![],
            trait_catalog: vec![],
            initial_population_profiles: vec![],
            policies: DemographicPolicies::default(),
            genetics: GeneticCatalog::default(),
            territory: None,
            culture: None,
            economy: None,
            scheduled_events: None,
        }
    }

    fn territorial_scenario() -> Scenario {
        let mut scenario = base_scenario();
        scenario.schema_version = Scenario::TERRITORIAL_SCHEMA_VERSION;
        scenario.rules_version = Scenario::TERRITORIAL_RULES_VERSION;
        scenario.settlements.push(Settlement {
            id: SettlementId(2),
            logical_id: LogicalId(2),
            name: "Hill".into(),
            culture_values: BTreeMap::new(),
        });
        scenario.territory = Some(TerritoryConfig {
            routes: vec![DirectedRoute {
                logical_id: LogicalId(900),
                from: SettlementId(1),
                to: SettlementId(2),
                cost: 0,
                capacity_per_year: None,
                open_intervals: Vec::new(),
            }],
            ..TerritoryConfig::default()
        });
        scenario.culture = Some(CultureConfig::default());
        scenario
    }

    fn event_scenario() -> Scenario {
        let mut scenario = territorial_scenario();
        scenario.schema_version = Scenario::EVENT_SCHEMA_VERSION;
        scenario.rules_version = Scenario::EVENT_RULES_VERSION;
        scenario.economy = Some(EconomicConfig::default());
        scenario.scheduled_events = Some(Vec::new());
        scenario
    }

    fn scheduled_event(
        id: u64,
        name: &str,
        start_year: Year,
        priority: i32,
        effects: Vec<EventEffect>,
    ) -> ScheduledEvent {
        ScheduledEvent {
            id: LogicalId(id),
            name: name.into(),
            start_year,
            duration_years: Some(1),
            priority,
            population_filter: EventPopulationFilter::default(),
            effects,
        }
    }

    fn productive_event_scenario(events: Vec<ScheduledEvent>) -> Scenario {
        let mut scenario = event_scenario();
        let farmer = ProfessionId::from("farmer");
        let grain = GoodId::from("grain");
        let territory = scenario.territory.as_mut().unwrap();
        territory.professions.push(ProfessionDefinition {
            id: farmer.clone(),
            name: "Farmer".into(),
        });
        territory
            .profession_options_by_settlement
            .insert(SettlementId(1), [farmer.clone()].into_iter().collect());
        for person in &mut scenario.initial_people {
            person.profession = Some(farmer.clone());
        }
        scenario.economy = Some(EconomicConfig {
            goods: [(
                grain.clone(),
                GoodDefinition {
                    name: "Grain".into(),
                    unit: "sack".into(),
                    transport_units_per_unit: 1,
                    storable: true,
                    annual_storage_loss_ppm: 0,
                    initial_price: Money(1),
                    minimum_price: Money(1),
                    maximum_price: Money(1),
                },
            )]
            .into_iter()
            .collect(),
            profession_policies: [(
                farmer,
                ProfessionEconomicPolicy {
                    recipe: Some(ProductionRecipe {
                        batches_per_worker: 10,
                        maximum_batches_per_year: None,
                        inputs_per_batch: BTreeMap::new(),
                        outputs_per_batch: [(grain, chargen_domain::Quantity(1))]
                            .into_iter()
                            .collect(),
                    }),
                },
            )]
            .into_iter()
            .collect(),
            ..EconomicConfig::default()
        });
        scenario.scheduled_events = Some(events);
        scenario
    }

    fn one_locus_catalog() -> GeneticCatalog {
        let a = VariantId::from("amber");
        let b = VariantId::from("blue");
        GeneticCatalog {
            loci: vec![GeneticLocus {
                id: LocusId::from("eye"),
                variants: [a.clone(), b.clone()].into_iter().collect(),
                founder_distribution: vec![
                    WeightedVariant {
                        variant: a.clone(),
                        weight: 1,
                    },
                    WeightedVariant {
                        variant: b.clone(),
                        weight: 1,
                    },
                ],
                expression: GeneticExpression::Dominant {
                    dominant_variants: [a].into_iter().collect(),
                    dominant_value: 1,
                    recessive_value: 0,
                },
            }],
        }
    }

    #[test]
    fn cancellation_before_commit_discards_the_pending_year() {
        let scenario = base_scenario().validate().unwrap();
        let mut engine = Engine::from_scenario(scenario).unwrap();
        let before = engine.state().clone();
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(
            engine.advance_year(Some(&token)),
            Err(EngineError::Cancelled)
        ));
        assert_eq!(engine.state(), &before);
    }

    #[test]
    fn steps_record_all_phases_and_deferred_work_explicitly() {
        let scenario = base_scenario().validate().unwrap();
        let mut engine = Engine::from_scenario(scenario).unwrap();
        let commit = engine.advance_year(None).unwrap();
        assert_eq!(commit.year, Year(20));
        assert_eq!(commit.state.last_year_phases.len(), 12);
        assert_eq!(
            commit.state.last_year_phases[2].disposition,
            PhaseDisposition::Implemented
        );
        assert_eq!(
            commit.state.last_year_phases[3].disposition,
            PhaseDisposition::DeferredNoop
        );
        assert_eq!(
            commit.state.last_year_phases[3].description,
            "reconciliación agregada de hogares y empleo pendiente"
        );
        assert_eq!(commit.state.current_year, Some(Year(20)));
    }

    #[test]
    fn ruleset_three_empty_agenda_has_no_autonomous_events_and_future_agenda_branches_cleanly() {
        let mut empty_engine = Engine::from_scenario(event_scenario().validate().unwrap()).unwrap();
        let empty_close = empty_engine.advance_year(None).unwrap();
        assert!(!empty_close.events.iter().any(|event| matches!(
            event.kind,
            HistoricalEventKind::ScheduledEventOccurred { .. }
                | HistoricalEventKind::EventEffectRealized { .. }
        )));

        let future = scheduled_event(
            901,
            "future work restriction",
            Year(21),
            0,
            vec![EventEffect::WorkIncapacity {
                selection: EventSelection::Exact {
                    count: 1,
                    overflow: ExactOverflowPolicy::ClampWithWarning,
                },
            }],
        );
        let mut raw = event_scenario();
        raw.scheduled_events = Some(vec![future]);
        let validated = raw.validate().unwrap();
        let mut parent = Engine::from_scenario(validated.clone()).unwrap();
        let cut = parent.advance_year(None).unwrap().state;
        let with_agenda = Engine::new_for_branch_revision(validated, cut.clone())
            .unwrap()
            .advance_year(None)
            .unwrap();

        raw.scheduled_events = Some(Vec::new());
        let without_agenda =
            Engine::new_for_branch_revision(raw.validate_for_branch_revision().unwrap(), cut)
                .unwrap()
                .advance_year(None)
                .unwrap();
        assert!(with_agenda.events.iter().any(|event| matches!(
            event.kind,
            HistoricalEventKind::ScheduledEventOccurred {
                event_id: LogicalId(901),
                ..
            }
        )));
        assert!(!without_agenda.events.iter().any(|event| matches!(
            event.kind,
            HistoricalEventKind::ScheduledEventOccurred { .. }
        )));
    }

    #[test]
    fn exact_event_overflow_error_discards_year_and_clamp_records_realized_warning() {
        let mut raw = event_scenario();
        raw.scheduled_events = Some(vec![scheduled_event(
            902,
            "too many deaths",
            Year(20),
            0,
            vec![EventEffect::Deaths {
                selection: EventSelection::Exact {
                    count: 3,
                    overflow: ExactOverflowPolicy::Error,
                },
            }],
        )]);
        let mut engine = Engine::from_scenario(raw.validate().unwrap()).unwrap();
        let before = engine.state().clone();
        assert!(matches!(
            engine.advance_year(None),
            Err(EngineError::ScheduledEvent(_))
        ));
        assert_eq!(engine.state(), &before);

        raw.scheduled_events.as_mut().unwrap()[0].effects[0] = EventEffect::Deaths {
            selection: EventSelection::Exact {
                count: 3,
                overflow: ExactOverflowPolicy::ClampWithWarning,
            },
        };
        let mut engine = Engine::from_scenario(raw.validate().unwrap()).unwrap();
        let commit = engine.advance_year(None).unwrap();
        let realization = commit
            .events
            .iter()
            .find_map(|event| match &event.kind {
                HistoricalEventKind::EventEffectRealized {
                    realization:
                        EventRealization::People {
                            selected_person_ids,
                            requested_count,
                            realized_count,
                            warning,
                        },
                    ..
                } => Some((
                    selected_person_ids,
                    requested_count,
                    realized_count,
                    warning,
                )),
                _ => None,
            })
            .unwrap();
        assert_eq!(*realization.1, 3);
        assert_eq!(*realization.2, 2);
        assert_eq!(realization.0.len(), 2);
        assert!(matches!(
            realization.3,
            Some(chargen_domain::EventWarning::ExactCountClamped {
                requested_count: 3,
                eligible_count: 2,
                realized_count: 2,
            })
        ));
    }

    #[test]
    fn overlapping_death_events_follow_priority_without_double_deaths() {
        let mut raw = event_scenario();
        raw.scheduled_events = Some(vec![
            scheduled_event(
                904,
                "lower priority",
                Year(20),
                -1,
                vec![EventEffect::Deaths {
                    selection: EventSelection::Probabilistic {
                        probability_ppm: 1_000_000,
                    },
                }],
            ),
            scheduled_event(
                905,
                "higher priority",
                Year(20),
                5,
                vec![EventEffect::Deaths {
                    selection: EventSelection::Probabilistic {
                        probability_ppm: 1_000_000,
                    },
                }],
            ),
        ]);
        let mut engine = Engine::from_scenario(raw.validate().unwrap()).unwrap();
        let commit = engine.advance_year(None).unwrap();
        let scheduled_ids: Vec<_> = commit
            .events
            .iter()
            .filter_map(|event| match event.kind {
                HistoricalEventKind::ScheduledEventOccurred { event_id, .. } => Some(event_id),
                _ => None,
            })
            .collect();
        assert_eq!(scheduled_ids, vec![LogicalId(905), LogicalId(904)]);
        let mut died = BTreeSet::new();
        for event in &commit.events {
            if let HistoricalEventKind::PersonDied { person_id, .. } = event.kind {
                assert!(
                    died.insert(person_id),
                    "person {person_id:?} died more than once"
                );
            }
        }
        assert_eq!(died, [PersonId(1), PersonId(2)].into_iter().collect());
        let lower = commit.events.iter().find_map(|event| match &event.kind {
            HistoricalEventKind::EventEffectRealized {
                event_id: LogicalId(904),
                realization: EventRealization::People { realized_count, .. },
                ..
            } => Some(*realized_count),
            _ => None,
        });
        assert_eq!(lower, Some(0));
    }

    #[test]
    fn event_displacement_expands_household_and_records_the_actual_route() {
        let mut raw = event_scenario();
        raw.territory.as_mut().unwrap().migration.unit = MigrationUnit::Household;
        raw.scheduled_events = Some(vec![scheduled_event(
            906,
            "evacuation",
            Year(20),
            0,
            vec![EventEffect::Displacement {
                selection: EventSelection::Exact {
                    count: 1,
                    overflow: ExactOverflowPolicy::Error,
                },
                destination: SettlementId(2),
            }],
        )]);
        let mut engine = Engine::from_scenario(raw.validate().unwrap()).unwrap();
        let commit = engine.advance_year(None).unwrap();
        let realization = commit
            .events
            .iter()
            .find_map(|event| match &event.kind {
                HistoricalEventKind::EventEffectRealized {
                    realization:
                        EventRealization::Displacement {
                            requested_count,
                            realized_count,
                            requested_person_ids,
                            moved_person_ids,
                            moves,
                            ..
                        },
                    ..
                } => Some((
                    *requested_count,
                    *realized_count,
                    requested_person_ids,
                    moved_person_ids,
                    moves,
                    &event.affected_entities,
                )),
                _ => None,
            })
            .unwrap();
        assert_eq!((realization.0, realization.1), (1, 2));
        assert_eq!(realization.2.len(), 1);
        assert_eq!(realization.3.len(), 2);
        assert_eq!(realization.4[0].route_ids, vec![LogicalId(900)]);
        assert!(realization
            .5
            .contains(&EntityRef::World(chargen_domain::WorldId(1))));
        assert!(commit.events.iter().any(|event| matches!(
            event.kind,
            HistoricalEventKind::ResidentsMoved {
                reason: chargen_domain::MovementReason::ForcedDisplacement,
                ..
            }
        )));
    }

    #[test]
    fn closed_routes_and_prior_event_displacements_share_capacity() {
        let mut closed = event_scenario();
        closed.scheduled_events = Some(vec![scheduled_event(
            907,
            "closed crossing",
            Year(20),
            0,
            vec![
                EventEffect::RouteClosure {
                    route_ids: [LogicalId(900)].into_iter().collect(),
                },
                EventEffect::Displacement {
                    selection: EventSelection::Exact {
                        count: 1,
                        overflow: ExactOverflowPolicy::Error,
                    },
                    destination: SettlementId(2),
                },
            ],
        )]);
        let mut engine = Engine::from_scenario(closed.validate().unwrap()).unwrap();
        let before = engine.state().clone();
        assert!(matches!(
            engine.advance_year(None),
            Err(EngineError::InvalidMigration(_))
        ));
        assert_eq!(engine.state(), &before);

        let mut capacity = event_scenario();
        capacity.territory.as_mut().unwrap().routes[0].capacity_per_year = Some(1);
        capacity.scheduled_events = Some(vec![scheduled_event(
            908,
            "competing convoys",
            Year(20),
            0,
            vec![
                EventEffect::Displacement {
                    selection: EventSelection::Exact {
                        count: 1,
                        overflow: ExactOverflowPolicy::Error,
                    },
                    destination: SettlementId(2),
                },
                EventEffect::Displacement {
                    selection: EventSelection::Exact {
                        count: 1,
                        overflow: ExactOverflowPolicy::Error,
                    },
                    destination: SettlementId(2),
                },
            ],
        )]);
        let mut engine = Engine::from_scenario(capacity.validate().unwrap()).unwrap();
        let before = engine.state().clone();
        assert!(matches!(
            engine.advance_year(None),
            Err(EngineError::InvalidMigration(_))
        ));
        assert_eq!(engine.state(), &before);
    }

    #[test]
    fn incapacity_and_production_losses_compose_for_fixed_interval_then_expire() {
        let farmer = ProfessionId::from("farmer");
        let mut restriction = scheduled_event(
            909,
            "harvest emergency",
            Year(20),
            0,
            vec![
                EventEffect::WorkIncapacity {
                    selection: EventSelection::Exact {
                        count: 1,
                        overflow: ExactOverflowPolicy::Error,
                    },
                },
                EventEffect::ProductionLoss { loss_ppm: 500_000 },
            ],
        );
        restriction.population_filter.profession_ids = Some([farmer].into_iter().collect());
        let mut engine = Engine::from_scenario(
            productive_event_scenario(vec![restriction])
                .validate()
                .unwrap(),
        )
        .unwrap();
        let first = engine.advance_year(None).unwrap();
        let grain = GoodId::from("grain");
        let group_id = chargen_domain::EconomicGroupId {
            settlement_id: SettlementId(1),
            profession_id: Some(ProfessionId::from("farmer")),
        };
        let produced = |commit: &YearCommit| {
            commit
                .state
                .economy
                .last_year_report
                .as_ref()
                .unwrap()
                .groups
                .iter()
                .find(|group| group.group_id == group_id)
                .unwrap()
                .goods[&grain]
                .produced
                .0
        };
        assert_eq!(produced(&first), 5);
        assert_eq!(first.state.event_effects.len(), 2);
        let second = engine.advance_year(None).unwrap();
        assert!(second.state.event_effects.is_empty());
        assert_eq!(produced(&second), 20);
    }

    #[test]
    fn scarcity_response_uses_prior_close_for_mortality_and_current_a6_for_births_and_migration() {
        let mut raw = event_scenario();
        raw.economy = Some(EconomicConfig {
            responses: EconomicResponsePolicy {
                mortality_increase_at_full_scarcity_ppm: 400_000,
                birth_reduction_at_full_scarcity_ppm: 400_000,
                migration_opportunity_at_full_scarcity: -80,
            },
            ..EconomicConfig::default()
        });
        let validated = raw.validate().unwrap();
        let mut state = validated.initial_state();
        let config = validated.scenario().economy.as_ref().unwrap();
        economy::initialize_economy(config, &mut state, Year(20)).unwrap();
        state.economy.last_year_report = Some(EconomicYearReport {
            year: Year(19),
            signals: EconomySignals {
                scarcity_ppm_by_settlement: [(SettlementId(1), 500_000)].into_iter().collect(),
                ..EconomySignals::default()
            },
            ..EconomicYearReport::default()
        });
        state.economy.working_report = Some(EconomicYearReport {
            year: Year(20),
            signals: EconomySignals {
                scarcity_ppm_by_settlement: [(SettlementId(1), 250_000)].into_iter().collect(),
                ..EconomySignals::default()
            },
            ..EconomicYearReport::default()
        });

        assert_eq!(
            economy::adjust_mortality_probability(config, &state, SettlementId(1), 100_000),
            300_000
        );
        assert_eq!(
            economy::current_birth_scarcity_reduction(config, &state, SettlementId(1)),
            100_000
        );
        assert_eq!(
            economy::current_migration_scarcity_adjustment(config, &state, SettlementId(1)),
            -20
        );
        assert_eq!(
            economy::migration_scarcity_adjustment(config, &state, SettlementId(1)),
            -40
        );
    }

    #[test]
    fn voluntary_migration_resolves_shared_route_and_destination_capacity_as_a_batch() {
        let mut raw = territorial_scenario();
        let territory = raw.territory.as_mut().unwrap();
        territory.routes[0].capacity_per_year = Some(1);
        territory.settlement_capacities.insert(SettlementId(2), 1);
        territory.opportunity_scores = [(SettlementId(1), 0), (SettlementId(2), 100)]
            .into_iter()
            .collect();
        territory.migration = MigrationPolicy {
            enabled: true,
            probability_ppm: 1_000_000,
            unit: MigrationUnit::Person,
            minimum_age: 0,
            maximum_age: None,
            candidate_limit: 1,
            max_moving_units_per_year: 8,
            maximum_travel_cost: None,
            minimum_net_opportunity_gain: 0,
        };
        let mut engine = Engine::from_scenario(raw.validate().unwrap()).unwrap();
        let commit = engine.advance_year(None).unwrap();

        assert_eq!(
            commit
                .state
                .living_people_at(Year(20))
                .filter(|person| person.settlement_id == SettlementId(2))
                .count(),
            1
        );
        assert_eq!(
            commit
                .events
                .iter()
                .filter(|event| matches!(&event.kind, HistoricalEventKind::ResidentsMoved { .. }))
                .count(),
            1
        );
        let metrics = engine.last_metrics().unwrap();
        let destination = metrics
            .census_balance
            .iter()
            .find(|balance| balance.settlement_id == SettlementId(2))
            .unwrap();
        assert_eq!(
            (destination.before, destination.incoming, destination.after),
            (0, 1, 1)
        );
        assert_eq!(
            metrics
                .census_balance
                .iter()
                .map(|balance| balance.incoming)
                .sum::<u64>(),
            metrics
                .census_balance
                .iter()
                .map(|balance| balance.outgoing)
                .sum::<u64>()
        );
        let moved = commit
            .events
            .iter()
            .find_map(|event| match &event.kind {
                HistoricalEventKind::ResidentsMoved { person_ids, .. } => person_ids.first(),
                _ => None,
            })
            .copied()
            .unwrap();
        assert_eq!(commit.state.persons[&moved].household_id, None);
        assert_eq!(commit.state.households[&HouseholdId(1)].members.len(), 1);
    }

    #[test]
    fn forced_household_displacement_moves_the_whole_home_atomically() {
        let mut raw = territorial_scenario();
        raw.territory.as_mut().unwrap().migration.unit = MigrationUnit::Household;
        let mut engine = Engine::from_scenario(raw.validate().unwrap()).unwrap();
        let command = ForcedDisplacementCommand {
            id: LogicalId(777),
            person_ids: [PersonId(1)].into_iter().collect(),
            destination: SettlementId(2),
        };
        let commit = engine.advance_year_with_commands(None, &[command]).unwrap();

        assert!(commit.state.persons.values().all(|person| {
            person.settlement_id == SettlementId(2) && person.household_id == Some(HouseholdId(1))
        }));
        assert_eq!(
            commit.state.households[&HouseholdId(1)].settlement_id,
            SettlementId(2)
        );
        assert!(commit.events.iter().any(|event| matches!(
            &event.kind,
            HistoricalEventKind::ResidentsMoved {
                person_ids,
                reason: chargen_domain::MovementReason::ForcedDisplacement,
                ..
            } if person_ids.len() == 2
        )));
        let source = engine
            .last_metrics()
            .unwrap()
            .census_balance
            .iter()
            .find(|balance| balance.settlement_id == SettlementId(1))
            .unwrap();
        assert_eq!((source.before, source.outgoing, source.after), (2, 2, 0));
    }

    #[test]
    fn invalid_forced_displacement_leaves_the_previous_close_unchanged() {
        let mut raw = territorial_scenario();
        let territory = raw.territory.as_mut().unwrap();
        territory.migration.unit = MigrationUnit::Household;
        territory.settlement_capacities.insert(SettlementId(2), 1);
        let mut engine = Engine::from_scenario(raw.validate().unwrap()).unwrap();
        let before = engine.state().clone();
        let command = ForcedDisplacementCommand {
            id: LogicalId(778),
            person_ids: [PersonId(1)].into_iter().collect(),
            destination: SettlementId(2),
        };

        assert!(matches!(
            engine.advance_year_with_commands(None, &[command]),
            Err(EngineError::InvalidMigration(_))
        ));
        assert_eq!(engine.state(), &before);
    }

    #[test]
    fn cancellation_after_forced_preparation_discards_the_pending_year() {
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

        struct CancelDuringA0 {
            calls: AtomicUsize,
            token: CancellationToken,
        }

        impl MetricsClock for CancelDuringA0 {
            fn now_nanos(&self) -> u128 {
                if self.calls.fetch_add(1, AtomicOrdering::SeqCst) + 1 == 3 {
                    self.token.cancel();
                }
                0
            }
        }

        let mut raw = territorial_scenario();
        raw.territory.as_mut().unwrap().migration.unit = MigrationUnit::Household;
        let validated = raw.validate().unwrap();
        let state = generate_initial_state(&validated).unwrap();
        let token = CancellationToken::new();
        let clock = Arc::new(CancelDuringA0 {
            calls: AtomicUsize::new(0),
            token: token.clone(),
        });
        let mut engine = Engine::new_with_clock(validated, state, clock).unwrap();
        let before = engine.state().clone();
        let command = ForcedDisplacementCommand {
            id: LogicalId(779),
            person_ids: [PersonId(1)].into_iter().collect(),
            destination: SettlementId(2),
        };

        assert!(matches!(
            engine.advance_year_with_commands(Some(&token), &[command]),
            Err(EngineError::Cancelled)
        ));
        assert_eq!(engine.state(), &before);
    }

    #[test]
    fn branch_revision_retires_an_unavailable_profession_at_the_first_new_close() {
        let mut raw = territorial_scenario();
        raw.initial_people[0].profession = Some(ProfessionId::from("farmer"));
        let territory = raw.territory.as_mut().unwrap();
        territory.professions.push(ProfessionDefinition {
            id: ProfessionId::from("farmer"),
            name: "Farmer".into(),
        });
        territory.profession_options_by_settlement.insert(
            SettlementId(1),
            [ProfessionId::from("farmer")].into_iter().collect(),
        );
        let validated = raw.validate().unwrap();
        let mut parent = Engine::from_scenario(validated).unwrap();
        let cut = parent.advance_year(None).unwrap().state;

        let mut revised = raw;
        revised
            .territory
            .as_mut()
            .unwrap()
            .profession_options_by_settlement
            .remove(&SettlementId(1));
        assert!(revised.validate().is_err());
        let revised = revised.validate_for_branch_revision().unwrap();
        let mut child = Engine::new_for_branch_revision(revised, cut).unwrap();
        let commit = child.advance_year(None).unwrap();

        assert_eq!(commit.year, Year(21));
        assert_eq!(commit.state.persons[&PersonId(1)].profession, None);
        assert!(commit.events.iter().any(|event| matches!(
            &event.kind,
            HistoricalEventKind::ProfessionChanged {
                person_id,
                from: Some(_),
                to: None,
                ..
            } if *person_id == PersonId(1)
        )));
    }

    #[test]
    fn identical_branch_cut_and_rules_produce_the_same_next_annual_commit() {
        let mut raw = territorial_scenario();
        raw.policies.mortality = MortalityPolicy {
            default_probability_ppm: 350_000,
            age_bands: Vec::new(),
        };
        let validated = raw.validate().unwrap();
        let mut continuing = Engine::from_scenario(validated.clone()).unwrap();
        let mut fork_source = Engine::from_scenario(validated.clone()).unwrap();
        assert_eq!(
            continuing.advance_year(None).unwrap(),
            fork_source.advance_year(None).unwrap()
        );

        let mut child =
            Engine::new_for_branch_revision(validated, fork_source.state().clone()).unwrap();
        assert_eq!(
            continuing.advance_year(None).unwrap(),
            child.advance_year(None).unwrap()
        );
    }

    #[test]
    fn keyed_rng_ignores_storage_id_and_is_versioned_by_logical_identity() {
        let a = keyed_random_u64(
            11,
            1,
            DecisionDomain::BirthChance,
            Year(12),
            LogicalId(44),
            0,
        );
        let b = keyed_random_u64(
            11,
            1,
            DecisionDomain::BirthChance,
            Year(12),
            LogicalId(44),
            0,
        );
        let c = keyed_random_u64(
            11,
            1,
            DecisionDomain::BirthChance,
            Year(12),
            LogicalId(45),
            0,
        );
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn births_record_known_parents_and_inherit_actual_allele_sources() {
        let mut raw = base_scenario();
        raw.genetics = one_locus_catalog();
        raw.policies.births = BirthPolicy {
            enabled: true,
            probability_ppm: 1_000_000,
            minimum_years_between_births: 0,
            minimum_gestational_age: 18,
            maximum_gestational_age: 45,
            require_active_union: true,
            required_contributor_gamete: Some(chargen_domain::Gamete::Sperm),
            allow_unknown_contributor: false,
            maximum_births_per_person_per_year: 1,
            infant_mortality_probability_ppm: 0,
            child_reproductive_profiles: vec![],
            minimum_contributor_age: 18,
        };
        let scenario = raw.validate().unwrap();
        let mut engine = Engine::from_scenario(scenario).unwrap();
        let commit = engine.advance_year(None).unwrap();
        let child_event = commit
            .events
            .iter()
            .find(|event| matches!(event.kind, HistoricalEventKind::PersonBorn { .. }))
            .unwrap();
        let HistoricalEventKind::PersonBorn {
            person_id,
            genetic_parents,
            ..
        } = &child_event.kind
        else {
            unreachable!()
        };
        assert_eq!(genetic_parents.len(), 2);
        let child = &commit.state.persons[person_id];
        let alleles = child.genome.loci.get(&LocusId::from("eye")).unwrap();
        assert!(alleles.iter().all(|allele| allele.inherited_from.is_some()));
        assert_eq!(
            alleles
                .iter()
                .filter_map(|allele| allele.inherited_from)
                .collect::<BTreeSet<_>>(),
            [LogicalId(101), LogicalId(102)].into_iter().collect()
        );
        assert_eq!(
            commit
                .state
                .family_links_at(*person_id, Year(20))
                .filter(|link| link.kind == FamilyLinkKind::BiologicalParent)
                .count(),
            2
        );
    }

    #[test]
    fn configured_gestation_gap_skips_year_one_and_allows_birth_in_year_two() {
        let mut raw = base_scenario();
        raw.policies.births = BirthPolicy {
            enabled: true,
            probability_ppm: 1_000_000,
            minimum_years_between_births: 2,
            minimum_gestational_age: 18,
            maximum_gestational_age: 45,
            require_active_union: true,
            required_contributor_gamete: Some(chargen_domain::Gamete::Sperm),
            allow_unknown_contributor: false,
            maximum_births_per_person_per_year: 1,
            infant_mortality_probability_ppm: 0,
            child_reproductive_profiles: vec![],
            minimum_contributor_age: 18,
        };
        let scenario = raw.validate().unwrap();
        let mut engine = Engine::from_scenario(scenario).unwrap();
        let year_20 = engine.advance_year(None).unwrap();
        let year_21 = engine.advance_year(None).unwrap();
        let year_22 = engine.advance_year(None).unwrap();

        let born_years = [&year_20, &year_21, &year_22]
            .into_iter()
            .flat_map(|commit| &commit.events)
            .filter(|event| matches!(event.kind, HistoricalEventKind::PersonBorn { .. }))
            .map(|event| event.year)
            .collect::<Vec<_>>();
        assert_eq!(born_years, [Year(20), Year(22)]);
        assert_eq!(
            year_22.state.last_gestational_birth_year[&PersonId(1)],
            Year(22)
        );
    }

    #[test]
    fn multiple_unions_are_allowed_by_config_and_pairs_are_bounded() {
        let mut raw = base_scenario();
        raw.initial_people.extend((3..14).map(|id| Person {
            id: PersonId(id),
            logical_id: LogicalId(100 + id),
            birth_year: Year(0),
            death_year: None,
            settlement_id: SettlementId(1),
            household_id: None,
            reproductive: ReproductiveProfile::default(),
            traits: BTreeMap::new(),
            genome: Genome::default(),
            last_widowed_year: None,
            profession: None,
        }));
        raw.policies.unions = UnionPolicy {
            enabled: true,
            minimum_age: 18,
            maximum_age: None,
            formation_probability_ppm: 1_000_000,
            separation_probability_ppm: 0,
            candidate_limit: 2,
            formation_attempts_per_person: 2,
            max_active_unions_per_person: None,
            prohibited_kinship_generations: 0,
            allow_widowed_remarriage: true,
            kind: UnionKind::Partnership,
        };
        let scenario = raw.validate().unwrap();
        let mut engine = Engine::from_scenario(scenario).unwrap();
        let commit = engine.advance_year(None).unwrap();
        assert!(commit.state.unions.len() >= 2);
        assert!(
            commit
                .events
                .iter()
                .filter(|event| matches!(event.kind, HistoricalEventKind::UnionStarted { .. }))
                .count()
                > 0
        );
    }

    #[test]
    fn death_closes_unions_and_does_not_count_the_person_at_year_close() {
        let mut raw = base_scenario();
        raw.policies.mortality = MortalityPolicy {
            default_probability_ppm: 1_000_000,
            age_bands: vec![],
        };
        let scenario = raw.validate().unwrap();
        let mut engine = Engine::from_scenario(scenario).unwrap();
        let commit = engine.advance_year(None).unwrap();
        assert_eq!(commit.state.living_people_at(Year(20)).count(), 0);
        assert_eq!(commit.state.unions_at(PersonId(1), Year(20)).count(), 0);
        assert!(commit.events.iter().any(|event| matches!(
            event.kind,
            HistoricalEventKind::UnionEnded {
                reason: UnionEndReason::Widowhood(_),
                ..
            }
        )));
        let balance = &engine.last_metrics().unwrap().census_balance[0];
        assert_eq!(
            (
                balance.before,
                balance.births,
                balance.deaths,
                balance.after
            ),
            (2, 0, 2, 0)
        );
    }

    #[test]
    fn unknown_contributors_and_same_year_infant_death_are_recorded_honestly() {
        let mut raw = base_scenario();
        raw.initial_people[1].reproductive = ReproductiveProfile {
            gametes: BTreeSet::new(),
            can_gestate: false,
            fertility_enabled: true,
        };
        raw.policies.births = BirthPolicy {
            enabled: true,
            probability_ppm: 1_000_000,
            minimum_years_between_births: 0,
            minimum_gestational_age: 18,
            maximum_gestational_age: 45,
            require_active_union: false,
            required_contributor_gamete: Some(chargen_domain::Gamete::Sperm),
            allow_unknown_contributor: true,
            maximum_births_per_person_per_year: 1,
            infant_mortality_probability_ppm: 1_000_000,
            child_reproductive_profiles: vec![],
            minimum_contributor_age: 18,
        };
        let scenario = raw.validate().unwrap();
        let mut engine = Engine::from_scenario(scenario).unwrap();
        let commit = engine.advance_year(None).unwrap();
        let born = commit
            .events
            .iter()
            .find_map(|event| match &event.kind {
                HistoricalEventKind::PersonBorn {
                    person_id,
                    genetic_parents,
                    unknown_genetic_contributors,
                    ..
                } if *unknown_genetic_contributors == 1 && genetic_parents == &[PersonId(1)] => {
                    Some(*person_id)
                }
                _ => None,
            })
            .expect("birth should record one known and one unknown contributor");
        assert!(commit.events.iter().any(|event| matches!(event.kind, HistoricalEventKind::PersonDied { person_id, cause: DeathCause::InfantMortality } if person_id == born)));
        let balance = &engine.last_metrics().unwrap().census_balance[0];
        assert_eq!(
            (
                balance.before,
                balance.births,
                balance.deaths,
                balance.after
            ),
            (2, 1, 1, 2)
        );
    }

    #[test]
    fn caregiver_links_end_when_either_endpoint_dies_but_genealogy_remains() {
        let mut raw = base_scenario();
        raw.initial_unions.clear();
        raw.initial_family_links.push(FamilyLink {
            id: FamilyLinkId(1),
            logical_id: LogicalId(77),
            parent: PersonId(1),
            child: PersonId(2),
            kind: FamilyLinkKind::Caregiver,
            validity: YearInterval {
                start: Year(10),
                end_exclusive: None,
            },
        });
        raw.policies.mortality = MortalityPolicy {
            default_probability_ppm: 1_000_000,
            age_bands: vec![],
        };
        let scenario = raw.validate().unwrap();
        let mut engine = Engine::from_scenario(scenario).unwrap();
        let commit = engine.advance_year(None).unwrap();
        assert_eq!(
            commit.state.family_links[&FamilyLinkId(1)]
                .validity
                .end_exclusive,
            Some(Year(20))
        );
        assert!(commit.events.iter().any(|event| matches!(
            event.kind,
            HistoricalEventKind::FamilyLinkEnded {
                link_id: FamilyLinkId(1),
                ..
            }
        )));
    }

    #[test]
    fn configured_initial_profiles_generate_deterministic_ages_households_and_genomes() {
        let mut raw = base_scenario();
        raw.initial_population_profiles
            .push(InitialPopulationProfile {
                settlement_id: SettlementId(1),
                population_size: 10,
                household_size_minimum: 2,
                household_size_maximum: 3,
                household_kind: HouseholdKind::Collective,
                age_bands: vec![WeightedAgeBand {
                    minimum_age: 20,
                    maximum_age: 50,
                    weight: 1,
                }],
                reproductive_profiles: vec![WeightedReproductiveProfile {
                    profile: profile(false, Some(chargen_domain::Gamete::Sperm)),
                    weight: 1,
                }],
                profession_distribution: Vec::new(),
                trait_distributions: BTreeMap::new(),
            });
        raw.genetics = one_locus_catalog();
        let scenario = raw.validate().unwrap();
        let first = generate_initial_state(&scenario).unwrap();
        let second = generate_initial_state(&scenario).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.persons.len(), 12);
        assert!(first
            .persons
            .values()
            .all(|person| person.genome.loci.contains_key(&LocusId::from("eye"))));
        assert!(first
            .households
            .values()
            .any(|household| household.kind == HouseholdKind::Collective));
    }

    #[test]
    fn metrics_are_observational_and_do_not_change_deterministic_commits() {
        let validated = base_scenario().validate().unwrap();
        let mut first = Engine::from_scenario(validated.clone()).unwrap();
        let mut second = Engine::from_scenario(validated).unwrap();
        let first_commit = first.advance_year(None).unwrap();
        let second_commit = second.advance_year(None).unwrap();
        assert_eq!(first_commit, second_commit);
        let metrics = first.last_metrics().unwrap();
        assert_eq!(metrics.year, Year(20));
        assert_eq!(metrics.phases.len(), YearPhase::ALL.len());
        assert_eq!(
            metrics.census_balance[0].before + metrics.census_balance[0].births
                - metrics.census_balance[0].deaths,
            metrics.census_balance[0].after
        );
    }

    #[test]
    fn union_candidate_selection_rejects_known_siblings() {
        let mut raw = base_scenario();
        raw.initial_unions.clear();
        raw.initial_people.push(Person {
            id: PersonId(3),
            logical_id: LogicalId(103),
            birth_year: Year(-30),
            death_year: None,
            settlement_id: SettlementId(1),
            household_id: None,
            reproductive: ReproductiveProfile::default(),
            traits: BTreeMap::new(),
            genome: Genome::default(),
            last_widowed_year: None,
            profession: None,
        });
        raw.initial_family_links = vec![
            FamilyLink {
                id: FamilyLinkId(1),
                logical_id: LogicalId(201),
                parent: PersonId(3),
                child: PersonId(1),
                kind: FamilyLinkKind::BiologicalParent,
                validity: YearInterval {
                    start: Year(0),
                    end_exclusive: None,
                },
            },
            FamilyLink {
                id: FamilyLinkId(2),
                logical_id: LogicalId(202),
                parent: PersonId(3),
                child: PersonId(2),
                kind: FamilyLinkKind::BiologicalParent,
                validity: YearInterval {
                    start: Year(0),
                    end_exclusive: None,
                },
            },
        ];
        raw.policies.unions = UnionPolicy {
            enabled: true,
            minimum_age: 18,
            maximum_age: None,
            formation_probability_ppm: 1_000_000,
            separation_probability_ppm: 0,
            candidate_limit: 8,
            formation_attempts_per_person: 1,
            max_active_unions_per_person: Some(1),
            prohibited_kinship_generations: 2,
            allow_widowed_remarriage: true,
            kind: UnionKind::Partnership,
        };
        let mut engine = Engine::from_scenario(raw.validate().unwrap()).unwrap();
        let commit = engine.advance_year(None).unwrap();
        assert!(!commit
            .events
            .iter()
            .any(|event| matches!(event.kind, HistoricalEventKind::UnionStarted { .. })));
    }

    #[test]
    fn orphan_adoption_moves_the_household_and_adds_caregivers_next_year() {
        let mut raw = base_scenario();
        let child_id = PersonId(3);
        let orphan_household_id = HouseholdId(2);
        raw.initial_people.push(Person {
            id: child_id,
            logical_id: LogicalId(103),
            birth_year: Year(15),
            death_year: None,
            settlement_id: SettlementId(1),
            household_id: Some(orphan_household_id),
            reproductive: ReproductiveProfile::default(),
            traits: BTreeMap::new(),
            genome: Genome::default(),
            last_widowed_year: None,
            profession: None,
        });
        raw.initial_households.push(Household {
            id: orphan_household_id,
            logical_id: LogicalId(3),
            kind: HouseholdKind::SinglePerson,
            settlement_id: SettlementId(1),
            members: [child_id].into_iter().collect(),
            closed_year: None,
        });
        raw.policies.adoption = AdoptionPolicy {
            enabled: true,
            adopt_orphans: true,
            adoption_probability_ppm: 1_000_000,
            add_household_care_links: true,
            ..AdoptionPolicy::default()
        };
        let mut engine = Engine::from_scenario(raw.validate().unwrap()).unwrap();
        let adoption_year = engine.advance_year(None).unwrap();
        assert_eq!(
            adoption_year.state.persons[&child_id].household_id,
            Some(HouseholdId(1))
        );
        assert_eq!(
            adoption_year.state.households[&orphan_household_id].closed_year,
            Some(Year(20))
        );
        assert_eq!(
            adoption_year
                .state
                .family_links_at(child_id, Year(20))
                .filter(|link| link.kind == FamilyLinkKind::AdoptiveParent)
                .count(),
            1
        );
        let care_year = engine.advance_year(None).unwrap();
        assert_eq!(
            care_year
                .state
                .family_links_at(child_id, Year(21))
                .filter(|link| link.kind == FamilyLinkKind::Caregiver)
                .count(),
            2
        );
    }

    #[test]
    fn a_widowed_person_can_remarry_when_policy_allows_it() {
        let mut raw = base_scenario();
        raw.initial_people[1].birth_year = Year(-100);
        raw.initial_people.push(Person {
            id: PersonId(3),
            logical_id: LogicalId(103),
            birth_year: Year(0),
            death_year: None,
            settlement_id: SettlementId(1),
            household_id: None,
            reproductive: ReproductiveProfile::default(),
            traits: BTreeMap::new(),
            genome: Genome::default(),
            last_widowed_year: None,
            profession: None,
        });
        raw.policies.mortality = MortalityPolicy {
            default_probability_ppm: 0,
            age_bands: vec![
                AgeMortalityBand {
                    minimum_age: 0,
                    maximum_age: Some(100),
                    probability_ppm: 0,
                },
                AgeMortalityBand {
                    minimum_age: 101,
                    maximum_age: None,
                    probability_ppm: 1_000_000,
                },
            ],
        };
        raw.policies.unions = UnionPolicy {
            enabled: true,
            minimum_age: 18,
            maximum_age: None,
            formation_probability_ppm: 1_000_000,
            separation_probability_ppm: 0,
            candidate_limit: 8,
            formation_attempts_per_person: 1,
            max_active_unions_per_person: Some(1),
            prohibited_kinship_generations: 0,
            allow_widowed_remarriage: true,
            kind: UnionKind::Marriage,
        };
        let mut engine = Engine::from_scenario(raw.validate().unwrap()).unwrap();
        let commit = engine.advance_year(None).unwrap();
        assert_eq!(
            commit.state.persons[&PersonId(1)].last_widowed_year,
            Some(Year(20))
        );
        assert!(commit.events.iter().any(|event| matches!(&event.kind, HistoricalEventKind::UnionStarted { participants, .. } if participants.contains(&PersonId(1)) && participants.contains(&PersonId(3)))));
    }
}
