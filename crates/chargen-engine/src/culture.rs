//! Incremental culture, preconfigured affiliations, and bounded organizations.

use chargen_domain::{
    CultureAggregate, CultureConfig, CultureDimensionPolicy, CultureEligibility, CultureStateError,
    EntityRef, HistoricalEvent, HistoricalEventKind, InstitutionAffiliationEndReason,
    InstitutionId, InstitutionMembership, Organization, OrganizationDissolutionReason,
    OrganizationId, OrganizationMembership, OrganizationMembershipEndReason, Person, PersonId,
    SettlementId, State, TraitKey, Year, YearInterval,
};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CulturePopulationDelta {
    pub person_id: PersonId,
    pub from: Option<SettlementId>,
    pub to: Option<SettlementId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CultureBirthInput {
    pub person_id: PersonId,
    pub logical_id: chargen_domain::LogicalId,
    pub settlement_id: SettlementId,
    pub genetic_parents: Vec<PersonId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CultureBirthAssignment {
    pub person_id: PersonId,
    pub values: BTreeMap<TraitKey, i16>,
}

#[derive(Debug, Error)]
pub enum CultureError {
    #[error(transparent)]
    State(#[from] CultureStateError),
    #[error("cultural birth input contains duplicate person ID {0:?}")]
    DuplicateBirth(PersonId),
    #[error("cultural birth references a missing parent {0:?}")]
    MissingParent(PersonId),
    #[error("cultural operation exceeds the historical event order capacity")]
    EventOrderOverflow,
}

#[derive(Clone, Copy)]
struct CultureEventContext {
    year: Year,
    seed: u64,
    rules_version: u32,
}

/// Initialize configured cultural values for explicitly supplied founders and
/// establish the first baseline. Existing values from the scenario are kept.
pub fn initialize_culture(
    config: &CultureConfig,
    state: &mut State,
    year: Year,
) -> Result<(), CultureError> {
    for person in state.persons.values_mut() {
        for (key, policy) in &config.dimensions {
            let value = match person.traits.get(key).copied() {
                Some(value) => value,
                None => policy
                    .founder_value
                    .ok_or_else(|| CultureStateError::MissingTrait {
                        person_id: person.id,
                        key: key.clone(),
                    })?,
            };
            validate_cultural_value(policy, person.id, key, value)?;
            person.traits.insert(key.clone(), value);
        }
    }
    state.culture = Default::default();
    rebuild_culture_aggregates(config, state, year)
}

/// Rebuild all settlement sums from people, used for initialization and checks.
pub fn rebuild_culture_aggregates(
    config: &CultureConfig,
    state: &mut State,
    year: Year,
) -> Result<(), CultureError> {
    state.culture.settlements = calculate_settlement_aggregates(config, state, year)?;
    Ok(())
}

/// Reconcile a child branch's revised cultural policies with its inherited
/// state at the branch boundary. Existing personal values and temporal records
/// are retained; new dimensions receive only their explicit founder value.
/// Call this once when a branch first adopts a revised scenario.
pub fn reconcile_culture_config(
    config: &CultureConfig,
    state: &mut State,
    year: Year,
) -> Result<(), CultureError> {
    let institution_ids: BTreeSet<_> = config
        .institutions
        .iter()
        .map(|definition| definition.id.clone())
        .collect();
    if state
        .culture
        .institution_memberships
        .iter()
        .any(|membership| {
            membership.active_at_close(year)
                && !institution_ids.contains(&membership.institution_id)
        })
    {
        return Err(CultureStateError::InvalidConfiguration(
            "cannot retire an institution with active memberships at the revision boundary".into(),
        )
        .into());
    }

    for person in state.persons.values_mut() {
        for (key, policy) in &config.dimensions {
            let value = match person.traits.get(key).copied() {
                Some(value) => value,
                None => policy
                    .founder_value
                    .ok_or_else(|| CultureStateError::MissingTrait {
                        person_id: person.id,
                        key: key.clone(),
                    })?,
            };
            validate_cultural_value(policy, person.id, key, value)?;
            person.traits.entry(key.clone()).or_insert(value);
        }
    }

    rebuild_culture_aggregates(config, state, year)?;
    let organization_ids: Vec<_> = state.culture.organizations.keys().copied().collect();
    for organization_id in organization_ids {
        let members = state.culture.organizations[&organization_id]
            .current_members
            .clone();
        let values = aggregate_people(config, state, year, &members)?;
        state
            .culture
            .organizations
            .get_mut(&organization_id)
            .expect("organization ID collected from map")
            .culture_values = values;
    }
    Ok(())
}

/// Validate cached sums and temporal cultural records against canonical people.
pub fn validate_culture_state(
    config: &CultureConfig,
    state: &State,
    year: Year,
) -> Result<(), CultureError> {
    for person in state.persons.values() {
        for (key, policy) in &config.dimensions {
            cultural_value(person, key, policy)?;
        }
    }
    let expected = calculate_settlement_aggregates(config, state, year)?;
    if expected != state.culture.settlements {
        return Err(CultureStateError::InvalidState(
            "settlement cultural aggregates do not match eligible living members".into(),
        )
        .into());
    }

    let institution_ids: BTreeSet<_> = config
        .institutions
        .iter()
        .map(|definition| definition.id.clone())
        .collect();
    let mut affiliation_intervals: BTreeMap<(&InstitutionId, PersonId), Vec<YearInterval>> =
        BTreeMap::new();
    for affiliation in &state.culture.institution_memberships {
        if !affiliation.validity.is_valid()
            || affiliation.validity.start > year
            || affiliation
                .validity
                .end_exclusive
                .is_some_and(|end| end > year)
        {
            return Err(CultureStateError::InvalidState(
                "institution membership has an invalid or future interval".into(),
            )
            .into());
        }
        if affiliation.active_at_close(year)
            && !institution_ids.contains(&affiliation.institution_id)
        {
            return Err(CultureStateError::InvalidState(format!(
                "active membership references unknown institution {}",
                affiliation.institution_id.0
            ))
            .into());
        }
        let person = state
            .persons
            .get(&affiliation.person_id)
            .ok_or(CultureStateError::MissingPerson(affiliation.person_id))?;
        if !state.settlements.contains_key(&affiliation.settlement_id) {
            return Err(CultureStateError::InvalidState(
                "institution membership references unknown settlement".into(),
            )
            .into());
        }
        if affiliation.active_at_close(year) && !person.alive_at_close(year) {
            return Err(CultureStateError::InvalidState(
                "dead person has an active institution affiliation".into(),
            )
            .into());
        }
        affiliation_intervals
            .entry((&affiliation.institution_id, affiliation.person_id))
            .or_default()
            .push(affiliation.validity);
    }
    validate_non_overlapping_intervals(affiliation_intervals)?;
    let expected_active_affiliations = state
        .culture
        .institution_memberships
        .iter()
        .enumerate()
        .filter(|(_, membership)| membership.active_at_close(year))
        .fold(
            BTreeMap::<PersonId, BTreeMap<InstitutionId, u64>>::new(),
            |mut active, (index, membership)| {
                active.entry(membership.person_id).or_default().insert(
                    membership.institution_id.clone(),
                    u64::try_from(index).unwrap_or(u64::MAX),
                );
                active
            },
        );
    if expected_active_affiliations
        != state
            .culture
            .active_institution_membership_indices_by_person
    {
        return Err(CultureStateError::InvalidState(
            "active institution membership index disagrees with temporal intervals".into(),
        )
        .into());
    }

    let mut max_organization_id = None;
    for (organization_id, organization) in &state.culture.organizations {
        if *organization_id != organization.id {
            return Err(CultureStateError::InvalidState(
                "organization map key differs from entity ID".into(),
            )
            .into());
        }
        max_organization_id = Some(max_organization_id.unwrap_or(0_u64).max(organization_id.0));
        if !state.settlements.contains_key(&organization.settlement_id) {
            return Err(CultureStateError::InvalidState(
                "organization references unknown settlement".into(),
            )
            .into());
        }
        if organization.founded_year > year
            || organization
                .dissolved_year
                .is_some_and(|dissolved| dissolved > year || dissolved < organization.founded_year)
        {
            return Err(CultureStateError::InvalidState(
                "organization has an invalid or future lifetime".into(),
            )
            .into());
        }
        let mut history_founders = BTreeSet::new();
        let mut member_intervals: BTreeMap<PersonId, Vec<YearInterval>> = BTreeMap::new();
        let mut members_at_close = BTreeSet::new();
        for membership in &organization.memberships {
            if !membership.validity.is_valid()
                || membership.validity.start < organization.founded_year
                || membership.validity.start > year
                || membership
                    .validity
                    .end_exclusive
                    .is_some_and(|end| end > year)
            {
                return Err(CultureStateError::InvalidState(
                    "organization membership has an invalid or future interval".into(),
                )
                .into());
            }
            let person = state
                .persons
                .get(&membership.person_id)
                .ok_or(CultureStateError::MissingPerson(membership.person_id))?;
            if membership.founder {
                history_founders.insert(membership.person_id);
            }
            if membership.validity.contains(year) {
                if !person.alive_at_close(year) {
                    return Err(CultureStateError::InvalidState(
                        "dead person has an active organization membership".into(),
                    )
                    .into());
                }
                members_at_close.insert(membership.person_id);
            }
            member_intervals
                .entry(membership.person_id)
                .or_default()
                .push(membership.validity);
        }
        validate_non_overlapping_intervals(member_intervals)?;
        if history_founders != organization.founders {
            return Err(CultureStateError::InvalidState(
                "organization founders do not match its historical founder memberships".into(),
            )
            .into());
        }
        if members_at_close != organization.current_members {
            return Err(CultureStateError::InvalidState(
                "organization current-member index disagrees with temporal intervals".into(),
            )
            .into());
        }
        if !organization.active_at_close(year) && !organization.current_members.is_empty() {
            return Err(CultureStateError::InvalidState(
                "dissolved organization still has current members".into(),
            )
            .into());
        }
        if organization.active_at_close(year) {
            let expected_values =
                aggregate_people(config, state, year, &organization.current_members)?;
            if expected_values != organization.culture_values {
                return Err(CultureStateError::InvalidState(format!(
                    "organization {} cultural aggregate does not match its active members",
                    organization_id.0
                ))
                .into());
            }
        }
    }
    if max_organization_id.is_some_and(|max| state.culture.next_organization_id <= max) {
        return Err(CultureStateError::InvalidState(
            "next organization ID does not exceed the stored IDs".into(),
        )
        .into());
    }
    let expected_membership_index = state.culture.organizations.iter().fold(
        BTreeMap::<PersonId, BTreeSet<OrganizationId>>::new(),
        |mut index, (organization_id, organization)| {
            for person_id in &organization.current_members {
                index
                    .entry(*person_id)
                    .or_default()
                    .insert(*organization_id);
            }
            index
        },
    );
    if expected_membership_index != state.culture.organization_ids_by_person {
        return Err(CultureStateError::InvalidState(
            "organization reverse membership index disagrees with active memberships".into(),
        )
        .into());
    }
    Ok(())
}

fn aggregate_people(
    config: &CultureConfig,
    state: &State,
    year: Year,
    members: &BTreeSet<PersonId>,
) -> Result<BTreeMap<TraitKey, CultureAggregate>, CultureError> {
    let mut values: BTreeMap<_, _> = config
        .dimensions
        .keys()
        .cloned()
        .map(|key| (key, CultureAggregate::default()))
        .collect();
    for person_id in members {
        let person = state
            .persons
            .get(person_id)
            .ok_or(CultureStateError::MissingPerson(*person_id))?;
        for (key, policy) in &config.dimensions {
            if eligible(person, policy.eligibility, year) {
                values
                    .get_mut(key)
                    .expect("dimension initialized")
                    .add(cultural_value(person, key, policy)?, policy.member_weight)?;
            }
        }
    }
    Ok(values)
}

fn validate_non_overlapping_intervals<K: Ord>(
    mut intervals: BTreeMap<K, Vec<YearInterval>>,
) -> Result<(), CultureError> {
    for values in intervals.values_mut() {
        values.sort_by_key(|interval| (interval.start, interval.end_exclusive));
        let mut previous: Option<YearInterval> = None;
        for interval in values.iter().copied() {
            // Empty same-year intervals are valid records for intra-year
            // starts/ends and do not overlap neighboring memberships.
            if interval.end_exclusive == Some(interval.start) {
                continue;
            }
            if let Some(previous) = previous {
                if previous
                    .end_exclusive
                    .is_none_or(|end| end > interval.start)
                {
                    return Err(CultureStateError::InvalidState(
                        "temporal membership intervals overlap".into(),
                    )
                    .into());
                }
            }
            previous = Some(interval);
        }
    }
    Ok(())
}

fn calculate_settlement_aggregates(
    config: &CultureConfig,
    state: &State,
    year: Year,
) -> Result<BTreeMap<SettlementId, BTreeMap<TraitKey, CultureAggregate>>, CultureError> {
    let mut aggregates: BTreeMap<SettlementId, BTreeMap<TraitKey, CultureAggregate>> =
        BTreeMap::new();
    for settlement_id in state.settlements.keys().copied() {
        aggregates.insert(
            settlement_id,
            config
                .dimensions
                .keys()
                .cloned()
                .map(|key| (key, CultureAggregate::default()))
                .collect(),
        );
    }
    for person in state
        .persons
        .values()
        .filter(|person| person.alive_at_close(year))
    {
        let Some(settlement) = aggregates.get_mut(&person.settlement_id) else {
            return Err(CultureStateError::InvalidState(format!(
                "person {} references unknown settlement {}",
                person.id.0, person.settlement_id.0
            ))
            .into());
        };
        for (key, policy) in &config.dimensions {
            if !eligible(person, policy.eligibility, year) {
                continue;
            }
            let value = cultural_value(person, key, policy)?;
            settlement
                .get_mut(key)
                .expect("configured dimension was initialized")
                .add(value, policy.member_weight)?;
        }
    }
    Ok(aggregates)
}

/// Apply one death, birth, or migration to the settlement sums and close any
/// affiliation whose configured continuity ends at the change.
pub fn apply_population_delta(
    config: &CultureConfig,
    state: &mut State,
    year: Year,
    delta: CulturePopulationDelta,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), CultureError> {
    if delta.from == delta.to {
        return Ok(());
    }
    let person = state
        .persons
        .get(&delta.person_id)
        .cloned()
        .ok_or(CultureStateError::MissingPerson(delta.person_id))?;

    if let Some(from) = delta.from {
        adjust_settlement_person(config, state, &person, from, year, false)?;
    }

    if delta.to.is_none() || delta.from.is_some_and(|from| Some(from) != delta.to) {
        let reason = if delta.to.is_none() {
            InstitutionAffiliationEndReason::EndpointDeath
        } else {
            InstitutionAffiliationEndReason::LeftOnMigration
        };
        close_institution_affiliations(config, state, &person, year, reason, events)?;
        let organization_reason = if delta.to.is_none() {
            OrganizationMembershipEndReason::EndpointDeath
        } else {
            OrganizationMembershipEndReason::LeftOnMigration
        };
        if delta.to.is_none() || !config.organizations.continue_after_migration {
            close_organization_memberships(
                config,
                state,
                &person,
                year,
                organization_reason,
                events,
            )?;
        }
    }

    if let Some(to) = delta.to {
        if person.alive_at_close(year) {
            if person.settlement_id != to {
                return Err(CultureStateError::InvalidState(format!(
                    "person {} is in settlement {}, but movement targets {}",
                    person.id.0, person.settlement_id.0, to.0
                ))
                .into());
            }
            adjust_settlement_person(config, state, &person, to, year, true)?;
        }
    }
    Ok(())
}

fn adjust_settlement_person(
    config: &CultureConfig,
    state: &mut State,
    person: &Person,
    settlement_id: SettlementId,
    year: Year,
    add: bool,
) -> Result<(), CultureError> {
    let aggregates = state
        .culture
        .settlements
        .get_mut(&settlement_id)
        .ok_or_else(|| {
            CultureStateError::InvalidState(format!(
                "missing culture aggregate for settlement {}",
                settlement_id.0
            ))
        })?;
    for (key, policy) in &config.dimensions {
        if !eligible_by_age(person, policy.eligibility, year)
            || (add && !person.alive_at_close(year))
        {
            continue;
        }
        let value = cultural_value(person, key, policy)?;
        let aggregate = aggregates.get_mut(key).ok_or_else(|| {
            CultureStateError::InvalidState(format!(
                "missing aggregate for cultural dimension {}::{}",
                key.namespace, key.name
            ))
        })?;
        if add {
            aggregate.add(value, policy.member_weight)?;
        } else {
            aggregate.remove(value, policy.member_weight)?;
        }
    }
    Ok(())
}

/// Maintain aggregate eligibility as people cross configured age bands.
/// Called once at A0, before mortality for the new year.
pub fn apply_culture_age_transitions(
    config: &CultureConfig,
    state: &mut State,
    from_year: Year,
    to_year: Year,
) -> Result<(), CultureError> {
    if to_year.0 != from_year.0.saturating_add(1) {
        return Err(CultureStateError::InvalidState(
            "culture age transition must advance exactly one year".into(),
        )
        .into());
    }
    let age_dimensions: Vec<_> = config
        .dimensions
        .iter()
        .filter(|(_, policy)| !matches!(policy.eligibility, CultureEligibility::AllLiving))
        .collect();
    if age_dimensions.is_empty() {
        return Ok(());
    }

    // Keep only boundary crossings. Cloning full Person rows (which include
    // genomes and every trait) each year is prohibitive for large worlds.
    let mut changes = Vec::new();
    for person in state.persons.values() {
        let was_alive = person.alive_at_close(from_year);
        let will_be_alive = person.alive_at_start(to_year);
        if !was_alive && !will_be_alive {
            continue;
        }
        for (key, policy) in &age_dimensions {
            let was_eligible = was_alive
                && person
                    .age_at_close(from_year)
                    .is_some_and(|age| policy.eligibility.includes_age(age));
            let will_be_eligible = will_be_alive
                && person
                    .age_at_close(to_year)
                    .is_some_and(|age| policy.eligibility.includes_age(age));
            if was_eligible == will_be_eligible {
                continue;
            }
            let value = cultural_value(person, key, policy)?;
            changes.push((
                person.id,
                person.settlement_id,
                (*key).clone(),
                value,
                policy.member_weight,
                will_be_eligible,
            ));
        }
    }
    for (person_id, settlement_id, key, value, weight, add) in changes {
        adjust_one_dimension(state, settlement_id, &key, value, weight, add)?;
        if let Some(organization_ids) = state.culture.organization_ids_by_person.get(&person_id) {
            for organization_id in organization_ids {
                if let Some(organization) = state.culture.organizations.get_mut(organization_id) {
                    adjust_org_one_dimension(organization, &key, value, weight, add)?;
                }
            }
        }
    }
    Ok(())
}

fn adjust_one_dimension(
    state: &mut State,
    settlement_id: SettlementId,
    key: &TraitKey,
    value: i16,
    weight: u32,
    add: bool,
) -> Result<(), CultureError> {
    let aggregate = state
        .culture
        .settlements
        .get_mut(&settlement_id)
        .and_then(|values| values.get_mut(key))
        .ok_or_else(|| {
            CultureStateError::InvalidState(format!(
                "missing aggregate for dimension {}::{} in settlement {}",
                key.namespace, key.name, settlement_id.0
            ))
        })?;
    if add {
        aggregate.add(value, weight)?;
    } else {
        aggregate.remove(value, weight)?;
    }
    Ok(())
}

fn adjust_org_one_dimension(
    organization: &mut Organization,
    key: &TraitKey,
    value: i16,
    weight: u32,
    add: bool,
) -> Result<(), CultureError> {
    let aggregate = organization.culture_values.entry(key.clone()).or_default();
    if add {
        aggregate.add(value, weight)?;
    } else {
        aggregate.remove(value, weight)?;
    }
    Ok(())
}

/// Prepare every newborn cultural value from one immutable pre-birth snapshot.
/// The callback is supplied by the engine's key-based RNG and must return an
/// offset within `[-variation_max, variation_max]`.
pub fn prepare_birth_culture_batch<F>(
    config: &CultureConfig,
    state: &State,
    year: Year,
    births: &[CultureBirthInput],
    mut variation: F,
) -> Result<Vec<CultureBirthAssignment>, CultureError>
where
    F: FnMut(chargen_domain::LogicalId, &TraitKey, u16) -> i16,
{
    let local_snapshot = state.culture.settlements.clone();
    let mut seen = BTreeSet::new();
    let mut assignments = Vec::with_capacity(births.len());
    for birth in births {
        if !seen.insert(birth.person_id) {
            return Err(CultureError::DuplicateBirth(birth.person_id));
        }
        if !state.settlements.contains_key(&birth.settlement_id) {
            return Err(CultureStateError::InvalidState(format!(
                "newborn {} references unknown settlement {}",
                birth.person_id.0, birth.settlement_id.0
            ))
            .into());
        }
        let mut values = BTreeMap::new();
        for (key, policy) in &config.dimensions {
            let parent_value = weighted_parent_mean(state, &birth.genetic_parents, key, policy)?;
            let local_value = local_snapshot
                .get(&birth.settlement_id)
                .and_then(|values| values.get(key))
                .and_then(|aggregate| aggregate.value());
            let value = weighted_cultural_base(parent_value, local_value, policy)?;
            let offset = i32::from(variation(birth.logical_id, key, policy.variation_max)).clamp(
                -i32::from(policy.variation_max),
                i32::from(policy.variation_max),
            );
            let final_value = (i32::from(value) + offset)
                .clamp(i32::from(policy.minimum), i32::from(policy.maximum))
                as i16;
            values.insert(key.clone(), final_value);
        }
        assignments.push(CultureBirthAssignment {
            person_id: birth.person_id,
            values,
        });
    }
    let _ = year; // Birth year is used by the engine event; this pure step only needs its close snapshot.
    Ok(assignments)
}

fn weighted_parent_mean(
    state: &State,
    parent_ids: &[PersonId],
    key: &TraitKey,
    policy: &CultureDimensionPolicy,
) -> Result<Option<i16>, CultureError> {
    if policy.parent_weight == 0 || parent_ids.is_empty() {
        return Ok(None);
    }
    let unique: BTreeSet<_> = parent_ids.iter().copied().collect();
    let mut sum = 0_i128;
    let mut count = 0_i128;
    for parent_id in unique {
        let parent = state
            .persons
            .get(&parent_id)
            .ok_or(CultureError::MissingParent(parent_id))?;
        if let Some(value) = parent.traits.get(key).copied() {
            sum += i128::from(value);
            count += 1;
        }
    }
    if count == 0 {
        Ok(None)
    } else {
        Ok(Some(round_ratio(sum, count) as i16))
    }
}

fn weighted_cultural_base(
    parent: Option<i16>,
    local: Option<i16>,
    policy: &CultureDimensionPolicy,
) -> Result<i16, CultureError> {
    let mut weighted_sum = 0_i128;
    let mut total_weight = 0_i128;
    if let Some(value) = parent.filter(|_| policy.parent_weight > 0) {
        weighted_sum += i128::from(value) * i128::from(policy.parent_weight);
        total_weight += i128::from(policy.parent_weight);
    }
    if let Some(value) = local.filter(|_| policy.local_weight > 0) {
        weighted_sum += i128::from(value) * i128::from(policy.local_weight);
        total_weight += i128::from(policy.local_weight);
    }
    if total_weight == 0 {
        return policy
            .founder_value
            .ok_or_else(|| {
                CultureStateError::InvalidConfiguration(
                    "newborn has no parent/local cultural source and no founder value".into(),
                )
            })
            .map_err(Into::into);
    }
    // Birth composition uses integer fixed-point truncation. This makes the
    // result independent of host rounding modes (e.g. 66.66... becomes 66).
    let value =
        (weighted_sum / total_weight).clamp(i128::from(policy.minimum), i128::from(policy.maximum));
    Ok(value as i16)
}

fn round_ratio(numerator: i128, denominator: i128) -> i128 {
    if numerator < 0 {
        -((-numerator + denominator / 2) / denominator)
    } else {
        (numerator + denominator / 2) / denominator
    }
}

/// Apply the prepared cohort after all births and infant deaths have resolved.
/// Newborn traits are set as a batch; only children alive at close affect sums.
pub fn apply_birth_culture_batch(
    config: &CultureConfig,
    state: &mut State,
    year: Year,
    assignments: &[CultureBirthAssignment],
) -> Result<(), CultureError> {
    let mut seen = BTreeSet::new();
    for assignment in assignments {
        if !seen.insert(assignment.person_id) {
            return Err(CultureError::DuplicateBirth(assignment.person_id));
        }
        if assignment.values.len() != config.dimensions.len()
            || config
                .dimensions
                .keys()
                .any(|key| !assignment.values.contains_key(key))
        {
            return Err(CultureStateError::InvalidState(format!(
                "newborn {} has an incomplete cultural assignment",
                assignment.person_id.0
            ))
            .into());
        }
        let person = state
            .persons
            .get(&assignment.person_id)
            .ok_or(CultureStateError::MissingPerson(assignment.person_id))?;
        if person.birth_year != year {
            return Err(CultureStateError::InvalidState(format!(
                "cultural assignment may only change values at birth (person {}, birth year {})",
                person.id.0, person.birth_year.0
            ))
            .into());
        }
        for (key, value) in &assignment.values {
            validate_cultural_value(&config.dimensions[key], assignment.person_id, key, *value)?;
        }
    }

    for assignment in assignments {
        let person = state
            .persons
            .get_mut(&assignment.person_id)
            .expect("newborns were checked above");
        for (key, value) in &assignment.values {
            person.traits.insert(key.clone(), *value);
        }
    }

    for assignment in assignments {
        let person = state
            .persons
            .get(&assignment.person_id)
            .expect("newborns were checked above")
            .clone();
        if person.alive_at_close(year) {
            adjust_settlement_person(config, state, &person, person.settlement_id, year, true)?;
        }
    }
    Ok(())
}

/// Run A10: preconfigured institution/religion affiliation and bounded
/// organization recruitment, founding, and dissolution.
pub fn apply_a10_culture(
    config: &CultureConfig,
    state: &mut State,
    year: Year,
    seed: u64,
    rules_version: u32,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), CultureError> {
    let mut people_by_settlement: BTreeMap<SettlementId, Vec<PersonId>> = BTreeMap::new();
    for person in state.living_people_at(year) {
        people_by_settlement
            .entry(person.settlement_id)
            .or_default()
            .push(person.id);
    }
    for people in people_by_settlement.values_mut() {
        people.sort_by_key(|id| state.persons[id].logical_id);
    }

    apply_institution_affiliations(
        config,
        state,
        year,
        seed,
        rules_version,
        &people_by_settlement,
        events,
    )?;
    apply_organization_memberships(
        config,
        state,
        year,
        seed,
        rules_version,
        &people_by_settlement,
        events,
    )?;
    Ok(())
}

fn apply_institution_affiliations(
    config: &CultureConfig,
    state: &mut State,
    year: Year,
    seed: u64,
    rules_version: u32,
    people_by_settlement: &BTreeMap<SettlementId, Vec<PersonId>>,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), CultureError> {
    let mut active_pairs = BTreeSet::new();
    let mut active_counts: BTreeMap<PersonId, usize> = BTreeMap::new();
    for (person_id, institutions) in &state
        .culture
        .active_institution_membership_indices_by_person
    {
        active_counts.insert(*person_id, institutions.len());
        active_pairs.extend(
            institutions
                .keys()
                .cloned()
                .map(|institution_id| (institution_id, *person_id)),
        );
    }
    let institutions = config.institutions.clone();
    for institution in institutions {
        let policy = &institution.affiliation;
        if !policy.enabled || policy.opportunity_ppm == 0 {
            continue;
        }
        for (settlement_id, people) in people_by_settlement {
            if institution
                .allowed_settlements
                .as_ref()
                .is_some_and(|allowed| !allowed.contains(settlement_id))
            {
                continue;
            }
            let eligible_candidates = people.iter().copied().filter(|person_id| {
                let person = &state.persons[person_id];
                let age = person.age_at_close(year).unwrap_or(u32::MAX);
                !active_pairs.contains(&(institution.id.clone(), *person_id))
                    && age >= u32::from(policy.minimum_age)
                    && policy.maximum_age.is_none_or(|max| age <= u32::from(max))
                    && active_counts.get(person_id).copied().unwrap_or(0)
                        < policy.maximum_affiliations_per_person.unwrap_or(u16::MAX) as usize
            });
            let candidates = sample_people(
                state,
                eligible_candidates,
                usize::from(policy.candidate_limit),
                |person| {
                    super::keyed_random_u64(
                        seed,
                        rules_version,
                        super::DecisionDomain::InstitutionAffiliation,
                        year,
                        person.logical_id,
                        stable_string_key(&institution.id.0),
                    )
                },
            );
            for person_id in candidates {
                let person = state.persons[&person_id].clone();
                if affinity_to_values(config, &person, &institution.values)?
                    < policy.minimum_affinity_ppm
                {
                    continue;
                }
                if !passes_probability(
                    seed,
                    rules_version,
                    super::DecisionDomain::InstitutionAffiliation,
                    year,
                    person.logical_id,
                    stable_string_key(&institution.id.0).wrapping_add(1),
                    policy.opportunity_ppm,
                ) {
                    continue;
                }
                let membership = InstitutionMembership {
                    institution_id: institution.id.clone(),
                    person_id,
                    settlement_id: *settlement_id,
                    validity: YearInterval {
                        start: year,
                        end_exclusive: None,
                    },
                };
                let index = u64::try_from(state.culture.institution_memberships.len())
                    .map_err(|_| CultureStateError::Overflow)?;
                state.culture.institution_memberships.push(membership);
                state
                    .culture
                    .active_institution_membership_indices_by_person
                    .entry(person_id)
                    .or_default()
                    .insert(institution.id.clone(), index);
                active_pairs.insert((institution.id.clone(), person_id));
                *active_counts.entry(person_id).or_default() += 1;
                emit_event(
                    CultureEventContext {
                        year,
                        seed,
                        rules_version,
                    },
                    person.logical_id,
                    events,
                    vec![
                        EntityRef::World(state.world_id),
                        EntityRef::Person(person_id),
                        EntityRef::Settlement(*settlement_id),
                    ],
                    HistoricalEventKind::InstitutionAffiliationStarted {
                        institution_id: institution.id.clone(),
                        person_id,
                        settlement_id: Some(*settlement_id),
                    },
                )?;
            }
        }
    }
    Ok(())
}

fn apply_organization_memberships(
    config: &CultureConfig,
    state: &mut State,
    year: Year,
    seed: u64,
    rules_version: u32,
    people_by_settlement: &BTreeMap<SettlementId, Vec<PersonId>>,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), CultureError> {
    let policy = &config.organizations;
    if !policy.enabled {
        return Ok(());
    }

    let existing_ids: Vec<_> = state.culture.organizations.keys().copied().collect();
    for organization_id in existing_ids {
        let Some(org) = state.culture.organizations.get(&organization_id) else {
            continue;
        };
        if !org.active_at_close(year) {
            continue;
        }
        if org.current_members.len() < usize::from(policy.dissolve_below_members) {
            dissolve_organization(
                state,
                organization_id,
                CultureEventContext {
                    year,
                    seed,
                    rules_version,
                },
                events,
            )?;
            continue;
        }
        let Some(candidate_ids) = people_by_settlement.get(&org.settlement_id) else {
            continue;
        };
        let eligible_candidates = candidate_ids
            .iter()
            .copied()
            .filter(|person_id| !org.current_members.contains(person_id))
            .filter(|person_id| organization_age_eligible(&state.persons[person_id], policy, year));
        let candidates = sample_people(
            state,
            eligible_candidates,
            usize::from(policy.candidate_limit_per_settlement),
            |person| {
                super::keyed_random_u64(
                    seed,
                    rules_version,
                    super::DecisionDomain::OrganizationCandidate,
                    year,
                    person.logical_id,
                    org.logical_id.0,
                )
            },
        );
        for person_id in candidates {
            let can_join = state
                .culture
                .organizations
                .get(&organization_id)
                .is_some_and(|org| org.current_members.len() < usize::from(policy.maximum_members));
            if !can_join {
                break;
            }
            let person = state.persons[&person_id].clone();
            if affinity_to_org(
                config,
                &person,
                &state.culture.organizations[&organization_id],
            )? < policy.minimum_affinity_ppm
            {
                continue;
            }
            if passes_probability(
                seed,
                rules_version,
                super::DecisionDomain::OrganizationCandidate,
                year,
                person.logical_id,
                organization_id.0,
                policy.formation_opportunity_ppm,
            ) {
                start_organization_membership(
                    config,
                    state,
                    organization_id,
                    person_id,
                    CultureEventContext {
                        year,
                        seed,
                        rules_version,
                    },
                    false,
                    events,
                )?;
            }
        }
    }

    for settlement_id in state.settlements.keys().copied().collect::<Vec<_>>() {
        let counter = state
            .culture
            .organization_cadence_counters
            .entry(settlement_id)
            .or_default();
        let due = (*counter).is_multiple_of(u32::from(policy.cadence_years));
        *counter = counter.saturating_add(1);
        if !due {
            continue;
        }
        let active_count = state
            .culture
            .organizations
            .values()
            .filter(|org| org.settlement_id == settlement_id && org.active_at_close(year))
            .count();
        if active_count >= usize::from(policy.maximum_organizations_per_settlement) {
            continue;
        }
        let Some(residents) = people_by_settlement.get(&settlement_id) else {
            continue;
        };
        let eligible_residents = residents
            .iter()
            .copied()
            .filter(|person_id| organization_age_eligible(&state.persons[person_id], policy, year));
        let candidates = sample_people(
            state,
            eligible_residents,
            usize::from(policy.candidate_limit_per_settlement),
            |person| {
                super::keyed_random_u64(
                    seed,
                    rules_version,
                    super::DecisionDomain::OrganizationCandidate,
                    year,
                    person.logical_id,
                    settlement_id.0,
                )
            },
        );
        let Some(seed_person_id) = candidates.first().copied() else {
            continue;
        };
        let seed_person = state.persons[&seed_person_id].clone();
        let mut founders = Vec::new();
        for candidate_id in candidates {
            let candidate = &state.persons[&candidate_id];
            if affinity_to_person(config, &seed_person, candidate)? < policy.minimum_affinity_ppm {
                continue;
            }
            if passes_probability(
                seed,
                rules_version,
                super::DecisionDomain::OrganizationCandidate,
                year,
                candidate.logical_id,
                seed_person.logical_id.0,
                policy.formation_opportunity_ppm,
            ) {
                founders.push(candidate_id);
            }
            if founders.len() >= usize::from(policy.maximum_members) {
                break;
            }
        }
        if founders.len() < usize::from(policy.minimum_founders) {
            continue;
        }
        create_organization(
            config,
            state,
            CultureEventContext {
                year,
                seed,
                rules_version,
            },
            settlement_id,
            founders,
            events,
        )?;
    }
    Ok(())
}

fn create_organization(
    config: &CultureConfig,
    state: &mut State,
    event_context: CultureEventContext,
    settlement_id: SettlementId,
    founders: Vec<PersonId>,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), CultureError> {
    let id = OrganizationId(state.culture.next_organization_id);
    state.culture.next_organization_id = state
        .culture
        .next_organization_id
        .checked_add(1)
        .ok_or(CultureStateError::Overflow)?;
    let first_founder = *founders
        .first()
        .ok_or_else(|| CultureStateError::InvalidState("organization requires founders".into()))?;
    let founder_logical_id = state.persons[&first_founder].logical_id;
    let settlement_logical_id = state.settlements[&settlement_id].logical_id;
    let logical_id = chargen_domain::LogicalId(super::keyed_random_u64(
        event_context.seed,
        event_context.rules_version,
        super::DecisionDomain::OrganizationIdentity,
        event_context.year,
        settlement_logical_id,
        founder_logical_id.0,
    ));
    let settlement_name = &state.settlements[&settlement_id].name;
    let name = format!("Collective at {settlement_name} ({})", event_context.year.0);
    let mut organization = Organization {
        id,
        logical_id,
        name,
        settlement_id,
        founded_year: event_context.year,
        dissolved_year: None,
        founders: founders.iter().copied().collect(),
        memberships: Vec::with_capacity(founders.len()),
        current_members: BTreeSet::new(),
        culture_values: config
            .dimensions
            .keys()
            .cloned()
            .map(|key| (key, CultureAggregate::default()))
            .collect(),
    };
    for person_id in &founders {
        let person = &state.persons[person_id];
        organization.memberships.push(OrganizationMembership {
            person_id: *person_id,
            validity: YearInterval {
                start: event_context.year,
                end_exclusive: None,
            },
            founder: true,
        });
        organization.current_members.insert(*person_id);
        add_person_to_org_aggregate(config, &mut organization, person, event_context.year)?;
    }
    state.culture.organizations.insert(id, organization);
    let founders_sorted: Vec<_> = state.culture.organizations[&id]
        .founders
        .iter()
        .copied()
        .collect();
    for person_id in &founders_sorted {
        state
            .culture
            .organization_ids_by_person
            .entry(*person_id)
            .or_default()
            .insert(id);
    }
    let mut entities = vec![
        EntityRef::World(state.world_id),
        EntityRef::Settlement(settlement_id),
        EntityRef::Organization(id),
    ];
    entities.extend(founders_sorted.iter().copied().map(EntityRef::Person));
    emit_event(
        event_context,
        logical_id,
        events,
        entities,
        HistoricalEventKind::OrganizationFounded {
            organization_id: id,
            settlement_id,
            founders: founders_sorted.clone(),
        },
    )?;
    for person_id in founders_sorted {
        emit_organization_membership_event(
            state,
            event_context,
            id,
            person_id,
            events,
            OrganizationMembershipEvent::Started { founder: true },
        )?;
    }
    Ok(())
}

fn start_organization_membership(
    config: &CultureConfig,
    state: &mut State,
    organization_id: OrganizationId,
    person_id: PersonId,
    event_context: CultureEventContext,
    founder: bool,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), CultureError> {
    let person = state
        .persons
        .get(&person_id)
        .cloned()
        .ok_or(CultureStateError::MissingPerson(person_id))?;
    let organization = state
        .culture
        .organizations
        .get_mut(&organization_id)
        .ok_or_else(|| CultureStateError::InvalidState("unknown organization".into()))?;
    if organization.current_members.contains(&person_id) {
        return Ok(());
    }
    organization.memberships.push(OrganizationMembership {
        person_id,
        validity: YearInterval {
            start: event_context.year,
            end_exclusive: None,
        },
        founder,
    });
    organization.current_members.insert(person_id);
    add_person_to_org_aggregate(config, organization, &person, event_context.year)?;
    if founder {
        organization.founders.insert(person_id);
    }
    state
        .culture
        .organization_ids_by_person
        .entry(person_id)
        .or_default()
        .insert(organization_id);
    emit_organization_membership_event(
        state,
        event_context,
        organization_id,
        person_id,
        events,
        OrganizationMembershipEvent::Started { founder },
    )?;
    Ok(())
}

fn close_organization_memberships(
    config: &CultureConfig,
    state: &mut State,
    person: &Person,
    year: Year,
    reason: OrganizationMembershipEndReason,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), CultureError> {
    let organization_ids: Vec<_> = state
        .culture
        .organization_ids_by_person
        .get(&person.id)
        .into_iter()
        .flat_map(|ids| ids.iter().copied())
        .collect();
    for organization_id in organization_ids {
        {
            let organization = state
                .culture
                .organizations
                .get_mut(&organization_id)
                .expect("collected from this map");
            organization.current_members.remove(&person.id);
            let membership = organization
                .memberships
                .iter_mut()
                .rev()
                .find(|membership| {
                    membership.person_id == person.id && membership.validity.end_exclusive.is_none()
                })
                .ok_or_else(|| {
                    CultureStateError::InvalidState(format!(
                        "current member {} has no open membership interval",
                        person.id.0
                    ))
                })?;
            membership.validity.end_exclusive = Some(year);
            for (key, policy) in &config.dimensions {
                if eligible_by_age(person, policy.eligibility, year) {
                    let value = cultural_value(person, key, policy)?;
                    adjust_org_one_dimension(
                        organization,
                        key,
                        value,
                        policy.member_weight,
                        false,
                    )?;
                }
            }
        }
        let empty = state
            .culture
            .organization_ids_by_person
            .get_mut(&person.id)
            .is_some_and(|ids| {
                ids.remove(&organization_id);
                ids.is_empty()
            });
        if empty {
            state.culture.organization_ids_by_person.remove(&person.id);
        }
        emit_organization_membership_event(
            state,
            CultureEventContext {
                year,
                seed: state.seed,
                rules_version: state.rules_version,
            },
            organization_id,
            person.id,
            events,
            OrganizationMembershipEvent::Ended { reason },
        )?;
    }
    Ok(())
}

fn dissolve_organization(
    state: &mut State,
    organization_id: OrganizationId,
    event_context: CultureEventContext,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), CultureError> {
    let (person_ids, logical_id, settlement_id) = {
        let organization = state
            .culture
            .organizations
            .get_mut(&organization_id)
            .ok_or_else(|| CultureStateError::InvalidState("unknown organization".into()))?;
        if organization.dissolved_year.is_some() {
            return Ok(());
        }
        let person_ids: Vec<_> = organization.current_members.iter().copied().collect();
        for membership in &mut organization.memberships {
            if membership.validity.end_exclusive.is_none() {
                membership.validity.end_exclusive = Some(event_context.year);
            }
        }
        organization.current_members.clear();
        organization.culture_values.clear();
        organization.dissolved_year = Some(event_context.year);
        (
            person_ids,
            organization.logical_id,
            organization.settlement_id,
        )
    };
    for person_id in person_ids {
        let empty = state
            .culture
            .organization_ids_by_person
            .get_mut(&person_id)
            .is_some_and(|ids| {
                ids.remove(&organization_id);
                ids.is_empty()
            });
        if empty {
            state.culture.organization_ids_by_person.remove(&person_id);
        }
        emit_organization_membership_event(
            state,
            event_context,
            organization_id,
            person_id,
            events,
            OrganizationMembershipEvent::Ended {
                reason: OrganizationMembershipEndReason::OrganizationDissolved,
            },
        )?;
    }
    emit_event(
        event_context,
        logical_id,
        events,
        vec![
            EntityRef::World(state.world_id),
            EntityRef::Organization(organization_id),
            EntityRef::Settlement(settlement_id),
        ],
        HistoricalEventKind::OrganizationDissolved {
            organization_id,
            reason: OrganizationDissolutionReason::BelowMemberThreshold,
        },
    )?;
    Ok(())
}

fn close_institution_affiliations(
    config: &CultureConfig,
    state: &mut State,
    person: &Person,
    year: Year,
    reason: InstitutionAffiliationEndReason,
    events: &mut Vec<HistoricalEvent>,
) -> Result<(), CultureError> {
    let definitions: BTreeMap<_, _> = config
        .institutions
        .iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect();
    let active = state
        .culture
        .active_institution_membership_indices_by_person
        .get(&person.id)
        .cloned()
        .unwrap_or_default();
    let mut ended = Vec::new();
    for (institution_id, index) in active {
        if reason == InstitutionAffiliationEndReason::LeftOnMigration
            && definitions
                .get(&institution_id)
                .is_some_and(|definition| definition.affiliation.continue_after_migration)
        {
            continue;
        }
        let index = usize::try_from(index).map_err(|_| CultureStateError::Overflow)?;
        let membership = state
            .culture
            .institution_memberships
            .get_mut(index)
            .ok_or_else(|| {
                CultureStateError::InvalidState(
                    "active institution index references a missing membership".into(),
                )
            })?;
        if membership.person_id != person.id
            || membership.institution_id != institution_id
            || !membership.active_at_close(year)
        {
            return Err(CultureStateError::InvalidState(
                "active institution index references a different or inactive membership".into(),
            )
            .into());
        }
        membership.validity.end_exclusive = Some(year);
        ended.push((institution_id, membership.settlement_id));
    }
    if !ended.is_empty() {
        let active_by_person = state
            .culture
            .active_institution_membership_indices_by_person
            .get_mut(&person.id)
            .expect("active affiliations were collected above");
        for (institution_id, _) in &ended {
            active_by_person.remove(institution_id);
        }
        if active_by_person.is_empty() {
            state
                .culture
                .active_institution_membership_indices_by_person
                .remove(&person.id);
        }
    }
    for (institution_id, settlement_id) in ended {
        emit_event(
            CultureEventContext {
                year,
                seed: state.seed,
                rules_version: state.rules_version,
            },
            person.logical_id,
            events,
            vec![
                EntityRef::World(state.world_id),
                EntityRef::Person(person.id),
                EntityRef::Settlement(settlement_id),
            ],
            HistoricalEventKind::InstitutionAffiliationEnded {
                institution_id,
                person_id: person.id,
                settlement_id: Some(settlement_id),
                reason,
            },
        )?;
    }
    Ok(())
}

fn add_person_to_org_aggregate(
    config: &CultureConfig,
    organization: &mut Organization,
    person: &Person,
    year: Year,
) -> Result<(), CultureError> {
    for (key, policy) in &config.dimensions {
        if eligible(person, policy.eligibility, year) {
            let value = cultural_value(person, key, policy)?;
            organization
                .culture_values
                .entry(key.clone())
                .or_default()
                .add(value, policy.member_weight)?;
        }
    }
    Ok(())
}

fn affinity_to_org(
    config: &CultureConfig,
    person: &Person,
    organization: &Organization,
) -> Result<u32, CultureError> {
    let reference: BTreeMap<_, _> = organization
        .culture_values
        .iter()
        .filter_map(|(key, aggregate)| aggregate.value().map(|value| (key.clone(), value)))
        .collect();
    affinity_to_values(config, person, &reference)
}

fn affinity_to_person(
    config: &CultureConfig,
    left: &Person,
    right: &Person,
) -> Result<u32, CultureError> {
    let mut total_similarity = 0_i128;
    let mut total_weight = 0_i128;
    for (key, policy) in &config.dimensions {
        let (Some(left_value), Some(right_value)) = (
            left.traits.get(key).copied(),
            right.traits.get(key).copied(),
        ) else {
            continue;
        };
        let similarity = similarity_ppm(left_value, right_value, policy);
        total_similarity += i128::from(similarity) * i128::from(policy.member_weight);
        total_weight += i128::from(policy.member_weight);
    }
    if total_weight == 0 {
        Ok(0)
    } else {
        Ok((total_similarity / total_weight) as u32)
    }
}

fn affinity_to_values(
    config: &CultureConfig,
    person: &Person,
    reference: &BTreeMap<TraitKey, i16>,
) -> Result<u32, CultureError> {
    let mut total_similarity = 0_i128;
    let mut total_weight = 0_i128;
    for (key, reference_value) in reference {
        let Some(policy) = config.dimensions.get(key) else {
            continue;
        };
        let value = cultural_value(person, key, policy)?;
        total_similarity += i128::from(similarity_ppm(value, *reference_value, policy))
            * i128::from(policy.member_weight);
        total_weight += i128::from(policy.member_weight);
    }
    if total_weight == 0 {
        Ok(0)
    } else {
        Ok((total_similarity / total_weight) as u32)
    }
}

fn similarity_ppm(left: i16, right: i16, policy: &CultureDimensionPolicy) -> u32 {
    let range = i64::from(policy.maximum) - i64::from(policy.minimum);
    if range == 0 {
        return if left == right { 1_000_000 } else { 0 };
    }
    let distance = (i64::from(left) - i64::from(right)).abs().min(range);
    (((range - distance) * 1_000_000) / range) as u32
}

fn eligible(person: &Person, rule: CultureEligibility, year: Year) -> bool {
    person.alive_at_close(year) && eligible_by_age(person, rule, year)
}

fn eligible_by_age(person: &Person, rule: CultureEligibility, year: Year) -> bool {
    person
        .age_at_close(year)
        .is_some_and(|age| rule.includes_age(age))
}

fn organization_age_eligible(
    person: &Person,
    policy: &chargen_domain::OrganizationPolicy,
    year: Year,
) -> bool {
    person.age_at_close(year).is_some_and(|age| {
        age >= u32::from(policy.minimum_age)
            && policy
                .maximum_age
                .is_none_or(|maximum| age <= u32::from(maximum))
    })
}

/// Select at most `limit` candidates by a stable random rank without first
/// materializing and sorting the entire population. This keeps bounded A10
/// decisions bounded in both allocation and ordering work.
fn sample_people<I, F>(state: &State, people: I, limit: usize, mut rank: F) -> Vec<PersonId>
where
    I: IntoIterator<Item = PersonId>,
    F: FnMut(&Person) -> u64,
{
    if limit == 0 {
        return Vec::new();
    }
    let mut selected: BinaryHeap<(u64, chargen_domain::LogicalId, PersonId)> =
        BinaryHeap::with_capacity(limit.min(1024));
    for person_id in people {
        let Some(person) = state.persons.get(&person_id) else {
            continue;
        };
        let candidate = (rank(person), person.logical_id, person.id);
        if selected.len() < limit {
            selected.push(candidate);
        } else if selected.peek().is_some_and(|worst| candidate < *worst) {
            selected.pop();
            selected.push(candidate);
        }
    }
    let mut ordered: Vec<_> = selected.into_iter().collect();
    ordered.sort_unstable();
    ordered
        .into_iter()
        .map(|(_, _, person_id)| person_id)
        .collect()
}

fn cultural_value(
    person: &Person,
    key: &TraitKey,
    policy: &CultureDimensionPolicy,
) -> Result<i16, CultureError> {
    let value = person
        .traits
        .get(key)
        .copied()
        .ok_or_else(|| CultureStateError::MissingTrait {
            person_id: person.id,
            key: key.clone(),
        })?;
    validate_cultural_value(policy, person.id, key, value)?;
    Ok(value)
}

fn validate_cultural_value(
    policy: &CultureDimensionPolicy,
    person_id: PersonId,
    key: &TraitKey,
    value: i16,
) -> Result<(), CultureError> {
    if !(policy.minimum..=policy.maximum).contains(&value) {
        return Err(CultureStateError::InvalidState(format!(
            "person {} value for {}::{} is outside cultural bounds",
            person_id.0, key.namespace, key.name
        ))
        .into());
    }
    Ok(())
}

fn passes_probability(
    seed: u64,
    rules_version: u32,
    domain: super::DecisionDomain,
    year: Year,
    logical_id: chargen_domain::LogicalId,
    ordinal: u64,
    probability_ppm: u32,
) -> bool {
    probability_ppm >= 1_000_000
        || (probability_ppm > 0
            && super::keyed_random_u64(seed, rules_version, domain, year, logical_id, ordinal)
                % 1_000_000
                < u64::from(probability_ppm))
}

fn stable_string_key(value: &str) -> u64 {
    value.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn emit_organization_membership_event(
    state: &State,
    event_context: CultureEventContext,
    organization_id: OrganizationId,
    person_id: PersonId,
    events: &mut Vec<HistoricalEvent>,
    action: OrganizationMembershipEvent,
) -> Result<(), CultureError> {
    let organization = &state.culture.organizations[&organization_id];
    let person = &state.persons[&person_id];
    let kind = match action {
        OrganizationMembershipEvent::Started { founder } => {
            HistoricalEventKind::OrganizationMembershipStarted {
                organization_id,
                person_id,
                founder,
            }
        }
        OrganizationMembershipEvent::Ended { reason } => {
            HistoricalEventKind::OrganizationMembershipEnded {
                organization_id,
                person_id,
                reason,
            }
        }
    };
    emit_event(
        event_context,
        person.logical_id,
        events,
        vec![
            EntityRef::World(state.world_id),
            EntityRef::Person(person_id),
            EntityRef::Settlement(organization.settlement_id),
            EntityRef::Organization(organization_id),
        ],
        kind,
    )
}

#[derive(Clone, Copy, Debug)]
enum OrganizationMembershipEvent {
    Started {
        founder: bool,
    },
    Ended {
        reason: OrganizationMembershipEndReason,
    },
}

fn emit_event(
    event_context: CultureEventContext,
    subject: chargen_domain::LogicalId,
    events: &mut Vec<HistoricalEvent>,
    affected_entities: Vec<EntityRef>,
    kind: HistoricalEventKind,
) -> Result<(), CultureError> {
    let order = u32::try_from(events.len()).map_err(|_| CultureError::EventOrderOverflow)?;
    let mut ordinal = u64::from(order);
    let id = loop {
        let candidate = chargen_domain::LogicalId(super::keyed_random_u64(
            event_context.seed,
            event_context.rules_version,
            super::DecisionDomain::CultureEvent,
            event_context.year,
            subject,
            ordinal,
        ));
        if events.iter().all(|event| event.id != candidate) {
            break candidate;
        }
        ordinal = ordinal.wrapping_add(1);
    };
    events.push(HistoricalEvent {
        id,
        year: event_context.year,
        order,
        affected_entities,
        kind,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chargen_domain::{
        AffiliationPolicy, CultureState, Genome, InstitutionDefinition, InstitutionKind,
        InstitutionScope, LogicalId, Person, ReproductiveProfile, Settlement, WorldId,
    };

    fn key() -> TraitKey {
        TraitKey::new("culture", "trust")
    }

    fn config() -> CultureConfig {
        CultureConfig {
            dimensions: BTreeMap::from([(
                key(),
                CultureDimensionPolicy {
                    parent_weight: 2,
                    local_weight: 1,
                    variation_max: 0,
                    minimum: 0,
                    maximum: 100,
                    founder_value: Some(50),
                    ..CultureDimensionPolicy::default()
                },
            )]),
            ..CultureConfig::default()
        }
    }

    fn person(id: u64, settlement_id: SettlementId, birth_year: i32, value: i16) -> Person {
        Person {
            id: PersonId(id),
            logical_id: LogicalId(1000 + id),
            birth_year: Year(birth_year),
            death_year: None,
            settlement_id,
            household_id: None,
            reproductive: ReproductiveProfile::default(),
            traits: BTreeMap::from([(key(), value)]),
            genome: Genome::default(),
            last_widowed_year: None,
            profession: None,
        }
    }

    fn state(people: Vec<Person>, settlement_ids: &[SettlementId]) -> State {
        let persons: BTreeMap<_, _> = people
            .into_iter()
            .map(|person| (person.id, person))
            .collect();
        let settlements = settlement_ids
            .iter()
            .copied()
            .map(|id| {
                (
                    id,
                    Settlement {
                        id,
                        logical_id: LogicalId(10_000 + id.0),
                        name: format!("Settlement {}", id.0),
                        culture_values: BTreeMap::new(),
                    },
                )
            })
            .collect();
        State {
            world_id: WorldId(7),
            rules_version: 2,
            seed: 55,
            start_year: Year(0),
            current_year: Some(Year(0)),
            next_person_id: persons.keys().map(|id| id.0).max().unwrap_or(0) + 1,
            next_household_id: 1,
            next_union_id: 1,
            next_family_link_id: 1,
            settlements,
            persons,
            households: BTreeMap::new(),
            unions: BTreeMap::new(),
            family_links: BTreeMap::new(),
            last_gestational_birth_year: BTreeMap::new(),
            residences: BTreeMap::new(),
            profession_intervals: BTreeMap::new(),
            last_year_settlement_balances: BTreeMap::new(),
            culture: CultureState::default(),
            economy: chargen_domain::EconomyState::default(),
            event_effects: Vec::new(),
            last_year_phases: Vec::new(),
        }
    }

    #[test]
    fn similarity_is_bounded_and_exact_values_match() {
        let policy = CultureDimensionPolicy {
            minimum: -10,
            maximum: 10,
            ..CultureDimensionPolicy::default()
        };
        assert_eq!(similarity_ppm(4, 4, &policy), 1_000_000);
        assert_eq!(similarity_ppm(-10, 10, &policy), 0);
        assert!(similarity_ppm(0, 1, &policy) < 1_000_000);
    }

    #[test]
    fn birth_base_uses_configured_weights_and_explicit_founder_value() {
        let policy = CultureDimensionPolicy {
            parent_weight: 3,
            local_weight: 1,
            founder_value: Some(25),
            minimum: 0,
            maximum: 100,
            ..CultureDimensionPolicy::default()
        };
        assert_eq!(
            weighted_cultural_base(Some(80), Some(40), &policy).unwrap(),
            70
        );
        assert_eq!(weighted_cultural_base(None, None, &policy).unwrap(), 25);
    }

    #[test]
    fn incremental_settlement_aggregates_match_a_full_recalculation() {
        let first = SettlementId(1);
        let second = SettlementId(2);
        let mut state = state(
            vec![person(1, first, -25, 20), person(2, first, -24, 40)],
            &[first, second],
        );
        let config = config();
        initialize_culture(&config, &mut state, Year(0)).unwrap();

        state.persons.get_mut(&PersonId(2)).unwrap().settlement_id = second;
        apply_population_delta(
            &config,
            &mut state,
            Year(0),
            CulturePopulationDelta {
                person_id: PersonId(2),
                from: Some(first),
                to: Some(second),
            },
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(state.persons[&PersonId(2)].traits[&key()], 40);
        validate_culture_state(&config, &state, Year(0)).unwrap();

        state.persons.get_mut(&PersonId(2)).unwrap().death_year = Some(Year(1));
        apply_population_delta(
            &config,
            &mut state,
            Year(1),
            CulturePopulationDelta {
                person_id: PersonId(2),
                from: Some(second),
                to: None,
            },
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(state.persons[&PersonId(2)].traits[&key()], 40);
        validate_culture_state(&config, &state, Year(1)).unwrap();

        let mut recalculated = state.clone();
        rebuild_culture_aggregates(&config, &mut recalculated, Year(1)).unwrap();
        assert_eq!(state.culture.settlements, recalculated.culture.settlements);
        assert_eq!(state.culture.settlements[&second][&key()].value(), None);
    }

    #[test]
    fn newborns_share_one_prebirth_local_snapshot_and_parent_mix_uses_floor() {
        let settlement = SettlementId(1);
        let parents = vec![
            person(1, settlement, -25, 20),
            person(2, settlement, -25, 40),
            person(3, settlement, -25, 80),
            person(4, settlement, -25, 100),
        ];
        let mut state = state(parents, &[settlement]);
        let config = config();
        initialize_culture(&config, &mut state, Year(0)).unwrap();
        assert_eq!(
            state.culture.settlements[&settlement][&key()].value(),
            Some(60)
        );
        let first = CultureBirthInput {
            person_id: PersonId(5),
            logical_id: LogicalId(5005),
            settlement_id: settlement,
            genetic_parents: vec![PersonId(1), PersonId(2)],
        };
        let second = CultureBirthInput {
            person_id: PersonId(6),
            logical_id: LogicalId(5006),
            settlement_id: settlement,
            genetic_parents: vec![PersonId(4), PersonId(2)],
        };
        let forward = prepare_birth_culture_batch(
            &config,
            &state,
            Year(0),
            &[first.clone(), second.clone()],
            |_, _, _| 0,
        )
        .unwrap();
        let reversed =
            prepare_birth_culture_batch(&config, &state, Year(0), &[second, first], |_, _, _| 0)
                .unwrap();
        let by_id: BTreeMap<_, _> = forward
            .iter()
            .map(|assignment| (assignment.person_id, assignment.values[&key()]))
            .collect();
        let reverse_by_id: BTreeMap<_, _> = reversed
            .iter()
            .map(|assignment| (assignment.person_id, assignment.values[&key()]))
            .collect();
        assert_eq!(by_id, reverse_by_id);
        assert_eq!(by_id[&PersonId(5)], 40);
        assert_eq!(by_id[&PersonId(6)], 66);

        for id in [PersonId(5), PersonId(6)] {
            state.persons.insert(id, person(id.0, settlement, 0, 0));
        }
        apply_birth_culture_batch(&config, &mut state, Year(0), &forward).unwrap();
        assert_eq!(state.persons[&PersonId(5)].traits[&key()], 40);
        assert_eq!(state.persons[&PersonId(6)].traits[&key()], 66);
        assert_eq!(
            state.culture.settlements[&settlement][&key()].value(),
            Some(58)
        );
        validate_culture_state(&config, &state, Year(0)).unwrap();
    }

    #[test]
    fn empty_settlements_have_none_culture_values() {
        let settlement = SettlementId(9);
        let mut state = state(Vec::new(), &[settlement]);
        let config = config();
        initialize_culture(&config, &mut state, Year(0)).unwrap();
        let aggregate = state.culture.settlements[&settlement][&key()];
        assert_eq!(aggregate.member_count, 0);
        assert_eq!(aggregate.value(), None);
    }

    #[test]
    fn age_eligibility_updates_only_aggregate_values_at_boundary() {
        let settlement = SettlementId(1);
        let mut state = state(
            vec![
                person(1, settlement, -17, 20),
                person(2, settlement, -25, 40),
            ],
            &[settlement],
        );
        let mut config = config();
        config.dimensions.get_mut(&key()).unwrap().eligibility =
            CultureEligibility::MinimumAge { minimum_age: 18 };
        initialize_culture(&config, &mut state, Year(0)).unwrap();
        assert_eq!(
            state.culture.settlements[&settlement][&key()].value(),
            Some(40)
        );

        apply_culture_age_transitions(&config, &mut state, Year(0), Year(1)).unwrap();
        assert_eq!(
            state.culture.settlements[&settlement][&key()].value(),
            Some(30)
        );
        assert_eq!(state.persons[&PersonId(1)].traits[&key()], 20);
        assert_eq!(state.persons[&PersonId(2)].traits[&key()], 40);
        validate_culture_state(&config, &state, Year(1)).unwrap();
    }

    #[test]
    fn affiliation_and_migration_leave_adult_values_frozen() {
        let first = SettlementId(1);
        let second = SettlementId(2);
        let mut state = state(vec![person(1, first, -25, 40)], &[first, second]);
        let mut config = config();
        config.institutions.push(InstitutionDefinition {
            id: InstitutionId::from("council"),
            name: "Council".into(),
            kind: InstitutionKind::Institution,
            scope: InstitutionScope::Settlement,
            allowed_settlements: Some([first].into_iter().collect()),
            values: BTreeMap::from([(key(), 40)]),
            affiliation: AffiliationPolicy {
                enabled: true,
                minimum_age: 18,
                maximum_age: None,
                minimum_affinity_ppm: 1_000_000,
                opportunity_ppm: 1_000_000,
                candidate_limit: 1,
                maximum_affiliations_per_person: Some(1),
                continue_after_migration: false,
            },
        });
        initialize_culture(&config, &mut state, Year(0)).unwrap();
        let mut events = Vec::new();
        apply_a10_culture(&config, &mut state, Year(0), 55, 2, &mut events).unwrap();
        assert_eq!(state.culture.institution_memberships.len(), 1);
        assert_eq!(state.persons[&PersonId(1)].traits[&key()], 40);

        state.persons.get_mut(&PersonId(1)).unwrap().settlement_id = second;
        apply_population_delta(
            &config,
            &mut state,
            Year(1),
            CulturePopulationDelta {
                person_id: PersonId(1),
                from: Some(first),
                to: Some(second),
            },
            &mut events,
        )
        .unwrap();
        assert_eq!(state.persons[&PersonId(1)].traits[&key()], 40);
        assert_eq!(
            state.culture.institution_memberships[0]
                .validity
                .end_exclusive,
            Some(Year(1))
        );
        assert!(events.iter().any(|event| matches!(
            &event.kind,
            HistoricalEventKind::InstitutionAffiliationEnded {
                institution_id,
                person_id: PersonId(1),
                reason: InstitutionAffiliationEndReason::LeftOnMigration,
                ..
            } if institution_id == &InstitutionId::from("council")
        )));
        validate_culture_state(&config, &state, Year(1)).unwrap();
    }

    #[test]
    fn branch_policy_reconciliation_keeps_adults_and_membership_history() {
        let settlement = SettlementId(1);
        let mut state = state(
            vec![
                person(1, settlement, -25, 20),
                person(2, settlement, -24, 40),
                person(3, settlement, -23, 80),
            ],
            &[settlement],
        );
        let mut original_config = config();
        original_config.organizations = chargen_domain::OrganizationPolicy {
            enabled: true,
            cadence_years: 1,
            candidate_limit_per_settlement: 3,
            maximum_organizations_per_settlement: 1,
            minimum_founders: 2,
            maximum_members: 3,
            dissolve_below_members: 1,
            minimum_age: 18,
            maximum_age: None,
            minimum_affinity_ppm: 0,
            formation_opportunity_ppm: 1_000_000,
            continue_after_migration: true,
        };
        original_config.institutions.push(InstitutionDefinition {
            id: InstitutionId::from("archive"),
            name: "Archive".into(),
            kind: InstitutionKind::Institution,
            scope: InstitutionScope::World,
            allowed_settlements: None,
            values: BTreeMap::from([(key(), 50)]),
            affiliation: AffiliationPolicy {
                enabled: true,
                minimum_age: 18,
                maximum_age: None,
                minimum_affinity_ppm: 0,
                opportunity_ppm: 1_000_000,
                candidate_limit: 3,
                maximum_affiliations_per_person: Some(1),
                continue_after_migration: true,
            },
        });
        initialize_culture(&original_config, &mut state, Year(0)).unwrap();
        apply_a10_culture(
            &original_config,
            &mut state,
            Year(0),
            55,
            2,
            &mut Vec::new(),
        )
        .unwrap();
        let organization_id = *state.culture.organizations.keys().next().unwrap();
        let old_memberships = state.culture.organizations[&organization_id]
            .memberships
            .clone();
        let old_institution_memberships = state.culture.institution_memberships.clone();

        let new_key = TraitKey::new("culture", "curiosity");
        let mut revised_config = original_config.clone();
        revised_config.dimensions.insert(
            new_key.clone(),
            CultureDimensionPolicy {
                founder_value: Some(70),
                ..CultureDimensionPolicy::default()
            },
        );
        reconcile_culture_config(&revised_config, &mut state, Year(0)).unwrap();
        assert_eq!(state.persons[&PersonId(1)].traits[&key()], 20);
        assert_eq!(state.persons[&PersonId(1)].traits[&new_key], 70);
        assert_eq!(
            state.culture.organizations[&organization_id].memberships,
            old_memberships
        );
        assert_eq!(
            state.culture.institution_memberships,
            old_institution_memberships
        );
        validate_culture_state(&revised_config, &state, Year(0)).unwrap();

        revised_config.dimensions.remove(&key());
        for institution in &mut revised_config.institutions {
            institution.values.remove(&key());
        }
        reconcile_culture_config(&revised_config, &mut state, Year(0)).unwrap();
        assert_eq!(state.persons[&PersonId(1)].traits[&key()], 20);
        assert_eq!(
            state.culture.settlements[&settlement]
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            [new_key.clone()].into_iter().collect()
        );
        assert_eq!(
            state.culture.organizations[&organization_id]
                .culture_values
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            [new_key].into_iter().collect()
        );
        validate_culture_state(&revised_config, &state, Year(0)).unwrap();
    }

    #[test]
    fn organization_limits_founder_and_dissolution_events_are_recorded() {
        let settlement = SettlementId(1);
        let destination = SettlementId(2);
        let mut state = state(
            vec![
                person(1, settlement, -25, 50),
                person(2, settlement, -24, 50),
                person(3, settlement, -23, 50),
                person(4, settlement, -3, 50),
            ],
            &[settlement, destination],
        );
        let mut config = config();
        config.organizations = chargen_domain::OrganizationPolicy {
            enabled: true,
            cadence_years: 1,
            candidate_limit_per_settlement: 3,
            maximum_organizations_per_settlement: 1,
            minimum_founders: 2,
            maximum_members: 3,
            dissolve_below_members: 2,
            minimum_age: 18,
            maximum_age: None,
            minimum_affinity_ppm: 0,
            formation_opportunity_ppm: 1_000_000,
            continue_after_migration: false,
        };
        initialize_culture(&config, &mut state, Year(0)).unwrap();
        let mut events = Vec::new();
        apply_a10_culture(&config, &mut state, Year(0), 55, 2, &mut events).unwrap();
        assert_eq!(state.culture.organizations.len(), 1);
        let organization_id = *state.culture.organizations.keys().next().unwrap();
        let organization = &state.culture.organizations[&organization_id];
        assert_eq!(organization.current_members.len(), 3);
        assert_eq!(organization.founders, organization.current_members);
        assert!(!organization.current_members.contains(&PersonId(4)));
        assert!(events.iter().any(|event| matches!(
            &event.kind,
            HistoricalEventKind::OrganizationFounded { organization_id: found, .. }
                if *found == organization_id
        )));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    &event.kind,
                    HistoricalEventKind::OrganizationMembershipStarted { founder: true, .. }
                ))
                .count(),
            3
        );
        validate_culture_state(&config, &state, Year(0)).unwrap();
        apply_a10_culture(&config, &mut state, Year(0), 55, 2, &mut events).unwrap();
        assert_eq!(state.culture.organizations.len(), 1);

        let migrant = *state.culture.organizations[&organization_id]
            .current_members
            .iter()
            .next()
            .unwrap();
        state.persons.get_mut(&migrant).unwrap().settlement_id = destination;
        let mut next_events = Vec::new();
        apply_population_delta(
            &config,
            &mut state,
            Year(1),
            CulturePopulationDelta {
                person_id: migrant,
                from: Some(settlement),
                to: Some(destination),
            },
            &mut next_events,
        )
        .unwrap();
        assert_eq!(state.persons[&migrant].traits[&key()], 50);
        assert!(next_events.iter().any(|event| matches!(
            &event.kind,
            HistoricalEventKind::OrganizationMembershipEnded {
                organization_id: ended,
                person_id,
                reason: OrganizationMembershipEndReason::LeftOnMigration,
            } if *ended == organization_id && *person_id == migrant
        )));

        let member_to_die = state.culture.organizations[&organization_id]
            .current_members
            .iter()
            .copied()
            .next()
            .unwrap();
        state.persons.get_mut(&member_to_die).unwrap().death_year = Some(Year(1));
        apply_population_delta(
            &config,
            &mut state,
            Year(1),
            CulturePopulationDelta {
                person_id: member_to_die,
                from: Some(settlement),
                to: None,
            },
            &mut next_events,
        )
        .unwrap();
        apply_a10_culture(&config, &mut state, Year(1), 55, 2, &mut next_events).unwrap();
        assert_eq!(
            state.culture.organizations[&organization_id].dissolved_year,
            Some(Year(1))
        );
        assert!(next_events.iter().any(|event| matches!(
            &event.kind,
            HistoricalEventKind::OrganizationDissolved {
                organization_id: dissolved,
                ..
            } if *dissolved == organization_id
        )));
        validate_culture_state(&config, &state, Year(1)).unwrap();
    }

    #[test]
    fn bounded_candidate_sampler_uses_stable_top_k() {
        let settlement = SettlementId(1);
        let people = (1..=20).map(|id| person(id, settlement, -20, 50)).collect();
        let state = state(people, &[settlement]);
        let rank = |person: &Person| (person.logical_id.0.wrapping_mul(17)) % 101;
        let mut expected: Vec<_> = state
            .persons
            .values()
            .map(|person| (rank(person), person.logical_id, person.id))
            .collect();
        expected.sort_unstable();
        expected.truncate(4);
        let actual = sample_people(&state, state.persons.keys().copied(), 4, rank);
        assert_eq!(
            actual,
            expected
                .into_iter()
                .map(|(_, _, id)| id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn zero_length_affiliations_do_not_overlap_adjacent_intervals() {
        let intervals = BTreeMap::from([(
            PersonId(1),
            vec![
                YearInterval {
                    start: Year(2),
                    end_exclusive: Some(Year(2)),
                },
                YearInterval {
                    start: Year(2),
                    end_exclusive: None,
                },
            ],
        )]);
        assert!(validate_non_overlapping_intervals(intervals).is_ok());
        let overlapping = BTreeMap::from([(
            PersonId(1),
            vec![
                YearInterval {
                    start: Year(1),
                    end_exclusive: Some(Year(3)),
                },
                YearInterval {
                    start: Year(2),
                    end_exclusive: None,
                },
            ],
        )]);
        assert!(validate_non_overlapping_intervals(overlapping).is_err());
    }
}
