//! Versioned settlement, route, residence, and migration configuration.

use crate::{LogicalId, Person, PersonId, SettlementId, ValidationIssue, Year, YearInterval};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfessionId(pub String);

impl From<&str> for ProfessionId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfessionDefinition {
    pub id: ProfessionId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectedRoute {
    pub logical_id: LogicalId,
    pub from: SettlementId,
    pub to: SettlementId,
    /// Non-negative route cost in the same integer units as settlement opportunity scores.
    pub cost: u32,
    /// Maximum number of residents that can traverse the route in one year; None is unlimited.
    pub capacity_per_year: Option<u32>,
    /// Empty means the route is always open. Otherwise any interval opens it.
    #[serde(default)]
    pub open_intervals: Vec<YearInterval>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationUnit {
    /// Move one resident. A resident in a larger household is detached and leaves the rest behind.
    #[default]
    Person,
    /// Move all living members of each eligible household together.
    Household,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationPolicy {
    pub enabled: bool,
    pub probability_ppm: u32,
    pub unit: MigrationUnit,
    pub minimum_age: u16,
    pub maximum_age: Option<u16>,
    /// Maximum destinations considered for each origin; bounded independently of population.
    pub candidate_limit: u16,
    /// Maximum moving units (persons or households) accepted during the annual batch.
    pub max_moving_units_per_year: u32,
    /// Optional hard cutoff for the shortest directed path cost.
    pub maximum_travel_cost: Option<u32>,
    /// Minimum `destination opportunity - origin opportunity - route cost` to propose a move.
    pub minimum_net_opportunity_gain: i64,
}

impl Default for MigrationPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            probability_ppm: 0,
            unit: MigrationUnit::Person,
            minimum_age: 18,
            maximum_age: None,
            candidate_limit: 8,
            max_moving_units_per_year: 0,
            maximum_travel_cost: None,
            minimum_net_opportunity_gain: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForcedDisplacementCommand {
    pub id: LogicalId,
    /// Selection of living residents; ruleset policy expands a selected household when configured.
    pub person_ids: BTreeSet<PersonId>,
    pub destination: SettlementId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledDisplacement {
    pub year: Year,
    pub command: ForcedDisplacementCommand,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerritoryConfig {
    /// Change this when route topology or availability is revised.
    pub route_revision: u64,
    pub routes: Vec<DirectedRoute>,
    /// Missing entries are unlimited; zero-capacity settlements are valid and remain in the world.
    #[serde(default)]
    pub settlement_capacities: BTreeMap<SettlementId, u32>,
    /// Fictional integer opportunity scores. Routes and migration policy define their meaning.
    #[serde(default)]
    pub opportunity_scores: BTreeMap<SettlementId, i64>,
    pub migration: MigrationPolicy,
    #[serde(default)]
    pub scheduled_displacements: Vec<ScheduledDisplacement>,
    #[serde(default)]
    pub professions: Vec<ProfessionDefinition>,
    /// A missing settlement entry offers no profession unless its configured set says otherwise.
    #[serde(default)]
    pub profession_options_by_settlement: BTreeMap<SettlementId, BTreeSet<ProfessionId>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResidenceInterval {
    pub settlement_id: SettlementId,
    pub validity: YearInterval,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfessionInterval {
    pub profession_id: ProfessionId,
    pub validity: YearInterval,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovementReason {
    Voluntary,
    ForcedDisplacement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfessionChangeReason {
    DestinationDoesNotOfferProfession,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementYearBalance {
    pub settlement_id: SettlementId,
    pub before: u64,
    pub births: u64,
    pub deaths: u64,
    pub incoming: u64,
    pub outgoing: u64,
    pub after: u64,
}

pub fn validate_territory_config(
    config: &TerritoryConfig,
    start_year: Year,
    settlements: &BTreeSet<SettlementId>,
    people: &BTreeMap<PersonId, &Person>,
    _household_members: &BTreeMap<crate::HouseholdId, &BTreeSet<PersonId>>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut initial_counts: BTreeMap<SettlementId, u64> = BTreeMap::new();
    for person in people
        .values()
        .filter(|person| person.alive_at_close(start_year))
    {
        *initial_counts.entry(person.settlement_id).or_default() += 1;
    }
    let mut route_ids = BTreeSet::new();
    for (index, route) in config.routes.iter().enumerate() {
        let path = format!("territory.routes[{index}]");
        if !route_ids.insert(route.logical_id) {
            issues.push(ValidationIssue::new(
                format!("{path}.logical_id"),
                "duplicate_logical_id",
                "route logical IDs must be unique",
            ));
        }
        if route.from == route.to {
            issues.push(ValidationIssue::new(
                &path,
                "self_route",
                "a directed route must connect different settlements",
            ));
        }
        for (field, settlement_id) in [("from", route.from), ("to", route.to)] {
            if !settlements.contains(&settlement_id) {
                issues.push(ValidationIssue::new(
                    format!("{path}.{field}"),
                    "unknown_reference",
                    "route endpoint does not exist",
                ));
            }
        }
        let mut intervals = route.open_intervals.clone();
        intervals.sort_by_key(|interval| interval.start);
        for (interval_index, interval) in intervals.iter().enumerate() {
            if !interval.is_valid() {
                issues.push(ValidationIssue::new(
                    format!("{path}.open_intervals[{interval_index}]"),
                    "invalid_interval",
                    "route interval end cannot precede its start",
                ));
            }
            if interval_index > 0 {
                let previous = intervals[interval_index - 1];
                if previous
                    .end_exclusive
                    .is_none_or(|end| end > interval.start)
                {
                    issues.push(ValidationIssue::new(
                        format!("{path}.open_intervals[{interval_index}]"),
                        "overlapping_intervals",
                        "route open intervals must not overlap",
                    ));
                }
            }
        }
    }

    for settlement_id in config.settlement_capacities.keys() {
        if !settlements.contains(settlement_id) {
            issues.push(ValidationIssue::new(
                format!("territory.settlement_capacities[{}]", settlement_id.0),
                "unknown_reference",
                "capacity references an unknown settlement",
            ));
        }
    }
    for (settlement_id, capacity) in &config.settlement_capacities {
        if initial_counts
            .get(settlement_id)
            .copied()
            .unwrap_or_default()
            > u64::from(*capacity)
        {
            issues.push(ValidationIssue::new(
                format!("territory.settlement_capacities[{}]", settlement_id.0),
                "initial_population_over_capacity",
                "initial living population exceeds settlement capacity",
            ));
        }
    }
    for settlement_id in config.opportunity_scores.keys() {
        if !settlements.contains(settlement_id) {
            issues.push(ValidationIssue::new(
                format!("territory.opportunity_scores[{}]", settlement_id.0),
                "unknown_reference",
                "opportunity score references an unknown settlement",
            ));
        }
    }

    let policy = &config.migration;
    if policy.probability_ppm > 1_000_000 {
        issues.push(ValidationIssue::new(
            "territory.migration.probability_ppm",
            "probability_out_of_range",
            "probability must be between 0 and 1,000,000 ppm",
        ));
    }
    if policy.enabled && (policy.candidate_limit == 0 || policy.candidate_limit > 1024) {
        issues.push(ValidationIssue::new(
            "territory.migration.candidate_limit",
            "out_of_range",
            "candidate limit must be between 1 and 1024",
        ));
    }
    if policy.enabled && policy.max_moving_units_per_year == 0 {
        issues.push(ValidationIssue::new(
            "territory.migration.max_moving_units_per_year",
            "out_of_range",
            "enabled migration needs a positive annual unit limit",
        ));
    }
    if policy
        .maximum_age
        .is_some_and(|maximum| maximum < policy.minimum_age)
    {
        issues.push(ValidationIssue::new(
            "territory.migration.maximum_age",
            "invalid_age_range",
            "maximum migration age must not precede minimum age",
        ));
    }

    let mut profession_ids = BTreeSet::new();
    for (index, profession) in config.professions.iter().enumerate() {
        if profession.id.0.trim().is_empty() || profession.name.trim().is_empty() {
            issues.push(ValidationIssue::new(
                format!("territory.professions[{index}]"),
                "empty_profession",
                "profession ID and name must not be empty",
            ));
        }
        if !profession_ids.insert(profession.id.clone()) {
            issues.push(ValidationIssue::new(
                format!("territory.professions[{index}].id"),
                "duplicate_id",
                "profession IDs must be unique",
            ));
        }
    }
    for (settlement_id, options) in &config.profession_options_by_settlement {
        if !settlements.contains(settlement_id) {
            issues.push(ValidationIssue::new(
                format!(
                    "territory.profession_options_by_settlement[{}]",
                    settlement_id.0
                ),
                "unknown_reference",
                "profession options reference an unknown settlement",
            ));
        }
        for profession_id in options {
            if !profession_ids.contains(profession_id) {
                issues.push(ValidationIssue::new(
                    format!(
                        "territory.profession_options_by_settlement[{}]",
                        settlement_id.0
                    ),
                    "unknown_profession",
                    "settlement references an unknown profession",
                ));
            }
        }
    }
    for (person_id, person) in people {
        if let Some(profession) = &person.profession {
            let path = format!("initial_people[{}].profession", person_id.0);
            if !profession_ids.contains(profession) {
                issues.push(ValidationIssue::new(
                    path,
                    "unknown_profession",
                    "initial profession must reference the configured profession catalog",
                ));
            } else if !config
                .profession_options_by_settlement
                .get(&person.settlement_id)
                .is_some_and(|options| options.contains(profession))
            {
                issues.push(ValidationIssue::new(
                    path,
                    "profession_unavailable",
                    "initial profession must be offered in the person's settlement",
                ));
            }
        }
    }

    let mut command_ids = BTreeSet::new();
    let mut scheduled_person_years = BTreeSet::new();
    let mut scheduled_household_years = BTreeSet::new();
    for (index, scheduled) in config.scheduled_displacements.iter().enumerate() {
        let path = format!("territory.scheduled_displacements[{index}]");
        if scheduled.year < start_year {
            issues.push(ValidationIssue::new(
                format!("{path}.year"),
                "before_start",
                "scheduled displacement cannot precede simulation start",
            ));
        }
        if !command_ids.insert((scheduled.year, scheduled.command.id)) {
            issues.push(ValidationIssue::new(
                format!("{path}.command.id"),
                "duplicate_command_id",
                "command IDs must be unique within a year",
            ));
        }
        if scheduled.command.person_ids.is_empty() {
            issues.push(ValidationIssue::new(
                format!("{path}.command.person_ids"),
                "empty_selection",
                "displacement must select at least one person",
            ));
        }
        if !settlements.contains(&scheduled.command.destination) {
            issues.push(ValidationIssue::new(
                format!("{path}.command.destination"),
                "unknown_reference",
                "displacement destination does not exist",
            ));
        }
        let selected = &scheduled.command.person_ids;
        let selected_households: BTreeSet<_> = selected
            .iter()
            .filter_map(|person_id| people.get(person_id).and_then(|person| person.household_id))
            .collect();
        if policy.unit == MigrationUnit::Household {
            for household_id in selected_households {
                if !scheduled_household_years.insert((scheduled.year, household_id)) {
                    issues.push(ValidationIssue::new(
                        format!("{path}.command.person_ids"),
                        "household_in_multiple_commands",
                        "a household cannot be split across forced commands in the same year",
                    ));
                }
            }
        }
        for person_id in selected {
            match people.get(person_id) {
                None => issues.push(ValidationIssue::new(
                    format!("{path}.command.person_ids"),
                    "unknown_reference",
                    "displacement person does not exist in the initial population",
                )),
                Some(person) => {
                    if !person.alive_at_start(scheduled.year) {
                        issues.push(ValidationIssue::new(
                            format!("{path}.command.person_ids"),
                            "person_not_alive",
                            "displacement person must be alive at the scheduled year start",
                        ));
                    }
                    if person.settlement_id == scheduled.command.destination {
                        issues.push(ValidationIssue::new(
                            format!("{path}.command.destination"),
                            "already_resident",
                            "displacement destination must differ from the person's origin",
                        ));
                    }
                    if !scheduled_person_years.insert((scheduled.year, *person_id)) {
                        issues.push(ValidationIssue::new(
                            format!("{path}.command.person_ids"),
                            "duplicate_displacement",
                            "a person cannot be displaced twice in the same year",
                        ));
                    }
                }
            }
        }
    }
    issues
}
