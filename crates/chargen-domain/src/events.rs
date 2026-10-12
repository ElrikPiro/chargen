//! Declarative, branch-versioned events and their durable realized effects.

use crate::{
    EconomyStockLossRecord, EntityRef, LogicalId, PersonId, ProfessionId, SettlementId,
    ValidationIssue, Year,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledEvent {
    pub id: LogicalId,
    pub name: String,
    pub start_year: Year,
    /// Number of annual closes for which non-immediate effects remain active. `None` is permanent.
    pub duration_years: Option<u32>,
    /// Higher priorities are applied first; logical event ID breaks ties.
    pub priority: i32,
    #[serde(default)]
    pub population_filter: EventPopulationFilter,
    pub effects: Vec<EventEffect>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EventPopulationFilter {
    /// `None` means every settlement in the world.
    pub settlement_ids: Option<BTreeSet<SettlementId>>,
    pub minimum_age: Option<u16>,
    pub maximum_age: Option<u16>,
    /// Also limits production and stock effects to groups with one of these professions.
    pub profession_ids: Option<BTreeSet<ProfessionId>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum EventEffect {
    Deaths {
        selection: EventSelection,
    },
    WorkIncapacity {
        selection: EventSelection,
    },
    ProductionLoss {
        loss_ppm: u32,
    },
    StockDestruction {
        loss_ppm: u32,
    },
    RouteClosure {
        route_ids: BTreeSet<LogicalId>,
    },
    Displacement {
        selection: EventSelection,
        destination: SettlementId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "mode",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum EventSelection {
    Exact {
        count: u32,
        overflow: ExactOverflowPolicy,
    },
    Probabilistic {
        probability_ppm: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactOverflowPolicy {
    Error,
    ClampWithWarning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum EventWarning {
    ExactCountClamped {
        requested_count: u32,
        eligible_count: u32,
        realized_count: u32,
    },
    DisplacementSkippedAlreadyMoved {
        skipped_count: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "data",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum EventRealization {
    People {
        selected_person_ids: Vec<PersonId>,
        requested_count: u32,
        realized_count: u32,
        warning: Option<EventWarning>,
    },
    ProductionLoss {
        settlement_ids: BTreeSet<SettlementId>,
        profession_ids: Option<BTreeSet<ProfessionId>>,
        loss_ppm: u32,
    },
    StockDestruction {
        losses: Vec<EconomyStockLossRecord>,
    },
    RouteClosure {
        route_ids: BTreeSet<LogicalId>,
    },
    Displacement {
        requested_count: u32,
        /// People actually moved, including household expansion when configured.
        realized_count: u32,
        requested_person_ids: Vec<PersonId>,
        moved_person_ids: Vec<PersonId>,
        moves: Vec<EventMove>,
        warning: Option<EventWarning>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventMove {
    pub person_ids: Vec<PersonId>,
    pub from: SettlementId,
    pub to: SettlementId,
    pub route_ids: Vec<LogicalId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveEventEffect {
    pub event_id: LogicalId,
    pub effect_index: u32,
    pub started_at: Year,
    /// Exclusive end; `None` denotes a permanent restriction.
    pub end_exclusive: Option<Year>,
    pub effect: EventEffect,
    /// Resolved once when the event starts. Later years never reroll these people.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_person_ids: Vec<PersonId>,
    /// Resolved scope when the event starts; later demographic changes do not retarget it.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub settlement_ids: BTreeSet<SettlementId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub route_ids: BTreeSet<LogicalId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profession_ids: Option<BTreeSet<ProfessionId>>,
}

impl ActiveEventEffect {
    pub fn active_at(&self, year: Year) -> bool {
        self.started_at <= year && self.end_exclusive.is_none_or(|end| year < end)
    }

    pub fn affects_person(&self, person_id: PersonId, year: Year) -> bool {
        self.active_at(year) && self.selected_person_ids.contains(&person_id)
    }

    pub fn closes_route(&self, route_id: LogicalId, year: Year) -> bool {
        self.active_at(year)
            && matches!(&self.effect, EventEffect::RouteClosure { .. })
            && self.route_ids.contains(&route_id)
    }
}

impl EventRealization {
    pub fn affected_entities(&self) -> Vec<EntityRef> {
        let mut entities = BTreeSet::new();
        match self {
            Self::People {
                selected_person_ids,
                ..
            } => entities.extend(selected_person_ids.iter().copied().map(EntityRef::Person)),
            Self::ProductionLoss { settlement_ids, .. } => {
                entities.extend(settlement_ids.iter().copied().map(EntityRef::Settlement))
            }
            Self::StockDestruction { losses } => entities.extend(losses.iter().flat_map(|loss| {
                [
                    EntityRef::EconomicGroup(loss.group_id.clone()),
                    EntityRef::Settlement(loss.group_id.settlement_id),
                ]
            })),
            Self::RouteClosure { route_ids } => {
                entities.extend(route_ids.iter().copied().map(EntityRef::Route));
            }
            Self::Displacement {
                moved_person_ids,
                moves,
                ..
            } => {
                entities.extend(moved_person_ids.iter().copied().map(EntityRef::Person));
                for movement in moves {
                    entities.insert(EntityRef::Settlement(movement.from));
                    entities.insert(EntityRef::Settlement(movement.to));
                    entities.extend(movement.route_ids.iter().copied().map(EntityRef::Route));
                }
            }
        }
        entities.into_iter().collect()
    }
}

pub(crate) fn validate_event_agenda(
    events: &[ScheduledEvent],
    start_year: Year,
    settlements: &BTreeSet<SettlementId>,
    professions: &BTreeSet<ProfessionId>,
    route_ids: &BTreeSet<LogicalId>,
    has_territory: bool,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut event_ids = BTreeSet::new();
    for (event_index, event) in events.iter().enumerate() {
        let path = format!("scheduled_events[{event_index}]");
        if !event_ids.insert(event.id) {
            issues.push(ValidationIssue::new(
                format!("{path}.id"),
                "duplicate_event_id",
                "scheduled event IDs must be unique",
            ));
        }
        if event.name.trim().is_empty() {
            issues.push(ValidationIssue::new(
                format!("{path}.name"),
                "empty_name",
                "scheduled event name must not be empty",
            ));
        }
        if event.start_year < start_year {
            issues.push(ValidationIssue::new(
                format!("{path}.start_year"),
                "event_before_start",
                "scheduled events cannot begin before the scenario start year",
            ));
        }
        if let Some(duration) = event.duration_years {
            if duration == 0 {
                issues.push(ValidationIssue::new(
                    format!("{path}.duration_years"),
                    "invalid_duration",
                    "event duration must be at least one annual close",
                ));
            } else if i32::try_from(duration)
                .ok()
                .and_then(|duration| event.start_year.checked_add(duration))
                .is_none()
            {
                issues.push(ValidationIssue::new(
                    format!("{path}.duration_years"),
                    "year_overflow",
                    "event end year exceeds the supported year range",
                ));
            }
        }
        if event.effects.is_empty() {
            issues.push(ValidationIssue::new(
                format!("{path}.effects"),
                "empty_event_effects",
                "a scheduled event must have at least one effect",
            ));
        }
        if let Some(scope) = &event.population_filter.settlement_ids {
            for settlement_id in scope {
                if !settlements.contains(settlement_id) {
                    issues.push(ValidationIssue::new(
                        format!("{path}.population_filter.settlement_ids"),
                        "unknown_settlement",
                        "event scope references an unknown settlement",
                    ));
                }
            }
        }
        if event
            .population_filter
            .minimum_age
            .zip(event.population_filter.maximum_age)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            issues.push(ValidationIssue::new(
                format!("{path}.population_filter"),
                "invalid_age_range",
                "minimum event age must not exceed maximum age",
            ));
        }
        if let Some(scope) = &event.population_filter.profession_ids {
            for profession_id in scope {
                if !professions.contains(profession_id) {
                    issues.push(ValidationIssue::new(
                        format!("{path}.population_filter.profession_ids"),
                        "unknown_profession",
                        "event filter references an unknown profession",
                    ));
                }
            }
        }
        for (effect_index, effect) in event.effects.iter().enumerate() {
            let effect_path = format!("{path}.effects[{effect_index}]");
            match effect {
                EventEffect::Deaths { selection }
                | EventEffect::WorkIncapacity { selection }
                | EventEffect::Displacement { selection, .. } => {
                    validate_selection(selection, &effect_path, &mut issues);
                    if matches!(effect, EventEffect::Displacement { .. }) && !has_territory {
                        issues.push(ValidationIssue::new(
                            &effect_path,
                            "missing_territory",
                            "displacement effects require territory and routes",
                        ));
                    }
                }
                EventEffect::ProductionLoss { loss_ppm }
                | EventEffect::StockDestruction { loss_ppm } => {
                    validate_ppm(*loss_ppm, &effect_path, &mut issues);
                    if event.population_filter.minimum_age.is_some()
                        || event.population_filter.maximum_age.is_some()
                    {
                        issues.push(ValidationIssue::new(
                            format!("{path}.population_filter"),
                            "age_filter_not_applicable",
                            "economic effects cannot use age filters",
                        ));
                    }
                }
                EventEffect::RouteClosure {
                    route_ids: selected,
                } => {
                    if !has_territory {
                        issues.push(ValidationIssue::new(
                            &effect_path,
                            "missing_territory",
                            "route closures require territory and routes",
                        ));
                    }
                    if event.population_filter != EventPopulationFilter::default() {
                        issues.push(ValidationIssue::new(
                            format!("{path}.population_filter"),
                            "filter_not_applicable",
                            "route closures target explicit route IDs and cannot use population filters",
                        ));
                    }
                    for route_id in selected {
                        if !route_ids.contains(route_id) {
                            issues.push(ValidationIssue::new(
                                format!("{effect_path}.route_ids"),
                                "unknown_route",
                                "route closure references an unknown route",
                            ));
                        }
                    }
                }
            }
            if let EventEffect::Displacement { destination, .. } = effect {
                if !settlements.contains(destination) {
                    issues.push(ValidationIssue::new(
                        format!("{effect_path}.destination"),
                        "unknown_settlement",
                        "displacement destination does not exist",
                    ));
                }
            }
        }
    }
    issues
}

pub(crate) fn validate_active_event_effects(
    effects: &[ActiveEventEffect],
    current_year: Option<Year>,
    person_ids: &BTreeSet<PersonId>,
    settlement_ids: &BTreeSet<SettlementId>,
    profession_ids: &BTreeSet<ProfessionId>,
    route_ids: &BTreeSet<LogicalId>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut identities = BTreeSet::new();
    let at_year = current_year;
    for (index, active) in effects.iter().enumerate() {
        let path = format!("event_effects[{index}]");
        if !identities.insert((active.event_id, active.effect_index)) {
            issues.push(ValidationIssue::new(
                &path,
                "duplicate_active_effect",
                "an event effect can have only one active record",
            ));
        }
        if active
            .end_exclusive
            .is_some_and(|end| end <= active.started_at)
        {
            issues.push(ValidationIssue::new(
                format!("{path}.end_exclusive"),
                "invalid_interval",
                "active effect end must be later than its start",
            ));
        }
        if let Some(at_year) = at_year {
            if !active.active_at(at_year) {
                issues.push(ValidationIssue::new(
                    &path,
                    "inactive_effect_retained",
                    "only event effects active at the last closed year may remain in state",
                ));
            }
        } else {
            issues.push(ValidationIssue::new(
                &path,
                "effect_before_first_close",
                "an unadvanced state cannot contain active effects",
            ));
        }
        let mut unique_people = BTreeSet::new();
        for person_id in &active.selected_person_ids {
            if !unique_people.insert(*person_id) {
                issues.push(ValidationIssue::new(
                    format!("{path}.selected_person_ids"),
                    "duplicate_person",
                    "an active effect cannot target one person twice",
                ));
            }
            if !person_ids.contains(person_id) {
                issues.push(ValidationIssue::new(
                    format!("{path}.selected_person_ids"),
                    "unknown_person",
                    "active effect references an unknown person",
                ));
            }
        }
        for settlement_id in &active.settlement_ids {
            if !settlement_ids.contains(settlement_id) {
                issues.push(ValidationIssue::new(
                    format!("{path}.settlement_ids"),
                    "unknown_settlement",
                    "active effect references an unknown settlement",
                ));
            }
        }
        if let Some(selected_professions) = &active.profession_ids {
            for profession_id in selected_professions {
                if !profession_ids.contains(profession_id) {
                    issues.push(ValidationIssue::new(
                        format!("{path}.profession_ids"),
                        "unknown_profession",
                        "active effect references an unknown profession",
                    ));
                }
            }
        }
        for route_id in &active.route_ids {
            if !route_ids.contains(route_id) {
                issues.push(ValidationIssue::new(
                    format!("{path}.route_ids"),
                    "unknown_route",
                    "active effect references an unknown route",
                ));
            }
        }
        match &active.effect {
            EventEffect::WorkIncapacity { selection } => {
                validate_selection(selection, &path, &mut issues);
                if !active.route_ids.is_empty() || !active.settlement_ids.is_empty() {
                    issues.push(ValidationIssue::new(
                        &path,
                        "unexpected_effect_scope",
                        "work incapacity targets selected people only",
                    ));
                }
                if active.profession_ids.is_some() {
                    issues.push(ValidationIssue::new(
                        format!("{path}.profession_ids"),
                        "unexpected_effect_scope",
                        "work incapacity stores its fixed person selection, not a profession filter",
                    ));
                }
                validate_resolved_selection(
                    selection,
                    active.selected_person_ids.len(),
                    &path,
                    &mut issues,
                );
            }
            EventEffect::ProductionLoss { loss_ppm } => {
                validate_ppm(*loss_ppm, &path, &mut issues);
                if !active.selected_person_ids.is_empty() || !active.route_ids.is_empty() {
                    issues.push(ValidationIssue::new(
                        &path,
                        "unexpected_effect_scope",
                        "economic effects target settlement and profession groups",
                    ));
                }
            }
            EventEffect::StockDestruction { .. }
            | EventEffect::Deaths { .. }
            | EventEffect::Displacement { .. } => {
                issues.push(ValidationIssue::new(
                    &path,
                    "instant_effect_persisted",
                    "instant event effects cannot remain active",
                ));
            }
            EventEffect::RouteClosure { .. } => {
                if !active.selected_person_ids.is_empty()
                    || !active.settlement_ids.is_empty()
                    || active.profession_ids.is_some()
                {
                    issues.push(ValidationIssue::new(
                        &path,
                        "invalid_route_effect_scope",
                        "route closure targets route IDs only",
                    ));
                }
            }
        }
        if let EventEffect::RouteClosure {
            route_ids: selected,
        } = &active.effect
        {
            if selected != &active.route_ids {
                issues.push(ValidationIssue::new(
                    format!("{path}.route_ids"),
                    "effect_scope_mismatch",
                    "active route IDs must match the event effect definition",
                ));
            }
        }
    }
    issues
}

pub(crate) fn validate_active_event_links(
    effects: &[ActiveEventEffect],
    agenda: &[ScheduledEvent],
) -> Vec<ValidationIssue> {
    let events: std::collections::BTreeMap<_, _> =
        agenda.iter().map(|event| (event.id, event)).collect();
    let mut issues = Vec::new();
    for (index, active) in effects.iter().enumerate() {
        let path = format!("event_effects[{index}]");
        let Some(event) = events.get(&active.event_id) else {
            issues.push(ValidationIssue::new(
                format!("{path}.event_id"),
                "unknown_event",
                "active effect must reference an event in the scenario agenda",
            ));
            continue;
        };
        if active.started_at != event.start_year {
            issues.push(ValidationIssue::new(
                format!("{path}.started_at"),
                "event_start_mismatch",
                "active effect starts in the event's configured start year",
            ));
        }
        let effect_index = usize::try_from(active.effect_index).ok();
        let Some(configured_effect) = effect_index.and_then(|index| event.effects.get(index))
        else {
            issues.push(ValidationIssue::new(
                format!("{path}.effect_index"),
                "unknown_event_effect",
                "active effect index must exist in the scheduled event",
            ));
            continue;
        };
        if configured_effect != &active.effect {
            issues.push(ValidationIssue::new(
                format!("{path}.effect"),
                "event_effect_mismatch",
                "active effect must match its scheduled event definition",
            ));
        }
        let expected_end = match event.duration_years {
            None => Some(None),
            Some(duration) => i32::try_from(duration)
                .ok()
                .and_then(|duration| event.start_year.checked_add(duration))
                .map(Some),
        };
        if expected_end.is_none() || active.end_exclusive != expected_end.flatten() {
            issues.push(ValidationIssue::new(
                format!("{path}.end_exclusive"),
                "event_duration_mismatch",
                "active effect interval must match the configured event duration",
            ));
        }
    }
    issues
}

fn validate_selection(selection: &EventSelection, path: &str, issues: &mut Vec<ValidationIssue>) {
    if let EventSelection::Probabilistic { probability_ppm } = selection {
        validate_ppm(*probability_ppm, path, issues);
    }
}

fn validate_resolved_selection(
    selection: &EventSelection,
    realized_count: usize,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    if let EventSelection::Exact { count, overflow } = selection {
        let valid = match overflow {
            ExactOverflowPolicy::Error => usize::try_from(*count).ok() == Some(realized_count),
            ExactOverflowPolicy::ClampWithWarning => realized_count <= *count as usize,
        };
        if !valid {
            issues.push(ValidationIssue::new(
                format!("{path}.selected_person_ids"),
                "selection_count_mismatch",
                "the fixed person selection is inconsistent with the event's exact-count policy",
            ));
        }
    }
}

fn validate_ppm(value: u32, path: &str, issues: &mut Vec<ValidationIssue>) {
    if value > 1_000_000 {
        issues.push(ValidationIssue::new(
            path,
            "probability_out_of_range",
            "probability or loss must be between 0 and 1,000,000 ppm",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(effect: EventEffect) -> ScheduledEvent {
        ScheduledEvent {
            id: LogicalId(10),
            name: "storm".into(),
            start_year: Year(1),
            duration_years: Some(1),
            priority: 0,
            population_filter: EventPopulationFilter::default(),
            effects: vec![effect],
        }
    }

    #[test]
    fn rejects_unknown_fields_in_nested_event_configuration() {
        let unknown_selection = serde_json::json!({
            "type": "deaths",
            "data": {
                "selection": {
                    "mode": "exact",
                    "data": { "count": 1, "overflow": "error", "future": true }
                }
            }
        });
        assert!(serde_json::from_value::<EventEffect>(unknown_selection).is_err());

        let unknown_filter = serde_json::json!({ "future": true });
        assert!(serde_json::from_value::<EventPopulationFilter>(unknown_filter).is_err());
    }

    #[test]
    fn validates_event_ids_durations_filters_routes_and_probability_bounds() {
        let mut overflow = event(EventEffect::Deaths {
            selection: EventSelection::Probabilistic {
                probability_ppm: 1_000_001,
            },
        });
        overflow.start_year = Year(i32::MAX);
        let mut route = event(EventEffect::RouteClosure {
            route_ids: [LogicalId(99)].into_iter().collect(),
        });
        route.population_filter.minimum_age = Some(10);
        let issues = validate_event_agenda(
            &[overflow, route],
            Year(0),
            &[SettlementId(1)].into_iter().collect(),
            &BTreeSet::new(),
            &[LogicalId(1)].into_iter().collect(),
            true,
        );
        let codes: BTreeSet<_> = issues.iter().map(|issue| issue.code.clone()).collect();
        assert!(codes.contains("year_overflow"));
        assert!(codes.contains("probability_out_of_range"));
        assert!(codes.contains("filter_not_applicable"));
        assert!(codes.contains("unknown_route"));
    }

    #[test]
    fn active_effect_must_match_its_scheduled_definition_and_interval() {
        let configured = event(EventEffect::RouteClosure {
            route_ids: [LogicalId(99)].into_iter().collect(),
        });
        let active = ActiveEventEffect {
            event_id: configured.id,
            effect_index: 0,
            started_at: Year(1),
            end_exclusive: Some(Year(3)),
            effect: EventEffect::RouteClosure {
                route_ids: [LogicalId(99)].into_iter().collect(),
            },
            selected_person_ids: Vec::new(),
            settlement_ids: BTreeSet::new(),
            route_ids: [LogicalId(99)].into_iter().collect(),
            profession_ids: None,
        };
        let issues = validate_active_event_links(&[active], &[configured]);
        assert!(issues
            .iter()
            .any(|issue| issue.code == "event_duration_mismatch"));
    }
}
