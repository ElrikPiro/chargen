//! Versioned culture, preconfigured affiliations, and organization state.
//!
//! Cultural values belong to people and are assigned at initialization or
//! birth. Aggregates are projections of the current eligible members; moving or
//! affiliating a person never edits their personal values.

use crate::{PersonId, SettlementId, TraitKey, ValidationIssue, Year, YearInterval};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Optional ruleset configuration. `None` keeps ruleset 1 behavior unchanged.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CultureConfig {
    pub dimensions: BTreeMap<TraitKey, CultureDimensionPolicy>,
    #[serde(default)]
    pub institutions: Vec<InstitutionDefinition>,
    #[serde(default)]
    pub organizations: OrganizationPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CultureDimensionPolicy {
    /// A person contributes only when this predicate holds at the year's close.
    pub eligibility: CultureEligibility,
    /// Population aggregate weight per eligible person; must be positive.
    pub member_weight: u32,
    /// Relative weights for known parent values and the pre-birth local snapshot.
    pub parent_weight: u32,
    pub local_weight: u32,
    /// Maximum absolute integer variation applied to a newborn.
    pub variation_max: u16,
    pub minimum: i16,
    pub maximum: i16,
    /// Used when neither a parent nor an inhabited local aggregate is available.
    pub founder_value: Option<i16>,
}

impl Default for CultureDimensionPolicy {
    fn default() -> Self {
        Self {
            eligibility: CultureEligibility::AllLiving,
            member_weight: 1,
            parent_weight: 1,
            local_weight: 1,
            variation_max: 0,
            minimum: 0,
            maximum: 100,
            founder_value: Some(50),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CultureEligibility {
    AllLiving,
    MinimumAge { minimum_age: u16 },
    AgeRange { minimum_age: u16, maximum_age: u16 },
}

impl CultureEligibility {
    pub fn includes_age(self, age: u32) -> bool {
        match self {
            Self::AllLiving => true,
            Self::MinimumAge { minimum_age } => age >= u32::from(minimum_age),
            Self::AgeRange {
                minimum_age,
                maximum_age,
            } => (u32::from(minimum_age)..=u32::from(maximum_age)).contains(&age),
        }
    }
}

/// Stable, namespaced identifier for a preconfigured institution or religion.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstitutionId(pub String);

impl From<&str> for InstitutionId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstitutionKind {
    Institution,
    Religion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstitutionScope {
    World,
    Settlement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstitutionDefinition {
    pub id: InstitutionId,
    pub name: String,
    pub kind: InstitutionKind,
    pub scope: InstitutionScope,
    /// `None` permits recruitment everywhere; a set restricts presence to these places.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_settlements: Option<BTreeSet<SettlementId>>,
    /// Reference values used for cultural affinity; they are not personal values.
    pub values: BTreeMap<TraitKey, i16>,
    #[serde(default)]
    pub affiliation: AffiliationPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AffiliationPolicy {
    pub enabled: bool,
    pub minimum_age: u16,
    pub maximum_age: Option<u16>,
    /// Minimum normalized cultural affinity, in parts per million. Per-dimension
    /// similarity is linear distance within the configured range, weighted by
    /// that dimension's `member_weight`.
    pub minimum_affinity_ppm: u32,
    /// Independent per-year opportunity, in parts per million.
    pub opportunity_ppm: u32,
    /// Number of logically ordered candidates considered per institution and settlement.
    pub candidate_limit: u16,
    pub maximum_affiliations_per_person: Option<u16>,
    /// Whether a member remains affiliated after moving to another settlement.
    pub continue_after_migration: bool,
}

impl Default for AffiliationPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            minimum_age: 0,
            maximum_age: None,
            minimum_affinity_ppm: 0,
            opportunity_ppm: 0,
            candidate_limit: 64,
            maximum_affiliations_per_person: None,
            continue_after_migration: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OrganizationPolicy {
    pub enabled: bool,
    pub cadence_years: u16,
    pub candidate_limit_per_settlement: u16,
    pub maximum_organizations_per_settlement: u16,
    pub minimum_founders: u16,
    pub maximum_members: u16,
    pub dissolve_below_members: u16,
    pub minimum_age: u16,
    pub maximum_age: Option<u16>,
    /// Minimum founder/member similarity in parts per million using the same
    /// configured-range and `member_weight` formula as institution affinity.
    pub minimum_affinity_ppm: u32,
    pub formation_opportunity_ppm: u32,
    /// If false, membership ends on emigration; if true, organizations travel with members.
    pub continue_after_migration: bool,
}

impl Default for OrganizationPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            cadence_years: 5,
            candidate_limit_per_settlement: 256,
            maximum_organizations_per_settlement: 16,
            minimum_founders: 3,
            maximum_members: 500,
            dissolve_below_members: 1,
            minimum_age: 18,
            maximum_age: None,
            minimum_affinity_ppm: 700_000,
            formation_opportunity_ppm: 0,
            continue_after_migration: true,
        }
    }
}

/// Incremental aggregate. It intentionally stores no sentinel for an empty group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CultureAggregate {
    pub weighted_sum: i64,
    pub total_weight: u64,
    pub member_count: u64,
}

impl CultureAggregate {
    pub fn value(self) -> Option<i16> {
        if self.total_weight == 0 {
            return None;
        }
        let rounded = div_round_nearest_away_from_zero(
            i128::from(self.weighted_sum),
            i128::from(self.total_weight),
        );
        Some(rounded.clamp(i128::from(i16::MIN), i128::from(i16::MAX)) as i16)
    }

    pub fn add(&mut self, value: i16, weight: u32) -> Result<(), CultureStateError> {
        if weight == 0 {
            return Ok(());
        }
        let weighted = i64::from(value)
            .checked_mul(i64::from(weight))
            .ok_or(CultureStateError::Overflow)?;
        let weighted_sum = self
            .weighted_sum
            .checked_add(weighted)
            .ok_or(CultureStateError::Overflow)?;
        let total_weight = self
            .total_weight
            .checked_add(u64::from(weight))
            .ok_or(CultureStateError::Overflow)?;
        let member_count = self
            .member_count
            .checked_add(1)
            .ok_or(CultureStateError::Overflow)?;
        self.weighted_sum = weighted_sum;
        self.total_weight = total_weight;
        self.member_count = member_count;
        Ok(())
    }

    pub fn remove(&mut self, value: i16, weight: u32) -> Result<(), CultureStateError> {
        if weight == 0 {
            return Ok(());
        }
        let weighted = i64::from(value)
            .checked_mul(i64::from(weight))
            .ok_or(CultureStateError::Overflow)?;
        let weighted_sum = self
            .weighted_sum
            .checked_sub(weighted)
            .ok_or(CultureStateError::Overflow)?;
        let total_weight = self
            .total_weight
            .checked_sub(u64::from(weight))
            .ok_or(CultureStateError::Underflow)?;
        let member_count = self
            .member_count
            .checked_sub(1)
            .ok_or(CultureStateError::Underflow)?;
        if total_weight == 0 && (weighted_sum != 0 || member_count != 0) {
            return Err(CultureStateError::InconsistentAggregate);
        }
        self.weighted_sum = weighted_sum;
        self.total_weight = total_weight;
        self.member_count = member_count;
        Ok(())
    }
}

fn div_round_nearest_away_from_zero(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    if numerator < 0 {
        -((-numerator + denominator / 2) / denominator)
    } else {
        (numerator + denominator / 2) / denominator
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CultureState {
    pub settlements: BTreeMap<SettlementId, BTreeMap<TraitKey, CultureAggregate>>,
    pub institution_memberships: Vec<InstitutionMembership>,
    /// Active memberships only, indexed for year-to-year A10 work and person exits.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub active_institution_membership_indices_by_person:
        BTreeMap<PersonId, BTreeMap<InstitutionId, u64>>,
    pub organizations: BTreeMap<OrganizationId, Organization>,
    /// Reverse membership index for deaths/moves; bounded by actual affiliations.
    pub organization_ids_by_person: BTreeMap<PersonId, BTreeSet<OrganizationId>>,
    pub next_organization_id: u64,
    pub organization_cadence_counters: BTreeMap<SettlementId, u32>,
}

impl CultureState {
    pub fn is_empty(&self) -> bool {
        self.settlements.is_empty()
            && self.institution_memberships.is_empty()
            && self
                .active_institution_membership_indices_by_person
                .is_empty()
            && self.organizations.is_empty()
            && self.organization_ids_by_person.is_empty()
            && self.next_organization_id == 0
            && self.organization_cadence_counters.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstitutionMembership {
    pub institution_id: InstitutionId,
    pub person_id: PersonId,
    pub settlement_id: SettlementId,
    pub validity: YearInterval,
}

impl InstitutionMembership {
    pub fn active_at_close(&self, year: Year) -> bool {
        self.validity.contains(year)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstitutionAffiliationEndReason {
    EndpointDeath,
    LeftOnMigration,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct OrganizationId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Organization {
    pub id: OrganizationId,
    pub logical_id: crate::LogicalId,
    pub name: String,
    pub settlement_id: SettlementId,
    pub founded_year: Year,
    /// First year at whose close the organization is inactive.
    pub dissolved_year: Option<Year>,
    pub founders: BTreeSet<PersonId>,
    /// Append-only historical affiliation intervals; multiple intervals permit rejoining.
    pub memberships: Vec<OrganizationMembership>,
    /// Index of members active at the current state head for incremental updates.
    pub current_members: BTreeSet<PersonId>,
    pub culture_values: BTreeMap<TraitKey, CultureAggregate>,
}

impl Organization {
    pub fn active_at_close(&self, year: Year) -> bool {
        self.founded_year <= year && self.dissolved_year.is_none_or(|end| year < end)
    }

    pub fn members_at_close(&self, year: Year) -> impl Iterator<Item = PersonId> + '_ {
        self.memberships
            .iter()
            .filter(move |membership| membership.validity.contains(year))
            .map(|membership| membership.person_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganizationMembership {
    pub person_id: PersonId,
    pub validity: YearInterval,
    pub founder: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationMembershipEndReason {
    EndpointDeath,
    LeftOnMigration,
    OrganizationDissolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationDissolutionReason {
    BelowMemberThreshold,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CultureStateError {
    Overflow,
    Underflow,
    InconsistentAggregate,
    MissingPerson(PersonId),
    MissingTrait { person_id: PersonId, key: TraitKey },
    InvalidConfiguration(String),
    InvalidState(String),
}

impl fmt::Display for CultureStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => f.write_str("cultural aggregate arithmetic overflow"),
            Self::Underflow => f.write_str("cultural aggregate arithmetic underflow"),
            Self::InconsistentAggregate => {
                f.write_str("empty cultural aggregate has residual values")
            }
            Self::MissingPerson(id) => {
                write!(f, "cultural operation references missing person {}", id.0)
            }
            Self::MissingTrait { person_id, key } => write!(
                f,
                "person {} is missing cultural trait {}::{}",
                person_id.0, key.namespace, key.name
            ),
            Self::InvalidConfiguration(message) => {
                write!(f, "invalid culture configuration: {message}")
            }
            Self::InvalidState(message) => write!(f, "invalid culture state: {message}"),
        }
    }
}

impl std::error::Error for CultureStateError {}

/// Validate config references/ranges against the scenario's qualified trait catalog.
pub fn validate_culture_config(
    config: &CultureConfig,
    trait_catalog: &BTreeMap<TraitKey, (i16, i16)>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    for (key, policy) in &config.dimensions {
        let path = format!("culture.dimensions.{}::{}", key.namespace, key.name);
        if policy.minimum > policy.maximum {
            issues.push(ValidationIssue::new(
                format!("{path}.minimum"),
                "invalid_trait_range",
                "minimum must not exceed maximum",
            ));
        }
        if policy.member_weight == 0 {
            issues.push(ValidationIssue::new(
                format!("{path}.member_weight"),
                "zero_member_weight",
                "member weight must be positive",
            ));
        }
        if policy.variation_max > i16::MAX as u16 {
            issues.push(ValidationIssue::new(
                format!("{path}.variation_max"),
                "variation_out_of_range",
                "variation maximum must fit a signed 16-bit trait offset",
            ));
        }
        if policy.parent_weight == 0 && policy.local_weight == 0 && policy.founder_value.is_none() {
            issues.push(ValidationIssue::new(
                &path,
                "missing_birth_source",
                "a dimension needs a parent/local source or an explicit founder value",
            ));
        }
        if let Some(value) = policy.founder_value {
            if !(policy.minimum..=policy.maximum).contains(&value) {
                issues.push(ValidationIssue::new(
                    format!("{path}.founder_value"),
                    "trait_value_out_of_range",
                    "founder value must be within the cultural range",
                ));
            }
        }
        match policy.eligibility {
            CultureEligibility::AgeRange {
                minimum_age,
                maximum_age,
            } if minimum_age > maximum_age => issues.push(ValidationIssue::new(
                format!("{path}.eligibility"),
                "invalid_age_range",
                "minimum age must not exceed maximum age",
            )),
            _ => {}
        }
        match trait_catalog.get(key) {
            None => issues.push(ValidationIssue::new(
                &path,
                "unknown_trait",
                "cultural dimension must reference a declared qualified trait",
            )),
            Some((catalog_minimum, catalog_maximum))
                if policy.minimum < *catalog_minimum || policy.maximum > *catalog_maximum =>
            {
                issues.push(ValidationIssue::new(
                    &path,
                    "trait_range_mismatch",
                    "cultural range must fit the declared trait range",
                ));
            }
            Some(_) => {}
        }
    }

    let mut ids = BTreeSet::new();
    for (index, institution) in config.institutions.iter().enumerate() {
        let path = format!("culture.institutions[{index}]");
        if institution.id.0.trim().is_empty() {
            issues.push(ValidationIssue::new(
                format!("{path}.id"),
                "empty_id",
                "institution ID must not be empty",
            ));
        }
        if !ids.insert(institution.id.clone()) {
            issues.push(ValidationIssue::new(
                format!("{path}.id"),
                "duplicate_id",
                "institution and religion IDs must be unique",
            ));
        }
        if institution.name.trim().is_empty() {
            issues.push(ValidationIssue::new(
                format!("{path}.name"),
                "empty_name",
                "institution name must not be empty",
            ));
        }
        validate_probability(
            institution.affiliation.opportunity_ppm,
            &format!("{path}.affiliation.opportunity_ppm"),
            &mut issues,
        );
        validate_probability(
            institution.affiliation.minimum_affinity_ppm,
            &format!("{path}.affiliation.minimum_affinity_ppm"),
            &mut issues,
        );
        if institution.affiliation.candidate_limit == 0 {
            issues.push(ValidationIssue::new(
                format!("{path}.affiliation.candidate_limit"),
                "zero_candidate_limit",
                "candidate limit must be positive",
            ));
        }
        if institution
            .affiliation
            .maximum_age
            .is_some_and(|maximum| maximum < institution.affiliation.minimum_age)
        {
            issues.push(ValidationIssue::new(
                format!("{path}.affiliation.maximum_age"),
                "invalid_age_range",
                "maximum age must not precede minimum age",
            ));
        }
        for (key, value) in &institution.values {
            match (config.dimensions.get(key), trait_catalog.get(key)) {
                (None, _) => issues.push(ValidationIssue::new(
                    format!("{path}.values.{}::{}", key.namespace, key.name),
                    "unknown_cultural_dimension",
                    "institution values must reference a configured cultural dimension",
                )),
                (Some(policy), _) if !(policy.minimum..=policy.maximum).contains(value) => {
                    issues.push(ValidationIssue::new(
                        format!("{path}.values.{}::{}", key.namespace, key.name),
                        "trait_value_out_of_range",
                        "institution value is outside the cultural dimension range",
                    ));
                }
                _ => {}
            }
        }
    }

    let organization = &config.organizations;
    validate_probability(
        organization.minimum_affinity_ppm,
        "culture.organizations.minimum_affinity_ppm",
        &mut issues,
    );
    validate_probability(
        organization.formation_opportunity_ppm,
        "culture.organizations.formation_opportunity_ppm",
        &mut issues,
    );
    if organization.enabled {
        for (path, invalid) in [
            ("cadence_years", organization.cadence_years == 0),
            (
                "candidate_limit_per_settlement",
                organization.candidate_limit_per_settlement == 0,
            ),
            (
                "maximum_organizations_per_settlement",
                organization.maximum_organizations_per_settlement == 0,
            ),
            ("minimum_founders", organization.minimum_founders == 0),
            (
                "maximum_members",
                organization.maximum_members < organization.minimum_founders,
            ),
            (
                "dissolve_below_members",
                organization.dissolve_below_members > organization.maximum_members,
            ),
        ] {
            if invalid {
                issues.push(ValidationIssue::new(
                    format!("culture.organizations.{path}"),
                    "invalid_limit",
                    "organization limits must be positive and internally consistent",
                ));
            }
        }
        if organization
            .maximum_age
            .is_some_and(|maximum| maximum < organization.minimum_age)
        {
            issues.push(ValidationIssue::new(
                "culture.organizations.maximum_age",
                "invalid_age_range",
                "maximum age must not precede minimum age",
            ));
        }
    }
    issues
}

/// Check preconfigured territorial presence against the scenario's settlements.
pub fn validate_culture_settlement_references(
    config: &CultureConfig,
    settlement_ids: &BTreeSet<SettlementId>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    for (index, institution) in config.institutions.iter().enumerate() {
        if let Some(allowed) = &institution.allowed_settlements {
            for settlement_id in allowed {
                if !settlement_ids.contains(settlement_id) {
                    issues.push(ValidationIssue::new(
                        format!("culture.institutions[{index}].allowed_settlements"),
                        "unknown_reference",
                        format!(
                            "settlement {} is not defined by this scenario",
                            settlement_id.0
                        ),
                    ));
                }
            }
        }
    }
    issues
}

fn validate_probability(value: u32, path: &str, issues: &mut Vec<ValidationIssue>) {
    if value > 1_000_000 {
        issues.push(ValidationIssue::new(
            path,
            "probability_out_of_range",
            "probability must be between 0 and 1,000,000 ppm",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_aggregate_rounds_signed_means_and_empty_is_absent() {
        let mut aggregate = CultureAggregate::default();
        assert_eq!(aggregate.value(), None);
        aggregate.add(-2, 1).unwrap();
        aggregate.add(-1, 1).unwrap();
        assert_eq!(aggregate.value(), Some(-2));
        aggregate.remove(-2, 1).unwrap();
        aggregate.remove(-1, 1).unwrap();
        assert_eq!(aggregate, CultureAggregate::default());
        assert_eq!(aggregate.value(), None);
    }

    #[test]
    fn culture_config_rejects_bad_dimension_and_duplicate_institution_ids() {
        let key = TraitKey::new("culture", "trust");
        let config = CultureConfig {
            dimensions: BTreeMap::from([(
                key.clone(),
                CultureDimensionPolicy {
                    minimum: 10,
                    maximum: 0,
                    founder_value: Some(20),
                    ..CultureDimensionPolicy::default()
                },
            )]),
            institutions: vec![
                InstitutionDefinition {
                    id: InstitutionId::from("council"),
                    name: "Council".into(),
                    kind: InstitutionKind::Institution,
                    scope: InstitutionScope::World,
                    allowed_settlements: None,
                    values: BTreeMap::new(),
                    affiliation: AffiliationPolicy::default(),
                },
                InstitutionDefinition {
                    id: InstitutionId::from("council"),
                    name: "Council again".into(),
                    kind: InstitutionKind::Religion,
                    scope: InstitutionScope::Settlement,
                    allowed_settlements: None,
                    values: BTreeMap::new(),
                    affiliation: AffiliationPolicy::default(),
                },
            ],
            ..CultureConfig::default()
        };
        let catalog = BTreeMap::from([(key, (0, 100))]);
        let issues = validate_culture_config(&config, &catalog);
        assert!(issues
            .iter()
            .any(|issue| issue.code == "invalid_trait_range"));
        assert!(issues.iter().any(|issue| issue.code == "duplicate_id"));
    }
}
