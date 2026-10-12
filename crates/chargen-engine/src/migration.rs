//! Directed route planning and deterministic batch resolution for ruleset 2.

use crate::{emit_event, keyed_random_u64, random_below, DecisionDomain, EngineError};
use chargen_domain::{
    DirectedRoute, EntityRef, ForcedDisplacementCommand, HistoricalEvent, HistoricalEventKind,
    LogicalId, MigrationUnit, MovementReason, PersonId, ProfessionChangeReason, ResidenceInterval,
    SettlementId, State, TerritoryConfig, ValidatedScenario, Year, YearInterval,
};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RoutePlan {
    pub settlements: Vec<SettlementId>,
    pub route_indexes: Vec<usize>,
    pub cost: u64,
}

type RouteFingerprint = Vec<(LogicalId, SettlementId, SettlementId, u32, Option<u32>)>;
type RouteCacheKey = (u64, RouteFingerprint);

#[derive(Default)]
pub(crate) struct RouteCache {
    key: Option<RouteCacheKey>,
    plans_by_origin: BTreeMap<SettlementId, BTreeMap<SettlementId, RoutePlan>>,
}

impl RouteCache {
    fn ensure_plans(
        &mut self,
        state: &State,
        config: &TerritoryConfig,
        year: Year,
        origin: SettlementId,
    ) {
        let mut active_routes: Vec<_> = config
            .routes
            .iter()
            .filter(|route| {
                route_is_open(route, year) && !state.route_is_closed(route.logical_id, year)
            })
            .map(|route| {
                (
                    route.logical_id,
                    route.from,
                    route.to,
                    route.cost,
                    route.capacity_per_year,
                )
            })
            .collect();
        active_routes.sort_unstable_by_key(|route| route.0);
        let key = (config.route_revision, active_routes);
        if self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.plans_by_origin.clear();
        }
        self.plans_by_origin
            .entry(origin)
            .or_insert_with(|| shortest_paths_from(state, config, year, origin));
    }

    fn plan_to(
        &mut self,
        state: &State,
        config: &TerritoryConfig,
        year: Year,
        origin: SettlementId,
        destination: SettlementId,
    ) -> Option<RoutePlan> {
        self.ensure_plans(state, config, year, origin);
        self.plans_by_origin
            .get(&origin)
            .and_then(|plans| plans.get(&destination))
            .cloned()
    }
}

