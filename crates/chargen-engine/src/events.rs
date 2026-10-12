//! Deterministic application of the scenario's scheduled, non-autonomous shocks.

use crate::{
    apply_death, derive_logical_id, emit_event, keyed_random_u64, random_below, AnnualIndexes,
    DecisionDomain, EngineError,
};
use chargen_domain::{
    ActiveEventEffect, DeathCause, EconomyYearModifiers, EntityRef, EventEffect, EventMove,
    EventPopulationFilter, EventRealization, EventSelection, EventWarning, ExactOverflowPolicy,
    ForcedDisplacementCommand, HistoricalEvent, HistoricalEventKind, PersonId, ScheduledEvent,
    State, ValidatedScenario, Year,
};
use std::cmp::Reverse;
use std::collections::BTreeSet;

use crate::migration::{self, MigrationUsage, RouteCache};

/// Remove restrictions whose exclusive end is this year's A0 boundary.
pub(crate) fn expire_event_effects(state: &mut State, year: Year) {
    state
        .event_effects
        .retain(|effect| effect.end_exclusive.is_none_or(|end| year < end));
}

/// Snapshot active event restrictions for A4. A selected worker is removed only
/// from production; group membership and every personal trait remain unchanged.
pub(crate) fn production_modifiers(state: &State, year: Year) -> EconomyYearModifiers {
    let mut modifiers = EconomyYearModifiers::default();
    for active in &state.event_effects {
        if active.active_at(year) && matches!(&active.effect, EventEffect::WorkIncapacity { .. }) {
            modifiers
                .incapacitated_person_ids
                .extend(active.selected_person_ids.iter().copied());
        }
    }
    for group in &state.economy.groups {
        let capacity = state
            .event_effects
            .iter()
            .filter(|active| {
                active.active_at(year)
                    && matches!(&active.effect, EventEffect::ProductionLoss { .. })
                    && active
                        .settlement_ids
                        .contains(&group.group_id.settlement_id)
                    && active.profession_ids.as_ref().is_none_or(|professions| {
                        group
                            .group_id
                            .profession_id
                            .as_ref()
                            .is_some_and(|profession| professions.contains(profession))
                    })
            })
            .fold(1_000_000_i128, |capacity, active| {
                let EventEffect::ProductionLoss { loss_ppm } = &active.effect else {
                    return capacity;
                };
                (capacity * i128::from(1_000_000_u32.saturating_sub(*loss_ppm))) / 1_000_000
            });
        if capacity < 1_000_000 {
            modifiers
                .production_capacity_ppm_by_group
                .insert(group.group_id.clone(), capacity.clamp(0, 1_000_000) as u32);
        }
    }
    modifiers
}

/// Apply each event that starts this year in stable priority / ID / effect order.
/// The call operates on the pending annual state, so any error discards the whole year.
pub(crate) fn apply_scheduled_events(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    history: &mut Vec<HistoricalEvent>,
    movement_usage: &mut MigrationUsage,
    route_cache: &mut RouteCache,
) -> Result<(), EngineError> {
    if state.rules_version != chargen_domain::Scenario::EVENT_RULES_VERSION {
        return Ok(());
    }
    let mut agenda: Vec<_> = scenario
        .scenario()
        .scheduled_events
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|event| event.start_year == year)
        .cloned()
        .collect();
    agenda.sort_by_key(|event| (Reverse(event.priority), event.id));

    for event in agenda {
        emit_event(
            state,
            history,
            year,
            event.id,
            vec![EntityRef::World(state.world_id)],
            HistoricalEventKind::ScheduledEventOccurred {
                event_id: event.id,
                name: event.name.clone(),
                priority: event.priority,
            },
        );
        for (effect_index, effect) in event.effects.iter().enumerate() {
            let effect_index = u32::try_from(effect_index).map_err(|_| {
                EngineError::ScheduledEvent("event has too many effects to index".into())
            })?;
            let realization = realize_effect(
                scenario,
                state,
                indexes,
                year,
                &event,
                effect_index,
                effect,
                history,
                movement_usage,
                route_cache,
            )?;
            let mut affected = realization.affected_entities();
            affected.push(EntityRef::World(state.world_id));
            affected.sort();
            affected.dedup();
            emit_event(
                state,
                history,
                year,
                event.id,
                affected,
                HistoricalEventKind::EventEffectRealized {
                    event_id: event.id,
                    event_name: event.name.clone(),
                    effect_index,
                    effect: effect.clone(),
                    realization,
                },
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn realize_effect(
    scenario: &ValidatedScenario,
    state: &mut State,
    indexes: &mut AnnualIndexes,
    year: Year,
    event: &ScheduledEvent,
    effect_index: u32,
    effect: &EventEffect,
    history: &mut Vec<HistoricalEvent>,
    movement_usage: &mut MigrationUsage,
    route_cache: &mut RouteCache,
) -> Result<EventRealization, EngineError> {
    let event_end = checked_event_end(event, year)?;
    match effect {
        EventEffect::Deaths { selection } => {
            let eligible = eligible_people(state, year, &event.population_filter);
            let (selected, requested_count, warning) = select_people(
                scenario,
                year,
                event,
                effect_index,
                selection,
                &eligible,
                state,
            )?;
            for person_id in &selected {
                apply_death(
                    scenario,
                    state,
                    indexes,
                    year,
                    *person_id,
                    DeathCause::ScheduledEvent {
                        event_id: event.id,
                        event_name: event.name.clone(),
                    },
                    history,
                )?;
            }
            Ok(EventRealization::People {
                realized_count: u32::try_from(selected.len()).map_err(|_| {
                    EngineError::ScheduledEvent("realized person count exceeds u32".into())
                })?,
                selected_person_ids: selected,
                requested_count,
                warning,
            })
        }
        EventEffect::WorkIncapacity { selection } => {
            let eligible = eligible_people(state, year, &event.population_filter);
            let (selected, requested_count, warning) = select_people(
                scenario,
                year,
                event,
                effect_index,
                selection,
                &eligible,
                state,
            )?;
            state.event_effects.push(ActiveEventEffect {
                event_id: event.id,
                effect_index,
                started_at: year,
                end_exclusive: event_end,
                effect: effect.clone(),
                selected_person_ids: selected.clone(),
                settlement_ids: BTreeSet::new(),
                route_ids: BTreeSet::new(),
                profession_ids: None,
            });
            Ok(EventRealization::People {
                realized_count: u32::try_from(selected.len()).map_err(|_| {
                    EngineError::ScheduledEvent("realized person count exceeds u32".into())
                })?,
                selected_person_ids: selected,
                requested_count,
                warning,
            })
        }
        EventEffect::ProductionLoss { loss_ppm } => {
            let settlement_ids = resolved_settlements(state, &event.population_filter);
            let profession_ids = event.population_filter.profession_ids.clone();
            state.event_effects.push(ActiveEventEffect {
                event_id: event.id,
                effect_index,
                started_at: year,
                end_exclusive: event_end,
                effect: effect.clone(),
                selected_person_ids: Vec::new(),
                settlement_ids: settlement_ids.clone(),
                route_ids: BTreeSet::new(),
                profession_ids: profession_ids.clone(),
            });
            Ok(EventRealization::ProductionLoss {
                settlement_ids,
                profession_ids,
                loss_ppm: *loss_ppm,
            })
        }
        EventEffect::StockDestruction { loss_ppm } => {
            let config = scenario.scenario().economy.as_ref().ok_or_else(|| {
                EngineError::Economy("event requires economic configuration".into())
            })?;
            let filter = &event.population_filter;
            let profession_scope = filter.profession_ids.as_ref();
            let mut losses = Vec::new();
            if let Some(settlements) = &filter.settlement_ids {
                for settlement_id in settlements {
                    let report = crate::economy::apply_stock_destruction(
                        config,
                        state,
                        year,
                        event.id,
                        Some(*settlement_id),
                        profession_scope,
                        *loss_ppm,
                    )
                    .map_err(|error| EngineError::Economy(error.to_string()))?;
                    losses.extend(report.losses);
                }
            } else {
                let report = crate::economy::apply_stock_destruction(
                    config,
                    state,
                    year,
                    event.id,
                    None,
                    profession_scope,
                    *loss_ppm,
                )
                .map_err(|error| EngineError::Economy(error.to_string()))?;
                losses.extend(report.losses);
            }
            losses.sort_by(|left, right| {
                (&left.group_id, &left.good_id).cmp(&(&right.group_id, &right.good_id))
            });
            Ok(EventRealization::StockDestruction { losses })
        }
        EventEffect::RouteClosure { route_ids } => {
            state.event_effects.push(ActiveEventEffect {
                event_id: event.id,
                effect_index,
                started_at: year,
                end_exclusive: event_end,
                effect: effect.clone(),
                selected_person_ids: Vec::new(),
                settlement_ids: BTreeSet::new(),
                route_ids: route_ids.clone(),
                profession_ids: None,
            });
            Ok(EventRealization::RouteClosure {
                route_ids: route_ids.clone(),
            })
        }
        EventEffect::Displacement {
            selection,
            destination,
        } => {
            let Some(territory) = scenario.scenario().territory.as_ref() else {
                return Err(EngineError::ScheduledEvent(
                    "displacement requires territory configuration".into(),
                ));
            };
            let mut eligible = eligible_people(state, year, &event.population_filter);
            eligible.retain(|person_id| {
                if state.persons.get(person_id).is_some_and(|person| {
                    person.settlement_id == *destination
                        || movement_usage.has_moved_person(*person_id)
                }) {
                    return false;
                }
                if territory.migration.unit == chargen_domain::MigrationUnit::Household {
                    let household = state
                        .persons
                        .get(person_id)
                        .and_then(|person| person.household_id)
                        .and_then(|id| state.households.get(&id));
                    if household.is_some_and(|household| {
                        household
                            .members
                            .iter()
                            .any(|member| movement_usage.has_moved_person(*member))
                    }) {
                        return false;
                    }
                }
                true
            });
            let (requested_person_ids, requested_count, warning) = select_people(
                scenario,
                year,
                event,
                effect_index,
                selection,
                &eligible,
                state,
            )?;
            let (move_groups, next_usage) = if requested_person_ids.is_empty() {
                (Vec::new(), movement_usage.clone())
            } else {
                let command_id = derive_logical_id(
                    scenario.scenario().seed,
                    scenario.scenario().rules_version,
                    DecisionDomain::ForcedDisplacementPriority,
                    year,
                    event.id,
                    u64::from(effect_index).wrapping_add(0x4556_454e_5400),
                );
                let command = ForcedDisplacementCommand {
                    id: command_id,
                    person_ids: requested_person_ids.iter().copied().collect(),
                    destination: *destination,
                };
                migration::prepare_event_displacement(
                    scenario,
                    state,
                    territory,
                    year,
                    &command,
                    movement_usage,
                    route_cache,
                )?
            };
            migration::apply_forced_displacements(
                scenario,
                state,
                territory,
                year,
                &move_groups,
                history,
            )?;
            *movement_usage = next_usage;
            let mut moved_ids: BTreeSet<_> = BTreeSet::new();
            let mut moves = Vec::new();
            for group in move_groups {
                let mut person_ids = group.people;
                person_ids.sort_by_key(|person_id| {
                    state
                        .persons
                        .get(person_id)
                        .map(|person| (person.logical_id, *person_id))
                });
                moved_ids.extend(person_ids.iter().copied());
                moves.push(EventMove {
                    person_ids,
                    from: group.from,
                    to: group.to,
                    route_ids: group.route_ids,
                });
            }
            moves.sort_by_key(|movement| (movement.from, movement.to));
            Ok(EventRealization::Displacement {
                requested_count,
                realized_count: u32::try_from(moved_ids.len()).map_err(|_| {
                    EngineError::ScheduledEvent("moved person count exceeds u32".into())
                })?,
                requested_person_ids,
                moved_person_ids: sort_people_by_logical(state, moved_ids.into_iter().collect()),
                moves,
                warning,
            })
        }
    }
}

fn checked_event_end(event: &ScheduledEvent, year: Year) -> Result<Option<Year>, EngineError> {
    event
        .duration_years
        .map(|duration| {
            i32::try_from(duration)
                .ok()
                .and_then(|duration| year.checked_add(duration))
                .ok_or_else(|| {
                    EngineError::ScheduledEvent(format!(
                        "event {:?} duration overflows the year range",
                        event.id
                    ))
                })
        })
        .transpose()
}

fn resolved_settlements(
    state: &State,
    filter: &EventPopulationFilter,
) -> BTreeSet<chargen_domain::SettlementId> {
    filter
        .settlement_ids
        .clone()
        .unwrap_or_else(|| state.settlements.keys().copied().collect::<BTreeSet<_>>())
}

fn eligible_people(state: &State, year: Year, filter: &EventPopulationFilter) -> Vec<PersonId> {
    let mut eligible: Vec<_> = state
        .persons
        .values()
        .filter(|person| person.alive_at_start(year) && person.death_year.is_none())
        .filter(|person| {
            filter
                .settlement_ids
                .as_ref()
                .is_none_or(|ids| ids.contains(&person.settlement_id))
        })
        .filter(|person| {
            filter.profession_ids.as_ref().is_none_or(|ids| {
                person
                    .profession
                    .as_ref()
                    .is_some_and(|profession| ids.contains(profession))
            })
        })
        .filter(|person| {
            let age = person.age_at_close(year).unwrap_or_default();
            filter
                .minimum_age
                .is_none_or(|minimum| age >= u32::from(minimum))
                && filter
                    .maximum_age
                    .is_none_or(|maximum| age <= u32::from(maximum))
        })
        .map(|person| person.id)
        .collect();
    eligible.sort_by_key(|person_id| {
        state
            .persons
            .get(person_id)
            .map(|person| (person.logical_id, *person_id))
    });
    eligible
}

fn select_people(
    scenario: &ValidatedScenario,
    year: Year,
    event: &ScheduledEvent,
    effect_index: u32,
    selection: &EventSelection,
    eligible: &[PersonId],
    state: &State,
) -> Result<(Vec<PersonId>, u32, Option<EventWarning>), EngineError> {
    let seed = scenario.scenario().seed;
    let rules = scenario.scenario().rules_version;
    let event_key = event.id.0 ^ u64::from(effect_index).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let eligible_count = u32::try_from(eligible.len())
        .map_err(|_| EngineError::ScheduledEvent("eligible population exceeds u32".into()))?;
    match *selection {
        EventSelection::Exact { count, overflow } => {
            if count > eligible_count && overflow == ExactOverflowPolicy::Error {
                return Err(EngineError::ScheduledEvent(format!(
                    "event {:?} effect {effect_index} requested {count} people but only {eligible_count} qualify",
                    event.id
                )));
            }
            let realized = count.min(eligible_count);
            let mut ranked: Vec<_> = eligible
                .iter()
                .filter_map(|person_id| {
                    let person = state.persons.get(person_id)?.logical_id;
                    Some((
                        keyed_random_u64(
                            seed,
                            rules,
                            DecisionDomain::EventSelection,
                            year,
                            person,
                            event_key,
                        ),
                        person,
                        *person_id,
                    ))
                })
                .collect();
            ranked.sort_unstable();
            let mut selected: Vec<_> = ranked
                .into_iter()
                .take(realized as usize)
                .map(|(_, _, person_id)| person_id)
                .collect();
            selected.sort_by_key(|person_id| {
                state.persons.get(person_id).map(|person| person.logical_id)
            });
            let warning = (count > eligible_count).then_some(EventWarning::ExactCountClamped {
                requested_count: count,
                eligible_count,
                realized_count: realized,
            });
            Ok((selected, count, warning))
        }
        EventSelection::Probabilistic { probability_ppm } => {
            let mut selected: Vec<_> = eligible
                .iter()
                .filter_map(|person_id| {
                    let person = state.persons.get(person_id)?.logical_id;
                    (probability_ppm >= 1_000_000
                        || (probability_ppm > 0
                            && random_below(
                                seed,
                                rules,
                                DecisionDomain::EventProbability,
                                year,
                                person,
                                event_key,
                                1_000_000,
                            ) < u64::from(probability_ppm)))
                    .then_some(*person_id)
                })
                .collect();
            selected.sort_by_key(|person_id| {
                state.persons.get(person_id).map(|person| person.logical_id)
            });
            let realized = u32::try_from(selected.len()).map_err(|_| {
                EngineError::ScheduledEvent("realized person count exceeds u32".into())
            })?;
            Ok((selected, realized, None))
        }
    }
}

fn sort_people_by_logical(state: &State, mut people: Vec<PersonId>) -> Vec<PersonId> {
    people.sort_by_key(|person_id| {
        state
            .persons
            .get(person_id)
            .map(|person| (person.logical_id, *person_id))
    });
    people
}