#[derive(Clone, Debug)]
pub(super) struct MoveGroup {
    pub(super) people: Vec<PersonId>,
    pub(super) from: SettlementId,
    pub(super) to: SettlementId,
    pub(super) route_ids: Vec<LogicalId>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct MigrationUsage {
    pub(super) route_people: BTreeMap<LogicalId, u64>,
    pub(super) moved_people: BTreeSet<PersonId>,
    pub(super) moving_units: u32,
}

impl MigrationUsage {
    pub(super) fn has_moved_person(&self, person_id: PersonId) -> bool {
        self.moved_people.contains(&person_id)
    }
}

#[derive(Clone, Debug)]
struct Proposal {
    logical_id: LogicalId,
    people: Vec<PersonId>,
    from: SettlementId,
    to: SettlementId,
    route: RoutePlan,
    priority: u64,
}

#[derive(Clone, Copy, Debug)]
struct CandidateDestination {
    settlement: SettlementId,
    logical_id: LogicalId,
    cost: u64,
    gain: i128,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct HeapNode {
    cost: u64,
    route_keys: Vec<LogicalId>,
    settlement_keys: Vec<LogicalId>,
    settlement: SettlementId,
    route_indexes: Vec<usize>,
    settlements: Vec<SettlementId>,
}

pub(crate) fn scheduled_commands(
    territory: &TerritoryConfig,
    year: Year,
) -> Vec<ForcedDisplacementCommand> {
    territory
        .scheduled_displacements
        .iter()
        .filter(|entry| entry.year == year)
        .map(|entry| entry.command.clone())
        .collect()
}

/// Validate and prepare all forced moves before any phase mutates the pending state.
pub(crate) fn prepare_forced_displacements(
    scenario: &ValidatedScenario,
    state: &State,
    territory: &TerritoryConfig,
    year: Year,
    explicit: &[ForcedDisplacementCommand],
    cache: &mut RouteCache,
) -> Result<(Vec<MoveGroup>, MigrationUsage), EngineError> {
    let mut commands = scheduled_commands(territory, year);
    commands.extend_from_slice(explicit);
    prepare_displacement_commands(
        scenario,
        state,
        territory,
        year,
        commands,
        MigrationUsage::default(),
        cache,
    )
}

/// Prepare one event displacement against the state left by preceding A1 shocks. Existing route
/// and resident usage is carried forward so superimposed events cannot reuse capacity or people.
pub(crate) fn prepare_event_displacement(
    scenario: &ValidatedScenario,
    state: &State,
    territory: &TerritoryConfig,
    year: Year,
    command: &ForcedDisplacementCommand,
    prior_usage: &MigrationUsage,
    cache: &mut RouteCache,
) -> Result<(Vec<MoveGroup>, MigrationUsage), EngineError> {
    prepare_displacement_commands(
        scenario,
        state,
        territory,
        year,
        vec![command.clone()],
        prior_usage.clone(),
        cache,
    )
}

fn prepare_displacement_commands(
    scenario: &ValidatedScenario,
    state: &State,
    territory: &TerritoryConfig,
    year: Year,
    mut commands: Vec<ForcedDisplacementCommand>,
    mut usage: MigrationUsage,
    cache: &mut RouteCache,
) -> Result<(Vec<MoveGroup>, MigrationUsage), EngineError> {
    if commands.is_empty() {
        return Ok((Vec::new(), usage));
    }

    let seed = scenario.scenario().seed;
    let rules = scenario.scenario().rules_version;
    let mut ids = BTreeSet::new();
    for command in &commands {
        if !ids.insert(command.id) {
            return Err(invalid_migration(
                "duplicate forced displacement command ID",
            ));
        }
        if command.person_ids.is_empty() {
            return Err(invalid_migration("forced displacement selection is empty"));
        }
        if !state.settlements.contains_key(&command.destination) {
            return Err(invalid_migration(
                "forced displacement destination is unknown",
            ));
        }
    }

    commands.sort_by_key(|command| {
        (
            keyed_random_u64(
                seed,
                rules,
                DecisionDomain::ForcedDisplacementPriority,
                year,
                command.id,
                0,
            ),
            command.id,
        )
    });

    let mut occupancy = census_at_start(state, year);
    let mut moved_households: BTreeSet<_> = if territory.migration.unit == MigrationUnit::Household
    {
        usage
            .moved_people
            .iter()
            .filter_map(|person_id| state.persons.get(person_id)?.household_id)
            .collect()
    } else {
        BTreeSet::new()
    };
    let mut groups = Vec::new();
    for command in commands {
        let mut people = command.person_ids.clone();
        let selected_households: BTreeSet<_> = people
            .iter()
            .filter_map(|person_id| state.persons.get(person_id)?.household_id)
            .collect();
        if territory.migration.unit == MigrationUnit::Household {
            if selected_households
                .iter()
                .any(|household_id| moved_households.contains(household_id))
            {
                return Err(invalid_migration(
                    "two forced commands select residents of the same household",
                ));
            }
            moved_households.extend(selected_households.iter().copied());
        }
        if territory.migration.unit == MigrationUnit::Household {
            for household_id in selected_households {
                let household = state
                    .households
                    .get(&household_id)
                    .ok_or_else(|| invalid_migration("person references missing household"))?;
                people.extend(household.members.iter().copied().filter(|person_id| {
                    state
                        .persons
                        .get(person_id)
                        .is_some_and(|person| person.alive_at_start(year))
                }));
            }
        }
        let mut by_origin: BTreeMap<SettlementId, Vec<PersonId>> = BTreeMap::new();
        for person_id in people {
            if usage.moved_people.contains(&person_id) {
                return Err(invalid_migration(
                    "a resident appears in more than one forced command",
                ));
            }
            let person = state
                .persons
                .get(&person_id)
                .filter(|person| person.alive_at_start(year))
                .ok_or_else(|| invalid_migration("forced command selects a nonliving person"))?;
            if person.settlement_id == command.destination {
                return Err(invalid_migration(
                    "forced displacement destination is already the person's residence",
                ));
            }
            by_origin
                .entry(person.settlement_id)
                .or_default()
                .push(person_id);
        }
        if by_origin.is_empty() {
            return Err(invalid_migration(
                "forced displacement has no living residents",
            ));
        }
        let mut command_groups = Vec::new();
        let mut command_route_people = BTreeMap::<LogicalId, u64>::new();
        let mut total_people = 0_u64;
        for (from, mut resident_ids) in by_origin {
            resident_ids.sort_by_key(|id| state.persons[id].logical_id);
            let route = cache
                .plan_to(state, territory, year, from, command.destination)
                .ok_or_else(|| {
                    invalid_migration("no directed route is open for the forced displacement")
                })?;
            if let Some(maximum) = territory.migration.maximum_travel_cost {
                if route.cost > u64::from(maximum) {
                    return Err(invalid_migration(
                        "forced displacement exceeds maximum travel cost",
                    ));
                }
            }
            let resident_count = u64::try_from(resident_ids.len())
                .map_err(|_| invalid_migration("forced displacement is too large"))?;
            for route_index in &route.route_indexes {
                let route_def = &territory.routes[*route_index];
                let logical_id = route_def.logical_id;
                let used = usage
                    .route_people
                    .get(&logical_id)
                    .copied()
                    .unwrap_or(0)
                    .checked_add(command_route_people.get(&logical_id).copied().unwrap_or(0))
                    .ok_or_else(|| invalid_migration("route capacity arithmetic overflow"))?;
                let attempted = used
                    .checked_add(resident_count)
                    .ok_or_else(|| invalid_migration("route capacity arithmetic overflow"))?;
                if route_def
                    .capacity_per_year
                    .is_some_and(|capacity| attempted > u64::from(capacity))
                {
                    return Err(invalid_migration(
                        "forced displacement exceeds route capacity",
                    ));
                }
                let planned = command_route_people.entry(logical_id).or_default();
                *planned = planned
                    .checked_add(resident_count)
                    .ok_or_else(|| invalid_migration("route capacity arithmetic overflow"))?;
            }
            total_people = total_people
                .checked_add(resident_count)
                .ok_or_else(|| invalid_migration("forced displacement is too large"))?;
            command_groups.push(MoveGroup {
                people: resident_ids,
                from,
                to: command.destination,
                route_ids: route
                    .route_indexes
                    .iter()
                    .map(|index| territory.routes[*index].logical_id)
                    .collect(),
            });
        }
        let destination_current = occupancy.get(&command.destination).copied().unwrap_or(0);
        let destination_incoming = total_people;
        let destination_final = destination_current
            .checked_add(destination_incoming)
            .ok_or_else(|| invalid_migration("forced displacement capacity arithmetic overflow"))?;
        if territory
            .settlement_capacities
            .get(&command.destination)
            .is_some_and(|capacity| destination_final > u64::from(*capacity))
        {
            return Err(invalid_migration(
                "forced displacement exceeds destination capacity",
            ));
        }
        for group in &command_groups {
            let count = occupancy.get(&group.from).copied().unwrap_or(0);
            occupancy.insert(
                group.from,
                count
                    .checked_sub(group.people.len() as u64)
                    .ok_or_else(|| invalid_migration("forced displacement census underflow"))?,
            );
        }
        occupancy.insert(command.destination, destination_final);
        usage.moving_units = usage.moving_units.saturating_add(1);
        usage.moved_people.extend(
            command_groups
                .iter()
                .flat_map(|group| group.people.iter().copied()),
        );
        for (route_id, count) in command_route_people {
            let total = usage.route_people.entry(route_id).or_default();
            *total = total
                .checked_add(count)
                .ok_or_else(|| invalid_migration("route capacity arithmetic overflow"))?;
        }
        groups.extend(command_groups);
    }
    Ok((groups, usage))
}

pub(crate) fn apply_forced_displacements(
    scenario: &ValidatedScenario,
    state: &mut State,
    territory: &TerritoryConfig,
    year: Year,
    groups: &[MoveGroup],
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    apply_move_groups(
        scenario,
        state,
        territory,
        year,
        groups,
        MovementReason::ForcedDisplacement,
        events,
    )
}

pub(crate) fn apply_voluntary_migration(
    scenario: &ValidatedScenario,
    state: &mut State,
    territory: &TerritoryConfig,
    year: Year,
    cache: &mut RouteCache,
    usage: &mut MigrationUsage,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    let policy = &territory.migration;
    if !policy.enabled || policy.probability_ppm == 0 {
        return Ok(());
    }
    let units = migration_units(state, year, policy.unit, &usage.moved_people);
    let origins: BTreeSet<_> = units.iter().map(|unit| unit.from).collect();
    for origin in &origins {
        cache.ensure_plans(state, territory, year, *origin);
    }
    let mut candidates_by_origin = BTreeMap::new();
    for origin in origins {
        let Some(plans) = cache.plans_by_origin.get(&origin) else {
            continue;
        };
        let origin_score = territory
            .opportunity_scores
            .get(&origin)
            .copied()
            .unwrap_or(0);
        let mut candidates: Vec<_> = plans
            .iter()
            .filter_map(|(destination, route)| {
                if *destination == origin
                    || policy
                        .maximum_travel_cost
                        .is_some_and(|maximum| route.cost > u64::from(maximum))
                {
                    return None;
                }
                let destination_score = territory
                    .opportunity_scores
                    .get(destination)
                    .copied()
                    .unwrap_or(0);
                let gain = i128::from(destination_score)
                    - i128::from(origin_score)
                    - i128::from(route.cost);
                if gain < i128::from(policy.minimum_net_opportunity_gain) {
                    return None;
                }
                Some(CandidateDestination {
                    settlement: *destination,
                    logical_id: state.settlements[destination].logical_id,
                    cost: route.cost,
                    gain,
                })
            })
            .collect();
        candidates.sort_by(|left, right| {
            right
                .gain
                .cmp(&left.gain)
                .then_with(|| left.cost.cmp(&right.cost))
                .then_with(|| left.logical_id.cmp(&right.logical_id))
        });
        candidates.truncate(usize::from(policy.candidate_limit));
        candidates_by_origin.insert(origin, candidates);
    }

    let mut proposals = Vec::new();
    for unit in units {
        let age = unit
            .people
            .iter()
            .filter_map(|id| state.persons[id].age_at_close(year))
            .max()
            .unwrap_or(0);
        if age < u32::from(policy.minimum_age)
            || policy.maximum_age.is_some_and(|max| age > u32::from(max))
        {
            continue;
        }
        let Some(candidates) = candidates_by_origin.get(&unit.from) else {
            continue;
        };
        if candidates.is_empty()
            || !super::occurs(
                scenario.scenario().seed,
                scenario.scenario().rules_version,
                DecisionDomain::MigrationOpportunity,
                year,
                unit.logical_id,
                0,
                policy.probability_ppm,
            )
        {
            continue;
        }
        let chosen = random_below(
            scenario.scenario().seed,
            scenario.scenario().rules_version,
            DecisionDomain::MigrationDestination,
            year,
            unit.logical_id,
            0,
            candidates.len() as u64,
        ) as usize;
        let to = candidates[chosen].settlement;
        let route = cache.plans_by_origin[&unit.from][&to].clone();
        let priority = keyed_random_u64(
            scenario.scenario().seed,
            scenario.scenario().rules_version,
            DecisionDomain::MigrationPriority,
            year,
            unit.logical_id,
            0,
        );
        proposals.push(Proposal {
            logical_id: unit.logical_id,
            people: unit.people,
            from: unit.from,
            to,
            route,
            priority,
        });
    }
    proposals.sort_by_key(|proposal| (proposal.priority, proposal.logical_id));

    let mut counts = census_at_close(state, year);
    let mut accepted = Vec::new();
    for proposal in proposals {
        if usage.moving_units >= policy.max_moving_units_per_year {
            break;
        }
        let people_count = proposal.people.len() as u64;
        let projected_destination_count = counts
            .get(&proposal.to)
            .copied()
            .unwrap_or(0)
            .checked_add(people_count);
        let capacity_available = territory
            .settlement_capacities
            .get(&proposal.to)
            .is_none_or(|capacity| {
                projected_destination_count.is_some_and(|count| count <= u64::from(*capacity))
            });
        if !capacity_available {
            continue;
        }
        let route_available = proposal.route.route_indexes.iter().all(|index| {
            let route = &territory.routes[*index];
            route.capacity_per_year.is_none_or(|capacity| {
                usage
                    .route_people
                    .get(&route.logical_id)
                    .copied()
                    .unwrap_or(0)
                    .checked_add(people_count)
                    .is_some_and(|count| count <= u64::from(capacity))
            })
        });
        if !route_available {
            continue;
        }
        let from_count = counts.get(&proposal.from).copied().unwrap_or(0);
        let to_count = counts.get(&proposal.to).copied().unwrap_or(0);
        let Some(new_from) = from_count.checked_sub(people_count) else {
            return Err(invalid_migration("migration census underflow"));
        };
        let Some(new_to) = to_count.checked_add(people_count) else {
            return Err(invalid_migration("migration census overflow"));
        };
        counts.insert(proposal.from, new_from);
        counts.insert(proposal.to, new_to);
        for index in &proposal.route.route_indexes {
            let total = usage
                .route_people
                .entry(territory.routes[*index].logical_id)
                .or_default();
            *total = total
                .checked_add(people_count)
                .ok_or_else(|| invalid_migration("route capacity arithmetic overflow"))?;
        }
        usage.moving_units += 1;
        usage.moved_people.extend(proposal.people.iter().copied());
        accepted.push(MoveGroup {
            people: proposal.people,
            from: proposal.from,
            to: proposal.to,
            route_ids: proposal
                .route
                .route_indexes
                .iter()
                .map(|index| territory.routes[*index].logical_id)
                .collect(),
        });
    }
    apply_move_groups(
        scenario,
        state,
        territory,
        year,
        &accepted,
        MovementReason::Voluntary,
        events,
    )
}

#[derive(Clone, Debug)]
struct MigrationUnitRecord {
    logical_id: LogicalId,
    people: Vec<PersonId>,
    from: SettlementId,
}

fn migration_units(
    state: &State,
    year: Year,
    unit: MigrationUnit,
    moved: &BTreeSet<PersonId>,
) -> Vec<MigrationUnitRecord> {
    let mut result = Vec::new();
    match unit {
        MigrationUnit::Person => {
            for person in state.living_people_at(year) {
                if moved.contains(&person.id) {
                    continue;
                }
                result.push(MigrationUnitRecord {
                    logical_id: person.logical_id,
                    people: vec![person.id],
                    from: person.settlement_id,
                });
            }
        }
        MigrationUnit::Household => {
            for household in state
                .households
                .values()
                .filter(|household| household.closed_year.is_none())
            {
                let people: Vec<_> = household
                    .members
                    .iter()
                    .copied()
                    .filter(|person_id| {
                        !moved.contains(person_id)
                            && state
                                .persons
                                .get(person_id)
                                .is_some_and(|person| person.alive_at_close(year))
                    })
                    .collect();
                if people.is_empty() || people.len() != household.members.len() {
                    continue;
                }
                result.push(MigrationUnitRecord {
                    logical_id: household.logical_id,
                    people,
                    from: household.settlement_id,
                });
            }
        }
    }
    result.sort_by_key(|unit| unit.logical_id);
    result
}

fn apply_move_groups(
    scenario: &ValidatedScenario,
    state: &mut State,
    territory: &TerritoryConfig,
    year: Year,
    groups: &[MoveGroup],
    reason: MovementReason,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), EngineError> {
    let mut destinations = BTreeMap::new();
    for group in groups {
        for person_id in &group.people {
            if destinations
                .insert(*person_id, (group.from, group.to))
                .is_some()
            {
                return Err(invalid_migration("resident appears in two accepted moves"));
            }
        }
    }
    if destinations.is_empty() {
        return Ok(());
    }

    // A household remains intact only when all current members move together to one place.
    let touched_households: BTreeSet<_> = destinations
        .keys()
        .filter_map(|person_id| state.persons.get(person_id)?.household_id)
        .collect();
    let mut preserve_household = BTreeSet::new();
    for household_id in &touched_households {
        let Some(household) = state.households.get(household_id) else {
            return Err(invalid_migration("person references missing household"));
        };
        let destinations_for_members: BTreeSet<_> = household
            .members
            .iter()
            .filter_map(|member| destinations.get(member).map(|(_, to)| *to))
            .collect();
        if destinations_for_members.len() == 1
            && household
                .members
                .iter()
                .all(|member| destinations.contains_key(member))
        {
            preserve_household.insert(*household_id);
        }
    }

    // Update temporal and person records together. Moving individuals leave their former
    // household; the remaining household stays valid or closes when empty.
    for person_id in destinations.keys().copied().collect::<Vec<_>>() {
        let (from, to) = destinations[&person_id];
        let person = state
            .persons
            .get(&person_id)
            .cloned()
            .ok_or_else(|| invalid_migration("accepted move references missing person"))?;
        if person.settlement_id != from || from == to {
            return Err(invalid_migration(
                "accepted move origin does not match current state",
            ));
        }
        if let Some(history) = state.residences.get_mut(&person_id) {
            if let Some(active) = history
                .iter_mut()
                .rev()
                .find(|interval| interval.validity.end_exclusive.is_none())
            {
                active.validity.end_exclusive = Some(year);
            }
            history.push(ResidenceInterval {
                settlement_id: to,
                validity: YearInterval {
                    start: year,
                    end_exclusive: None,
                },
            });
        } else {
            return Err(invalid_migration("resident lacks a residence history"));
        }
        let person_mut = state
            .persons
            .get_mut(&person_id)
            .expect("person checked above");
        person_mut.settlement_id = to;

        if let Some(profession_id) = person_mut.profession.clone() {
            let offered = territory
                .profession_options_by_settlement
                .get(&to)
                .is_some_and(|options| options.contains(&profession_id));
            if !offered {
                person_mut.profession = None;
                if let Some(intervals) = state.profession_intervals.get_mut(&person_id) {
                    if let Some(active) = intervals
                        .iter_mut()
                        .rev()
                        .find(|interval| interval.validity.end_exclusive.is_none())
                    {
                        active.validity.end_exclusive = Some(year);
                    }
                }
                emit_event(
                    state,
                    events,
                    year,
                    person.logical_id,
                    vec![
                        EntityRef::Person(person_id),
                        EntityRef::Settlement(from),
                        EntityRef::Settlement(to),
                    ],
                    HistoricalEventKind::ProfessionChanged {
                        person_id,
                        from: Some(profession_id),
                        to: None,
                        reason: ProfessionChangeReason::DestinationDoesNotOfferProfession,
                    },
                );
            }
        }
    }

    for household_id in touched_households {
        let household = state
            .households
            .get(&household_id)
            .cloned()
            .expect("touched household was checked");
        if preserve_household.contains(&household_id) {
            let destination = destinations[household.members.iter().next().expect("nonempty")].1;
            state
                .households
                .get_mut(&household_id)
                .expect("household checked")
                .settlement_id = destination;
            continue;
        }
        for person_id in household
            .members
            .iter()
            .filter(|id| destinations.contains_key(id))
        {
            state
                .households
                .get_mut(&household_id)
                .expect("household checked")
                .members
                .remove(person_id);
            state
                .persons
                .get_mut(person_id)
                .expect("person checked")
                .household_id = None;
            emit_event(
                state,
                events,
                year,
                state.persons[person_id].logical_id,
                vec![
                    EntityRef::Person(*person_id),
                    EntityRef::Household(household_id),
                ],
                HistoricalEventKind::HouseholdChanged {
                    person_id: *person_id,
                    from: Some(household_id),
                    to: None,
                },
            );
        }
        if state.households[&household_id].members.is_empty() {
            state
                .households
                .get_mut(&household_id)
                .expect("household checked")
                .closed_year = Some(year);
        }
    }

    if let Some(culture) = &scenario.scenario().culture {
        for (person_id, (from, to)) in &destinations {
            super::culture::apply_population_delta(
                culture,
                state,
                year,
                super::culture::CulturePopulationDelta {
                    person_id: *person_id,
                    from: Some(*from),
                    to: Some(*to),
                },
                events,
            )
            .map_err(|error| EngineError::Culture(error.to_string()))?;
        }
    }

    // Group movement events by directed settlement pair, with people in stable logical order.
    let mut by_pair: BTreeMap<(SettlementId, SettlementId), Vec<PersonId>> = BTreeMap::new();
    for (person_id, pair) in destinations {
        by_pair.entry(pair).or_default().push(person_id);
    }
    for ((from, to), mut person_ids) in by_pair {
        person_ids.sort_by_key(|id| state.persons[id].logical_id);
        let logical_id = person_ids
            .first()
            .map(|id| state.persons[id].logical_id)
            .unwrap_or_default();
        emit_event(
            state,
            events,
            year,
            logical_id,
            std::iter::once(EntityRef::World(state.world_id))
                .chain(std::iter::once(EntityRef::Settlement(from)))
                .chain(std::iter::once(EntityRef::Settlement(to)))
                .chain(person_ids.iter().copied().map(EntityRef::Person))
                .collect(),
            HistoricalEventKind::ResidentsMoved {
                person_ids,
                from,
                to,
                reason,
            },
        );
    }
    Ok(())
}

fn census_at_start(state: &State, year: Year) -> BTreeMap<SettlementId, u64> {
    let mut counts: BTreeMap<_, _> = state.settlements.keys().map(|id| (*id, 0)).collect();
    for person in state
        .persons
        .values()
        .filter(|person| person.alive_at_start(year))
    {
        *counts.entry(person.settlement_id).or_default() += 1;
    }
    counts
}

fn census_at_close(state: &State, year: Year) -> BTreeMap<SettlementId, u64> {
    let mut counts: BTreeMap<_, _> = state.settlements.keys().map(|id| (*id, 0)).collect();
    for person in state.living_people_at(year) {
        *counts.entry(person.settlement_id).or_default() += 1;
    }
    counts
}

fn shortest_paths_from(
    state: &State,
    config: &TerritoryConfig,
    year: Year,
    origin: SettlementId,
) -> BTreeMap<SettlementId, RoutePlan> {
    let mut adjacency: BTreeMap<SettlementId, Vec<(usize, &DirectedRoute)>> = BTreeMap::new();
    for (index, route) in config.routes.iter().enumerate() {
        if route_is_open(route, year)
            && !state.route_is_closed(route.logical_id, year)
            && route.capacity_per_year != Some(0)
        {
            adjacency
                .entry(route.from)
                .or_default()
                .push((index, route));
        }
    }
    for routes in adjacency.values_mut() {
        routes.sort_by_key(|(_, route)| route.logical_id);
    }
    let Some(origin_logical) = state
        .settlements
        .get(&origin)
        .map(|settlement| settlement.logical_id)
    else {
        return BTreeMap::new();
    };
    let initial = HeapNode {
        cost: 0,
        route_keys: Vec::new(),
        settlement_keys: vec![origin_logical],
        settlement: origin,
        route_indexes: Vec::new(),
        settlements: vec![origin],
    };
    let mut frontier = BinaryHeap::from([Reverse(initial)]);
    let mut best: BTreeMap<SettlementId, (u64, Vec<LogicalId>, Vec<LogicalId>)> =
        BTreeMap::from([(origin, (0, Vec::new(), vec![origin_logical]))]);
    let mut result = BTreeMap::new();
    while let Some(Reverse(node)) = frontier.pop() {
        let node_key = (
            node.cost,
            node.route_keys.clone(),
            node.settlement_keys.clone(),
        );
        if best.get(&node.settlement) != Some(&node_key) {
            continue;
        }
        result.insert(
            node.settlement,
            RoutePlan {
                settlements: node.settlements.clone(),
                route_indexes: node.route_indexes.clone(),
                cost: node.cost,
            },
        );
        for (route_index, route) in adjacency.get(&node.settlement).into_iter().flatten() {
            let Some(next_cost) = node.cost.checked_add(u64::from(route.cost)) else {
                continue;
            };
            let Some(next_logical) = state
                .settlements
                .get(&route.to)
                .map(|settlement| settlement.logical_id)
            else {
                continue;
            };
            let mut route_keys = node.route_keys.clone();
            route_keys.push(route.logical_id);
            let mut settlement_keys = node.settlement_keys.clone();
            settlement_keys.push(next_logical);
            let key = (next_cost, route_keys.clone(), settlement_keys.clone());
            if best.get(&route.to).is_some_and(|current| current <= &key) {
                continue;
            }
            best.insert(route.to, key);
            let mut route_indexes = node.route_indexes.clone();
            route_indexes.push(*route_index);
            let mut settlements = node.settlements.clone();
            settlements.push(route.to);
            frontier.push(Reverse(HeapNode {
                cost: next_cost,
                route_keys,
                settlement_keys,
                settlement: route.to,
                route_indexes,
                settlements,
            }));
        }
    }
    result
}

fn route_is_open(route: &DirectedRoute, year: Year) -> bool {
    route.open_intervals.is_empty()
        || route
            .open_intervals
            .iter()
            .any(|interval| interval.contains(year))
}

fn invalid_migration(message: &str) -> EngineError {
    EngineError::InvalidMigration(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_paths_are_directed_and_choose_stable_lowest_cost_route() {
        use chargen_domain::{LogicalId, Settlement, UnionId};
        let state = State {
            world_id: chargen_domain::WorldId(1),
            rules_version: 2,
            seed: 1,
            start_year: Year(0),
            current_year: None,
            next_person_id: 1,
            next_household_id: 1,
            next_union_id: 1,
            next_family_link_id: 1,
            settlements: [
                (
                    SettlementId(1),
                    Settlement {
                        id: SettlementId(1),
                        logical_id: LogicalId(101),
                        name: "A".into(),
                        culture_values: BTreeMap::new(),
                    },
                ),
                (
                    SettlementId(2),
                    Settlement {
                        id: SettlementId(2),
                        logical_id: LogicalId(102),
                        name: "B".into(),
                        culture_values: BTreeMap::new(),
                    },
                ),
                (
                    SettlementId(3),
                    Settlement {
                        id: SettlementId(3),
                        logical_id: LogicalId(103),
                        name: "C".into(),
                        culture_values: BTreeMap::new(),
                    },
                ),
            ]
            .into_iter()
            .collect(),
            persons: BTreeMap::new(),
            households: BTreeMap::new(),
            unions: BTreeMap::<UnionId, _>::new(),
            family_links: BTreeMap::new(),
            last_gestational_birth_year: BTreeMap::new(),
            residences: BTreeMap::new(),
            profession_intervals: BTreeMap::new(),
            last_year_settlement_balances: BTreeMap::new(),
            culture: Default::default(),
            economy: chargen_domain::EconomyState::default(),
            event_effects: Vec::new(),
            last_year_phases: Vec::new(),
        };
        let config = TerritoryConfig {
            routes: vec![
                DirectedRoute {
                    logical_id: LogicalId(1),
                    from: SettlementId(1),
                    to: SettlementId(2),
                    cost: 0,
                    capacity_per_year: Some(0),
                    open_intervals: vec![],
                },
                DirectedRoute {
                    logical_id: LogicalId(9),
                    from: SettlementId(1),
                    to: SettlementId(2),
                    cost: 7,
                    capacity_per_year: None,
                    open_intervals: vec![],
                },
                DirectedRoute {
                    logical_id: LogicalId(3),
                    from: SettlementId(1),
                    to: SettlementId(3),
                    cost: 1,
                    capacity_per_year: None,
                    open_intervals: vec![],
                },
                DirectedRoute {
                    logical_id: LogicalId(4),
                    from: SettlementId(3),
                    to: SettlementId(2),
                    cost: 1,
                    capacity_per_year: None,
                    open_intervals: vec![],
                },
            ],
            ..TerritoryConfig::default()
        };
        let paths = shortest_paths_from(&state, &config, Year(0), SettlementId(1));
        assert_eq!(paths[&SettlementId(2)].cost, 2);
        assert_eq!(
            paths[&SettlementId(2)].settlements,
            vec![SettlementId(1), SettlementId(3), SettlementId(2)]
        );
        assert!(
            !shortest_paths_from(&state, &config, Year(0), SettlementId(2))
                .contains_key(&SettlementId(1))
        );
    }
}
