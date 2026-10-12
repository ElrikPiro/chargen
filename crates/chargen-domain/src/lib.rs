use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub mod culture;
pub mod economy;
pub mod events;
pub mod territory;

pub use culture::*;
pub use economy::*;
pub use events::*;
pub use territory::*;

macro_rules! integer_id {
    ($name:ident, $inner:ty) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub $inner);

        impl From<$inner> for $name {
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }
    };
}

integer_id!(WorldId, u64);
integer_id!(PersonId, u64);
integer_id!(SettlementId, u64);
integer_id!(HouseholdId, u64);
integer_id!(UnionId, u64);
integer_id!(FamilyLinkId, u64);

/// Stable simulation identity. Its wire form is decimal text to preserve all 64 bits in JS clients.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogicalId(pub u64);

impl Serialize for LogicalId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for LogicalId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Decimal(String),
            Number(u64),
        }
        match Repr::deserialize(deserializer)? {
            Repr::Decimal(text) => text
                .parse::<u64>()
                .map(Self)
                .map_err(serde::de::Error::custom),
            Repr::Number(number) => Ok(Self(number)),
        }
    }
}

/// Simulation years are integer, inclusive at a year's close.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Year(pub i32);

impl Year {
    pub fn checked_add(self, years: i32) -> Option<Self> {
        self.0.checked_add(years).map(Self)
    }

    pub fn age_of(self, birth_year: Year) -> Option<u32> {
        self.0
            .checked_sub(birth_year.0)
            .and_then(|age| u32::try_from(age).ok())
    }
}

impl fmt::Display for Year {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Money(pub i64);

impl Serialize for Money {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Decimal(String),
            Integer(i64),
        }
        match Repr::deserialize(deserializer)? {
            Repr::Decimal(value) => value
                .parse::<i64>()
                .map(Self)
                .map_err(serde::de::Error::custom),
            Repr::Integer(value) => Ok(Self(value)),
        }
    }
}

impl Money {
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
    pub fn is_non_negative(self) -> bool {
        self.0 >= 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quantity(pub i64);

impl Serialize for Quantity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Quantity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Decimal(String),
            Integer(i64),
        }
        match Repr::deserialize(deserializer)? {
            Repr::Decimal(value) => value
                .parse::<i64>()
                .map(Self)
                .map_err(serde::de::Error::custom),
            Repr::Integer(value) => Ok(Self(value)),
        }
    }
}

impl Quantity {
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
    pub fn is_non_negative(self) -> bool {
        self.0 >= 0
    }
}

/// Half-open interval of simulated years: `start` is included and `end_exclusive` is excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct YearInterval {
    pub start: Year,
    pub end_exclusive: Option<Year>,
}

impl YearInterval {
    pub fn contains(self, year: Year) -> bool {
        self.start <= year && self.end_exclusive.is_none_or(|end| year < end)
    }

    pub fn is_valid(self) -> bool {
        self.end_exclusive.is_none_or(|end| end >= self.start)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TraitKey {
    pub namespace: String,
    pub name: String,
}

impl Serialize for TraitKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}::{}", self.namespace, self.name))
    }
}

impl<'de> Deserialize<'de> for TraitKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let Some((namespace, name)) = encoded.split_once("::") else {
            return Err(serde::de::Error::custom(
                "trait key must use namespace::name",
            ));
        };
        if namespace.is_empty()
            || name.is_empty()
            || namespace.contains("::")
            || name.contains("::")
        {
            return Err(serde::de::Error::custom(
                "trait key has an empty component or reserved delimiter",
            ));
        }
        Ok(Self::new(namespace, name))
    }
}

impl TraitKey {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocusId(pub String);

impl From<&str> for LocusId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VariantId(pub String);

impl From<&str> for VariantId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settlement {
    pub id: SettlementId,
    /// Stable simulation identity. It is deliberately independent of storage IDs.
    pub logical_id: LogicalId,
    pub name: String,
    pub culture_values: BTreeMap<TraitKey, i16>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub id: PersonId,
    /// RNG and identity derivation use this key, never the storage ID.
    pub logical_id: LogicalId,
    pub birth_year: Year,
    pub death_year: Option<Year>,
    pub settlement_id: SettlementId,
    pub household_id: Option<HouseholdId>,
    pub reproductive: ReproductiveProfile,
    pub traits: BTreeMap<TraitKey, i16>,
    pub genome: Genome,
    /// The year in which a previous union ended by a partner's death.
    pub last_widowed_year: Option<Year>,
    /// Optional ruleset-2 occupation. Omitted for legacy rulesets and snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profession: Option<ProfessionId>,
}

impl Person {
    pub fn age_at_close(&self, year: Year) -> Option<u32> {
        year.age_of(self.birth_year)
    }

    pub fn alive_at_close(&self, year: Year) -> bool {
        self.birth_year <= year && self.death_year.is_none_or(|death| death > year)
    }

    pub fn alive_at_start(&self, year: Year) -> bool {
        self.birth_year <= year && self.death_year.is_none_or(|death| death >= year)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gamete {
    Ovum,
    Sperm,
    Other,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReproductiveProfile {
    pub gametes: BTreeSet<Gamete>,
    pub can_gestate: bool,
    pub fertility_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Household {
    pub id: HouseholdId,
    pub logical_id: LogicalId,
    pub kind: HouseholdKind,
    pub settlement_id: SettlementId,
    pub members: BTreeSet<PersonId>,
    pub closed_year: Option<Year>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HouseholdKind {
    Family,
    Collective,
    SinglePerson,
    Institution,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Union {
    pub id: UnionId,
    pub logical_id: LogicalId,
    pub participants: BTreeSet<PersonId>,
    pub kind: UnionKind,
    pub validity: YearInterval,
}

impl Union {
    pub fn active_at_close(&self, year: Year) -> bool {
        self.validity.contains(year)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnionKind {
    Partnership,
    Marriage,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyLink {
    pub id: FamilyLinkId,
    pub logical_id: LogicalId,
    /// The adult or caregiver in this relation.
    pub parent: PersonId,
    /// The child or dependent in this relation.
    pub child: PersonId,
    pub kind: FamilyLinkKind,
    pub validity: YearInterval,
}

impl FamilyLink {
    pub fn active_at_close(&self, year: Year) -> bool {
        self.validity.contains(year)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FamilyLinkKind {
    BiologicalParent,
    AdoptiveParent,
    Caregiver,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genome {
    pub loci: BTreeMap<LocusId, [Allele; 2]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Allele {
    pub variant: VariantId,
    /// Known founder lineage, if the scenario actually supplied it.
    pub lineage: Option<LogicalId>,
    /// Immediate known logical parent from whom this allele was inherited.
    pub inherited_from: Option<LogicalId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneticLocus {
    pub id: LocusId,
    pub variants: BTreeSet<VariantId>,
    pub founder_distribution: Vec<WeightedVariant>,
    pub expression: GeneticExpression,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum GeneticExpression {
    Dominant {
        dominant_variants: BTreeSet<VariantId>,
        dominant_value: i16,
        recessive_value: i16,
    },
    Additive {
        values: BTreeMap<VariantId, i16>,
    },
}

impl Genome {
    /// Resolve compact genotypes to integer phenotypes according to the scenario catalog.
    pub fn express(
        &self,
        catalog: &GeneticCatalog,
    ) -> Result<BTreeMap<LocusId, i32>, Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        let mut phenotypes = BTreeMap::new();
        for locus in &catalog.loci {
            let Some(alleles) = self.loci.get(&locus.id) else {
                issues.push(ValidationIssue::new(
                    format!("genome.loci.{}", locus.id.0),
                    "missing_locus",
                    "genome is missing a locus",
                ));
                continue;
            };
            if alleles
                .iter()
                .any(|allele| !locus.variants.contains(&allele.variant))
            {
                issues.push(ValidationIssue::new(
                    format!("genome.loci.{}", locus.id.0),
                    "unknown_variant",
                    "genome contains a variant absent from the locus",
                ));
                continue;
            }
            let value = match &locus.expression {
                GeneticExpression::Dominant {
                    dominant_variants,
                    dominant_value,
                    recessive_value,
                } => {
                    if alleles
                        .iter()
                        .any(|allele| dominant_variants.contains(&allele.variant))
                    {
                        i32::from(*dominant_value)
                    } else {
                        i32::from(*recessive_value)
                    }
                }
                GeneticExpression::Additive { values } => {
                    let Some(first) = values.get(&alleles[0].variant) else {
                        issues.push(ValidationIssue::new(
                            format!("genetics.{}.expression", locus.id.0),
                            "missing_expression_value",
                            "variant lacks an additive value",
                        ));
                        continue;
                    };
                    let Some(second) = values.get(&alleles[1].variant) else {
                        issues.push(ValidationIssue::new(
                            format!("genetics.{}.expression", locus.id.0),
                            "missing_expression_value",
                            "variant lacks an additive value",
                        ));
                        continue;
                    };
                    i32::from(*first) + i32::from(*second)
                }
            };
            phenotypes.insert(locus.id.clone(), value);
        }
        if issues.is_empty() {
            Ok(phenotypes)
        } else {
            Err(issues)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightedVariant {
    pub variant: VariantId,
    pub weight: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneticCatalog {
    pub loci: Vec<GeneticLocus>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub world_id: WorldId,
    pub schema_version: u32,
    pub rules_version: u32,
    pub seed: u64,
    pub start_year: Year,
    pub settlements: Vec<Settlement>,
    pub initial_people: Vec<Person>,
    pub initial_households: Vec<Household>,
    pub initial_unions: Vec<Union>,
    pub initial_family_links: Vec<FamilyLink>,
    pub trait_catalog: Vec<TraitDefinition>,
    pub initial_population_profiles: Vec<InitialPopulationProfile>,
    pub policies: DemographicPolicies,
    pub genetics: GeneticCatalog,
    /// Geographic and migration rules are opt-in and only valid with ruleset 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub territory: Option<TerritoryConfig>,
    /// Cultural dynamics are opt-in and only valid with ruleset 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub culture: Option<CultureConfig>,
    /// Economic rules are required in ruleset 3 and omitted in rulesets 1 and 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub economy: Option<EconomicConfig>,
    /// Scheduled shocks are opt-in and only valid with ruleset 3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_events: Option<Vec<ScheduledEvent>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraitDefinition {
    pub key: TraitKey,
    pub minimum: i16,
    pub maximum: i16,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialPopulationProfile {
    pub settlement_id: SettlementId,
    pub population_size: u32,
    pub household_size_minimum: u16,
    pub household_size_maximum: u16,
    pub household_kind: HouseholdKind,
    pub age_bands: Vec<WeightedAgeBand>,
    pub reproductive_profiles: Vec<WeightedReproductiveProfile>,
    /// Ruleset-3 starting employment mix. An empty distribution leaves everyone unassigned.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profession_distribution: Vec<WeightedProfession>,
    pub trait_distributions: BTreeMap<TraitKey, Vec<WeightedTraitValue>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedAgeBand {
    pub minimum_age: u16,
    pub maximum_age: u16,
    pub weight: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedReproductiveProfile {
    pub profile: ReproductiveProfile,
    pub weight: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedTraitValue {
    pub value: i16,
    pub weight: u32,
}

impl Scenario {
    pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;
    pub const TERRITORIAL_SCHEMA_VERSION: u32 = 2;
    pub const EVENT_SCHEMA_VERSION: u32 = 3;
    pub const SUPPORTED_RULES_VERSION: u32 = 1;
    pub const TERRITORIAL_RULES_VERSION: u32 = 2;
    pub const EVENT_RULES_VERSION: u32 = 3;

    pub fn validate(&self) -> Result<ValidatedScenario, Vec<ValidationIssue>> {
        let issues = validate_scenario(self);
        if issues.is_empty() {
            Ok(ValidatedScenario {
                scenario: self.clone(),
            })
        } else {
            Err(issues)
        }
    }

    /// Validates a revised rules-2 or rules-3 scenario for the first child-branch close.
    /// The stored cut is validated under its parent revision; only an initial person's
    /// profession availability may differ because the new policy takes effect at A0.
    pub fn validate_for_branch_revision(&self) -> Result<ValidatedScenario, Vec<ValidationIssue>> {
        if !matches!(
            self.rules_version,
            Self::TERRITORIAL_RULES_VERSION | Self::EVENT_RULES_VERSION
        ) {
            return Err(vec![ValidationIssue::new(
                "rules_version",
                "revision_rules_unsupported",
                "branch policy revisions require rules version 2 or 3",
            )]);
        }
        let issues: Vec<_> = validate_scenario(self)
            .into_iter()
            .filter(|issue| {
                !(issue.code == "profession_unavailable"
                    && issue.path.starts_with("initial_people[")
                    && issue.path.ends_with(".profession"))
            })
            .collect();
        if issues.is_empty() {
            Ok(ValidatedScenario {
                scenario: self.clone(),
            })
        } else {
            Err(issues)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedScenario {
    scenario: Scenario,
}

impl ValidatedScenario {
    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    pub fn into_scenario(self) -> Scenario {
        self.scenario
    }

    pub fn initial_state(&self) -> State {
        let mut state = State {
            world_id: self.scenario.world_id,
            rules_version: self.scenario.rules_version,
            seed: self.scenario.seed,
            start_year: self.scenario.start_year,
            current_year: None,
            next_person_id: next_id(self.scenario.initial_people.iter().map(|p| p.id.0)),
            next_household_id: next_id(self.scenario.initial_households.iter().map(|h| h.id.0)),
            next_union_id: next_id(self.scenario.initial_unions.iter().map(|u| u.id.0)),
            next_family_link_id: next_id(self.scenario.initial_family_links.iter().map(|l| l.id.0)),
            settlements: self
                .scenario
                .settlements
                .iter()
                .cloned()
                .map(|s| (s.id, s))
                .collect(),
            persons: self
                .scenario
                .initial_people
                .iter()
                .cloned()
                .map(|p| (p.id, p))
                .collect(),
            households: self
                .scenario
                .initial_households
                .iter()
                .cloned()
                .map(|h| (h.id, h))
                .collect(),
            unions: self
                .scenario
                .initial_unions
                .iter()
                .cloned()
                .map(|u| (u.id, u))
                .collect(),
            family_links: self
                .scenario
                .initial_family_links
                .iter()
                .cloned()
                .map(|l| (l.id, l))
                .collect(),
            last_gestational_birth_year: BTreeMap::new(),
            last_year_phases: Vec::new(),
            residences: if self.scenario.rules_version >= Scenario::TERRITORIAL_RULES_VERSION {
                self.scenario
                    .initial_people
                    .iter()
                    .filter(|person| person.alive_at_close(self.scenario.start_year))
                    .map(|person| {
                        (
                            person.id,
                            vec![ResidenceInterval {
                                settlement_id: person.settlement_id,
                                validity: YearInterval {
                                    start: self.scenario.start_year,
                                    end_exclusive: None,
                                },
                            }],
                        )
                    })
                    .collect()
            } else {
                BTreeMap::new()
            },
            profession_intervals: if self.scenario.rules_version
                >= Scenario::TERRITORIAL_RULES_VERSION
            {
                self.scenario
                    .initial_people
                    .iter()
                    .filter(|person| {
                        person.alive_at_close(self.scenario.start_year)
                            && person.profession.is_some()
                    })
                    .map(|person| {
                        (
                            person.id,
                            vec![ProfessionInterval {
                                profession_id: person.profession.clone().expect("filtered above"),
                                validity: YearInterval {
                                    start: self.scenario.start_year,
                                    end_exclusive: None,
                                },
                            }],
                        )
                    })
                    .collect()
            } else {
                BTreeMap::new()
            },
            last_year_settlement_balances: BTreeMap::new(),
            culture: CultureState::default(),
            economy: EconomyState::default(),
            event_effects: Vec::new(),
        };
        if self.scenario.rules_version >= Scenario::TERRITORIAL_RULES_VERSION {
            let living: BTreeSet<_> = state
                .persons
                .values()
                .filter(|person| person.alive_at_close(self.scenario.start_year))
                .map(|person| person.id)
                .collect();
            let mut detached = Vec::new();
            for household in state.households.values_mut() {
                household.members.retain(|person_id| {
                    if living.contains(person_id) {
                        true
                    } else {
                        detached.push(*person_id);
                        false
                    }
                });
                if household.members.is_empty() {
                    household.closed_year = Some(self.scenario.start_year);
                }
            }
            for person_id in detached {
                if let Some(person) = state.persons.get_mut(&person_id) {
                    person.household_id = None;
                }
            }
        }
        state
    }

    pub fn validate_state(&self, state: &State) -> Result<(), Vec<ValidationIssue>> {
        validate_state(state, Some(&self.scenario))
    }
}

fn next_id(values: impl Iterator<Item = u64>) -> u64 {
    values.max().unwrap_or(0).saturating_add(1)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(Default)]
pub struct DemographicPolicies {
    pub mortality: MortalityPolicy,
    pub unions: UnionPolicy,
    pub births: BirthPolicy,
    pub adoption: AdoptionPolicy,
    pub traits: BTreeMap<TraitKey, TraitInheritancePolicy>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(Default)]
pub struct MortalityPolicy {
    pub default_probability_ppm: u32,
    pub age_bands: Vec<AgeMortalityBand>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgeMortalityBand {
    pub minimum_age: u16,
    pub maximum_age: Option<u16>,
    pub probability_ppm: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnionPolicy {
    pub enabled: bool,
    pub minimum_age: u16,
    pub maximum_age: Option<u16>,
    pub formation_probability_ppm: u32,
    pub separation_probability_ppm: u32,
    pub candidate_limit: u16,
    pub formation_attempts_per_person: u16,
    pub max_active_unions_per_person: Option<u16>,
    pub prohibited_kinship_generations: u8,
    pub allow_widowed_remarriage: bool,
    pub kind: UnionKind,
}

impl Default for UnionPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            minimum_age: 18,
            maximum_age: None,
            formation_probability_ppm: 0,
            separation_probability_ppm: 0,
            candidate_limit: 8,
            formation_attempts_per_person: 1,
            max_active_unions_per_person: Some(1),
            prohibited_kinship_generations: 2,
            allow_widowed_remarriage: true,
            kind: UnionKind::Partnership,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BirthPolicy {
    pub enabled: bool,
    pub probability_ppm: u32,
    /// Minimum whole years between actual gestational birth years; zero permits annual births.
    #[serde(default)]
    pub minimum_years_between_births: u16,
    pub minimum_gestational_age: u16,
    pub maximum_gestational_age: u16,
    pub require_active_union: bool,
    pub required_contributor_gamete: Option<Gamete>,
    pub minimum_contributor_age: u16,
    pub allow_unknown_contributor: bool,
    pub maximum_births_per_person_per_year: u8,
    pub infant_mortality_probability_ppm: u32,
    pub child_reproductive_profiles: Vec<WeightedReproductiveProfile>,
}

impl Default for BirthPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            probability_ppm: 0,
            minimum_years_between_births: 0,
            minimum_gestational_age: 18,
            maximum_gestational_age: 45,
            require_active_union: true,
            required_contributor_gamete: Some(Gamete::Sperm),
            minimum_contributor_age: 18,
            allow_unknown_contributor: false,
            maximum_births_per_person_per_year: 1,
            infant_mortality_probability_ppm: 0,
            child_reproductive_profiles: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptionPolicy {
    pub enabled: bool,
    pub adopt_orphans: bool,
    pub adoption_probability_ppm: u32,
    pub candidate_limit: u16,
    pub minor_age: u16,
    pub minimum_caregiver_age: u16,
    pub maximum_adoptions_per_year: u32,
    pub maximum_children_per_household: Option<u32>,
    pub maximum_caregivers_per_child: u16,
    pub move_to_adoptive_household: bool,
    pub add_household_care_links: bool,
}

impl Default for AdoptionPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            adopt_orphans: false,
            adoption_probability_ppm: 0,
            candidate_limit: 8,
            minor_age: 18,
            minimum_caregiver_age: 18,
            maximum_adoptions_per_year: 16,
            maximum_children_per_household: None,
            maximum_caregivers_per_child: 2,
            move_to_adoptive_household: true,
            add_household_care_links: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraitInheritancePolicy {
    pub parent_weight: u16,
    pub local_weight: u16,
    pub variation_max: i16,
    pub minimum: i16,
    pub maximum: i16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub world_id: WorldId,
    pub rules_version: u32,
    pub seed: u64,
    pub start_year: Year,
    /// Last fully closed year. None means the world has not advanced yet.
    pub current_year: Option<Year>,
    pub next_person_id: u64,
    pub next_household_id: u64,
    pub next_union_id: u64,
    pub next_family_link_id: u64,
    pub settlements: BTreeMap<SettlementId, Settlement>,
    pub persons: BTreeMap<PersonId, Person>,
    pub households: BTreeMap<HouseholdId, Household>,
    pub unions: BTreeMap<UnionId, Union>,
    pub family_links: BTreeMap<FamilyLinkId, FamilyLink>,
    /// Last realized birth year for each gestational parent, independent of parenthood links.
    #[serde(default)]
    pub last_gestational_birth_year: BTreeMap<PersonId, Year>,
    /// Ruleset-2 residence history; empty in ruleset 1 to preserve the v1 wire format.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub residences: BTreeMap<PersonId, Vec<ResidenceInterval>>,
    /// Historical profession intervals, keyed by person.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profession_intervals: BTreeMap<PersonId, Vec<ProfessionInterval>>,
    /// Demographic and movement balance from the last closed year.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub last_year_settlement_balances: BTreeMap<SettlementId, SettlementYearBalance>,
    /// Ruleset-2 culture projections and institutional history.
    #[serde(default, skip_serializing_if = "CultureState::is_empty")]
    pub culture: CultureState,
    /// Ruleset-3 production groups and balances; empty for earlier rulesets.
    #[serde(default, skip_serializing_if = "EconomyState::is_empty")]
    pub economy: EconomyState,
    /// Currently active scheduled-event restrictions. Empty in rulesets 1 and 2.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub event_effects: Vec<ActiveEventEffect>,
    /// Explicit execution state for every A0-A11 phase of the last year.
    pub last_year_phases: Vec<PhaseRecord>,
}

impl State {
    pub fn alive_at_close(&self, person_id: PersonId, year: Year) -> bool {
        self.persons
            .get(&person_id)
            .is_some_and(|person| person.alive_at_close(year))
    }

    pub fn living_people_at(&self, year: Year) -> impl Iterator<Item = &Person> {
        self.persons
            .values()
            .filter(move |person| person.alive_at_close(year))
    }

    pub fn unions_at(&self, person_id: PersonId, year: Year) -> impl Iterator<Item = &Union> {
        self.unions.values().filter(move |union| {
            union.active_at_close(year) && union.participants.contains(&person_id)
        })
    }

    /// Returns recorded links only. An empty result means no known family links, not a request to invent parents.
    pub fn family_links_at(
        &self,
        person_id: PersonId,
        year: Year,
    ) -> impl Iterator<Item = &FamilyLink> {
        self.family_links.values().filter(move |link| {
            link.active_at_close(year)
                && self
                    .persons
                    .get(&link.child)
                    .is_some_and(|child| child.birth_year <= year)
                && self
                    .persons
                    .get(&link.parent)
                    .is_some_and(|parent| parent.birth_year <= year)
                && (link.parent == person_id || link.child == person_id)
        })
    }

    pub fn validate(&self) -> Result<(), Vec<ValidationIssue>> {
        validate_state(self, None)
    }

    pub fn person_is_incapacitated(&self, person_id: PersonId, year: Year) -> bool {
        self.event_effects.iter().any(|effect| {
            effect.active_at(year)
                && matches!(effect.effect, EventEffect::WorkIncapacity { .. })
                && effect.selected_person_ids.contains(&person_id)
        })
    }

    pub fn route_is_closed(&self, route_id: LogicalId, year: Year) -> bool {
        self.event_effects
            .iter()
            .any(|effect| effect.closes_route(route_id, year))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoricalEvent {
    pub id: LogicalId,
    pub year: Year,
    pub order: u32,
    pub affected_entities: Vec<EntityRef>,
    pub kind: HistoricalEventKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum HistoricalEventKind {
    ScheduledEventOccurred {
        event_id: LogicalId,
        name: String,
        priority: i32,
    },
    EventEffectRealized {
        event_id: LogicalId,
        event_name: String,
        effect_index: u32,
        effect: EventEffect,
        realization: EventRealization,
    },
    EconomicYearClosed {
        summary: EconomicYearClosedSummary,
    },
    PersonBorn {
        person_id: PersonId,
        gestational_parent: PersonId,
        genetic_parents: Vec<PersonId>,
        unknown_genetic_contributors: u8,
        household_id: Option<HouseholdId>,
    },
    PersonDied {
        person_id: PersonId,
        cause: DeathCause,
    },
    UnionStarted {
        union_id: UnionId,
        participants: Vec<PersonId>,
        kind: UnionKind,
    },
    UnionEnded {
        union_id: UnionId,
        reason: UnionEndReason,
    },
    FamilyLinkEstablished {
        link_id: FamilyLinkId,
        parent: PersonId,
        child: PersonId,
        kind: FamilyLinkKind,
    },
    FamilyLinkEnded {
        link_id: FamilyLinkId,
        parent: PersonId,
        child: PersonId,
        kind: FamilyLinkKind,
        reason: FamilyLinkEndReason,
    },
    HouseholdChanged {
        person_id: PersonId,
        from: Option<HouseholdId>,
        to: Option<HouseholdId>,
    },
    ResidentsMoved {
        person_ids: Vec<PersonId>,
        from: SettlementId,
        to: SettlementId,
        reason: MovementReason,
    },
    ProfessionChanged {
        person_id: PersonId,
        from: Option<ProfessionId>,
        to: Option<ProfessionId>,
        reason: ProfessionChangeReason,
    },
    InstitutionAffiliationStarted {
        institution_id: InstitutionId,
        person_id: PersonId,
        settlement_id: Option<SettlementId>,
    },
    InstitutionAffiliationEnded {
        institution_id: InstitutionId,
        person_id: PersonId,
        settlement_id: Option<SettlementId>,
        reason: InstitutionAffiliationEndReason,
    },
    OrganizationFounded {
        organization_id: OrganizationId,
        settlement_id: SettlementId,
        founders: Vec<PersonId>,
    },
    OrganizationDissolved {
        organization_id: OrganizationId,
        reason: OrganizationDissolutionReason,
    },
    OrganizationMembershipStarted {
        organization_id: OrganizationId,
        person_id: PersonId,
        founder: bool,
    },
    OrganizationMembershipEnded {
        organization_id: OrganizationId,
        person_id: PersonId,
        reason: OrganizationMembershipEndReason,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cause", content = "person_id", rename_all = "snake_case")]
pub enum FamilyLinkEndReason {
    EndpointDeath(PersonId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathCause {
    OrdinaryMortality,
    InfantMortality,
    ScheduledEvent {
        event_id: LogicalId,
        event_name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cause", content = "person_id", rename_all = "snake_case")]
pub enum UnionEndReason {
    Separation,
    Widowhood(PersonId),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityRef {
    World(WorldId),
    Person(PersonId),
    Settlement(SettlementId),
    Household(HouseholdId),
    Union(UnionId),
    FamilyLink(FamilyLinkId),
    Organization(OrganizationId),
    Route(LogicalId),
    EconomicGroup(EconomicGroupId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum YearPhase {
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
    A8,
    A9,
    A10,
    A11,
}

impl YearPhase {
    pub const ALL: [Self; 12] = [
        Self::A0,
        Self::A1,
        Self::A2,
        Self::A3,
        Self::A4,
        Self::A5,
        Self::A6,
        Self::A7,
        Self::A8,
        Self::A9,
        Self::A10,
        Self::A11,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseDisposition {
    Implemented,
    DeferredNoop,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseRecord {
    pub phase: YearPhase,
    pub disposition: PhaseDisposition,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub path: String,
    pub code: String,
    pub message: String,
}

impl ValidationIssue {
    fn new(path: impl Into<String>, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} ({})", self.path, self.message, self.code)
    }
}

fn validate_scenario(scenario: &Scenario) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    if !matches!(
        scenario.schema_version,
        Scenario::SUPPORTED_SCHEMA_VERSION
            | Scenario::TERRITORIAL_SCHEMA_VERSION
            | Scenario::EVENT_SCHEMA_VERSION
    ) {
        issues.push(ValidationIssue::new(
            "schema_version",
            "unsupported_version",
            "unsupported scenario schema version",
        ));
    }
    if !matches!(
        scenario.rules_version,
        Scenario::SUPPORTED_RULES_VERSION
            | Scenario::TERRITORIAL_RULES_VERSION
            | Scenario::EVENT_RULES_VERSION
    ) {
        issues.push(ValidationIssue::new(
            "rules_version",
            "unsupported_version",
            "unsupported simulation rules version",
        ));
    }
    validate_policies(&scenario.policies, &mut issues);
    validate_genetics(&scenario.genetics, &mut issues);
    match scenario.rules_version {
        Scenario::SUPPORTED_RULES_VERSION => {
            if scenario.schema_version != Scenario::SUPPORTED_SCHEMA_VERSION {
                issues.push(ValidationIssue::new(
                    "schema_version",
                    "schema_rules_mismatch",
                    "schema version 1 is paired with rules version 1",
                ));
            }
            if scenario.territory.is_some()
                || scenario.culture.is_some()
                || scenario.economy.is_some()
                || scenario.scheduled_events.is_some()
            {
                issues.push(ValidationIssue::new(
                    "rules_version",
                    "unsupported_e2_configuration",
                    "territory and culture configuration require rules version 2",
                ));
            }
            for (index, person) in scenario.initial_people.iter().enumerate() {
                if person.profession.is_some() {
                    issues.push(ValidationIssue::new(
                        format!("initial_people[{index}].profession"),
                        "unsupported_e2_configuration",
                        "initial professions require rules version 2",
                    ));
                }
            }
        }
        Scenario::TERRITORIAL_RULES_VERSION => {
            if scenario.schema_version != Scenario::TERRITORIAL_SCHEMA_VERSION {
                issues.push(ValidationIssue::new(
                    "schema_version",
                    "schema_rules_mismatch",
                    "schema version 2 is paired with rules version 2",
                ));
            }
            if scenario.territory.is_none() || scenario.culture.is_none() {
                issues.push(ValidationIssue::new(
                    "rules_version",
                    "missing_e2_configuration",
                    "rules version 2 requires territory and culture configuration",
                ));
            }
            if scenario.scheduled_events.is_some() {
                issues.push(ValidationIssue::new(
                    "scheduled_events",
                    "unsupported_e3_configuration",
                    "scheduled events require rules version 3",
                ));
            }
            if scenario.economy.is_some() {
                issues.push(ValidationIssue::new(
                    "economy",
                    "unsupported_e3_configuration",
                    "economic configuration requires rules version 3",
                ));
            }
            if scenario
                .initial_population_profiles
                .iter()
                .any(|profile| !profile.profession_distribution.is_empty())
            {
                issues.push(ValidationIssue::new(
                    "initial_population_profiles",
                    "unsupported_e3_configuration",
                    "initial profession distributions require rules version 3",
                ));
            }
        }
        Scenario::EVENT_RULES_VERSION => {
            if scenario.schema_version != Scenario::EVENT_SCHEMA_VERSION {
                issues.push(ValidationIssue::new(
                    "schema_version",
                    "schema_rules_mismatch",
                    "schema version 3 is paired with rules version 3",
                ));
            }
            if scenario.territory.is_none()
                || scenario.culture.is_none()
                || scenario.economy.is_none()
                || scenario.scheduled_events.is_none()
            {
                issues.push(ValidationIssue::new(
                    "rules_version",
                    "missing_e3_configuration",
                    "rules version 3 requires territory, culture, economy and scheduled_events configuration",
                ));
            }
        }
        _ => {}
    }
    let locus_catalog: BTreeMap<_, _> = scenario
        .genetics
        .loci
        .iter()
        .map(|locus| (locus.id.clone(), locus))
        .collect();

    let mut trait_catalog = BTreeMap::new();
    for (index, definition) in scenario.trait_catalog.iter().enumerate() {
        let path = format!("trait_catalog[{index}]");
        validate_trait_key(&definition.key, &path, &mut issues);
        if definition.minimum > definition.maximum {
            issues.push(ValidationIssue::new(
                &path,
                "invalid_trait_range",
                "trait minimum must not exceed maximum",
            ));
        }
        if trait_catalog
            .insert(definition.key.clone(), definition)
            .is_some()
        {
            issues.push(ValidationIssue::new(
                format!("{path}.key"),
                "duplicate_trait",
                "trait catalog keys must be unique",
            ));
        }
    }
    for (key, policy) in &scenario.policies.traits {
        let path = format!("policies.traits.{}::{}", key.namespace, key.name);
        if let Some(definition) = trait_catalog.get(key) {
            if policy.minimum < definition.minimum || policy.maximum > definition.maximum {
                issues.push(ValidationIssue::new(
                    path,
                    "trait_range_mismatch",
                    "inheritance range must fit the trait catalog range",
                ));
            }
        } else {
            issues.push(ValidationIssue::new(
                path,
                "unknown_trait",
                "inheritance policy must reference a catalog trait",
            ));
        }
    }

    let settlement_ids = unique_ids(
        scenario.settlements.iter().map(|s| s.id),
        "settlements",
        &mut issues,
    );
    let settlement_logical = unique_ids(
        scenario.settlements.iter().map(|s| s.logical_id),
        "settlements.logical_id",
        &mut issues,
    );
    let person_ids = unique_ids(
        scenario.initial_people.iter().map(|p| p.id),
        "initial_people",
        &mut issues,
    );
    let person_logical = unique_ids(
        scenario.initial_people.iter().map(|p| p.logical_id),
        "initial_people.logical_id",
        &mut issues,
    );
    let household_ids = unique_ids(
        scenario.initial_households.iter().map(|h| h.id),
        "initial_households",
        &mut issues,
    );
    let household_logical = unique_ids(
        scenario.initial_households.iter().map(|h| h.logical_id),
        "initial_households.logical_id",
        &mut issues,
    );
    let union_ids = unique_ids(
        scenario.initial_unions.iter().map(|u| u.id),
        "initial_unions",
        &mut issues,
    );
    let union_logical = unique_ids(
        scenario.initial_unions.iter().map(|u| u.logical_id),
        "initial_unions.logical_id",
        &mut issues,
    );
    let link_ids = unique_ids(
        scenario.initial_family_links.iter().map(|l| l.id),
        "initial_family_links",
        &mut issues,
    );
    let link_logical = unique_ids(
        scenario.initial_family_links.iter().map(|l| l.logical_id),
        "initial_family_links.logical_id",
        &mut issues,
    );
    let _ = (
        settlement_logical,
        person_logical,
        household_logical,
        union_logical,
        link_logical,
    );

    if let Some(events) = &scenario.scheduled_events {
        let mut known_professions: BTreeSet<_> = scenario
            .territory
            .iter()
            .flat_map(|territory| territory.professions.iter().map(|item| item.id.clone()))
            .collect();
        known_professions.extend(
            scenario
                .initial_people
                .iter()
                .filter_map(|person| person.profession.clone()),
        );
        let known_routes = scenario
            .territory
            .iter()
            .flat_map(|territory| territory.routes.iter().map(|route| route.logical_id))
            .collect();
        issues.extend(validate_event_agenda(
            events,
            scenario.start_year,
            &settlement_ids,
            &known_professions,
            &known_routes,
            scenario.territory.is_some(),
        ));
    }

    let settlements: BTreeMap<_, _> = scenario.settlements.iter().map(|s| (s.id, s)).collect();
    let people: BTreeMap<_, _> = scenario.initial_people.iter().map(|p| (p.id, p)).collect();
    let households: BTreeMap<_, _> = scenario
        .initial_households
        .iter()
        .map(|h| (h.id, h))
        .collect();
    if let Some(config) = &scenario.territory {
        let household_members = households
            .iter()
            .map(|(id, household)| (*id, &household.members))
            .collect();
        issues.extend(validate_territory_config(
            config,
            scenario.start_year,
            &settlement_ids,
            &people,
            &household_members,
        ));
    }
    if let Some(config) = &scenario.economy {
        let known_professions = scenario
            .territory
            .iter()
            .flat_map(|territory| territory.professions.iter().map(|item| item.id.clone()))
            .collect();
        let route_ids = scenario
            .territory
            .iter()
            .flat_map(|territory| territory.routes.iter().map(|route| route.logical_id))
            .collect();
        issues.extend(validate_economic_config(
            config,
            &known_professions,
            &settlement_ids,
            &route_ids,
        ));
    }
    if let Some(config) = &scenario.culture {
        let ranges: BTreeMap<_, _> = scenario
            .trait_catalog
            .iter()
            .map(|definition| {
                (
                    definition.key.clone(),
                    (definition.minimum, definition.maximum),
                )
            })
            .collect();
        issues.extend(validate_culture_config(config, &ranges));
        issues.extend(validate_culture_settlement_references(
            config,
            &settlement_ids,
        ));
        for (index, person) in scenario.initial_people.iter().enumerate() {
            for (key, policy) in &config.dimensions {
                if person
                    .traits
                    .get(key)
                    .is_some_and(|value| !(policy.minimum..=policy.maximum).contains(value))
                {
                    issues.push(ValidationIssue::new(
                        format!(
                            "initial_people[{index}].traits.{}::{}",
                            key.namespace, key.name
                        ),
                        "trait_value_out_of_range",
                        "initial cultural trait value is outside its configured range",
                    ));
                }
            }
        }
    }
    for (index, settlement) in scenario.settlements.iter().enumerate() {
        if settlement.name.trim().is_empty() {
            issues.push(ValidationIssue::new(
                format!("settlements[{index}].name"),
                "empty_name",
                "settlement name must not be empty",
            ));
        }
        validate_trait_values(
            &settlement.culture_values,
            &trait_catalog,
            &format!("settlements[{index}].culture_values"),
            &mut issues,
        );
    }
    for (index, person) in scenario.initial_people.iter().enumerate() {
        let path = format!("initial_people[{index}]");
        if person.birth_year > scenario.start_year {
            issues.push(ValidationIssue::new(
                format!("{path}.birth_year"),
                "future_birth",
                "initial people must have been born by the scenario start year",
            ));
        }
        if person
            .death_year
            .is_some_and(|death| death < person.birth_year)
        {
            issues.push(ValidationIssue::new(
                format!("{path}.death_year"),
                "death_before_birth",
                "death cannot precede birth",
            ));
        }
        if person
            .death_year
            .is_some_and(|death| death >= scenario.start_year)
        {
            issues.push(ValidationIssue::new(
                format!("{path}.death_year"),
                "future_death",
                "initial population cannot declare a death in an unprocessed year",
            ));
        }
        if !settlement_ids.contains(&person.settlement_id) {
            issues.push(ValidationIssue::new(
                format!("{path}.settlement_id"),
                "unknown_reference",
                "settlement does not exist",
            ));
        }
        if let Some(household_id) = person.household_id {
            let Some(household) = households.get(&household_id) else {
                issues.push(ValidationIssue::new(
                    format!("{path}.household_id"),
                    "unknown_reference",
                    "household does not exist",
                ));
                continue;
            };
            if !household.members.contains(&person.id) {
                issues.push(ValidationIssue::new(
                    format!("{path}.household_id"),
                    "membership_mismatch",
                    "person and household membership must agree",
                ));
            }
            if household.settlement_id != person.settlement_id {
                issues.push(ValidationIssue::new(
                    format!("{path}.household_id"),
                    "residence_mismatch",
                    "person and household must reside in the same settlement",
                ));
            }
        }
        validate_trait_values(
            &person.traits,
            &trait_catalog,
            &format!("{path}.traits"),
            &mut issues,
        );
        validate_genome(
            &person.genome,
            &scenario.genetics,
            &locus_catalog,
            &format!("{path}.genome"),
            false,
            &mut issues,
        );
    }
    for (index, household) in scenario.initial_households.iter().enumerate() {
        if !settlement_ids.contains(&household.settlement_id) {
            issues.push(ValidationIssue::new(
                format!("initial_households[{index}].settlement_id"),
                "unknown_reference",
                "settlement does not exist",
            ));
        }
        for person_id in &household.members {
            match people.get(person_id) {
                None => issues.push(ValidationIssue::new(
                    format!("initial_households[{index}].members"),
                    "unknown_reference",
                    format!("person {} does not exist", person_id.0),
                )),
                Some(person) if person.household_id != Some(household.id) => {
                    issues.push(ValidationIssue::new(
                        format!("initial_households[{index}].members"),
                        "membership_mismatch",
                        format!(
                            "person {} does not point back to this household",
                            person_id.0
                        ),
                    ))
                }
                Some(_) => {}
            }
        }
    }
    for (index, union) in scenario.initial_unions.iter().enumerate() {
        if union.participants.len() < 2 {
            issues.push(ValidationIssue::new(
                format!("initial_unions[{index}].participants"),
                "invalid_participant_count",
                "a union must contain at least two participants",
            ));
        }
        if union.validity.start > scenario.start_year {
            issues.push(ValidationIssue::new(
                format!("initial_unions[{index}].start_year"),
                "future_union",
                "initial union cannot start after the scenario start year",
            ));
        }
        if !union.validity.is_valid() {
            issues.push(ValidationIssue::new(
                format!("initial_unions[{index}].end_year"),
                "invalid_interval",
                "union end year must be later than its start year",
            ));
        }
        if union
            .validity
            .end_exclusive
            .is_some_and(|end| end > scenario.start_year)
        {
            issues.push(ValidationIssue::new(
                format!("initial_unions[{index}].end_year"),
                "future_union_end",
                "initial relationship cannot disclose a future end year",
            ));
        }
        for participant in &union.participants {
            if !person_ids.contains(participant) {
                issues.push(ValidationIssue::new(
                    format!("initial_unions[{index}].participants"),
                    "unknown_reference",
                    format!("person {} does not exist", participant.0),
                ));
            }
        }
    }
    let mut parents_by_child: BTreeMap<PersonId, Vec<PersonId>> = BTreeMap::new();
    for (index, link) in scenario.initial_family_links.iter().enumerate() {
        if !person_ids.contains(&link.parent) || !person_ids.contains(&link.child) {
            issues.push(ValidationIssue::new(format!("initial_family_links[{index}]"), "unknown_reference", "family links must reference known people; unknown parents are represented by no link"));
            continue;
        }
        if link.parent == link.child {
            issues.push(ValidationIssue::new(
                format!("initial_family_links[{index}]"),
                "self_relation",
                "a person cannot be their own parent or caregiver",
            ));
        }
        if link.validity.start > scenario.start_year
            || link.validity.start < people[&link.child].birth_year
            || link.validity.start < people[&link.parent].birth_year
        {
            issues.push(ValidationIssue::new(format!("initial_family_links[{index}].start_year"), "invalid_start_year", "family relation must start after the child exists and no later than scenario start"));
        }
        if !link.validity.is_valid() {
            issues.push(ValidationIssue::new(
                format!("initial_family_links[{index}].end_year"),
                "invalid_interval",
                "family relation end year must be later than its start year",
            ));
        }
        if link
            .validity
            .end_exclusive
            .is_some_and(|end| end > scenario.start_year)
        {
            issues.push(ValidationIssue::new(
                format!("initial_family_links[{index}].end_year"),
                "future_link_end",
                "initial relationship cannot disclose a future end year",
            ));
        }
        if matches!(link.kind, FamilyLinkKind::BiologicalParent)
            && people[&link.parent].birth_year >= people[&link.child].birth_year
        {
            issues.push(ValidationIssue::new(
                format!("initial_family_links[{index}]"),
                "invalid_biological_age",
                "biological parent must be born before the child",
            ));
        }
        parents_by_child
            .entry(link.child)
            .or_default()
            .push(link.parent);
    }
    if has_cycle(&parents_by_child) {
        issues.push(ValidationIssue::new(
            "initial_family_links",
            "family_cycle",
            "parent and care relationships must be acyclic",
        ));
    }
    validate_initial_union_policies(scenario, &people, &parents_by_child, &mut issues);
    for (index, profile) in scenario.initial_population_profiles.iter().enumerate() {
        let path = format!("initial_population_profiles[{index}]");
        if !settlement_ids.contains(&profile.settlement_id) {
            issues.push(ValidationIssue::new(
                format!("{path}.settlement_id"),
                "unknown_reference",
                "settlement does not exist",
            ));
        }
        if profile.population_size > 0
            && (profile.household_size_minimum == 0
                || profile.household_size_maximum < profile.household_size_minimum)
        {
            issues.push(ValidationIssue::new(
                format!("{path}.household_size_minimum"),
                "invalid_household_size",
                "household size range must be positive and ordered",
            ));
        }
        if scenario.rules_version < Scenario::EVENT_RULES_VERSION
            && !profile.profession_distribution.is_empty()
        {
            issues.push(ValidationIssue::new(
                format!("{path}.profession_distribution"),
                "unsupported_e3_configuration",
                "initial profession distributions require rules version 3",
            ));
        }
        let profession_catalog: BTreeSet<_> = scenario
            .territory
            .iter()
            .flat_map(|territory| territory.professions.iter().map(|item| item.id.clone()))
            .collect();
        let available_professions = scenario.territory.as_ref().and_then(|territory| {
            territory
                .profession_options_by_settlement
                .get(&profile.settlement_id)
        });
        if profile.profession_distribution.iter().any(|entry| {
            entry.weight == 0
                || entry.profession_id.as_ref().is_some_and(|profession_id| {
                    !profession_catalog.contains(profession_id)
                        || !available_professions
                            .is_some_and(|available| available.contains(profession_id))
                })
        }) || (!profile.profession_distribution.is_empty()
            && profile
                .profession_distribution
                .iter()
                .all(|entry| entry.weight == 0))
        {
            issues.push(ValidationIssue::new(
                format!("{path}.profession_distribution"),
                "invalid_distribution",
                "initial profession distribution needs positive weights and professions available at the settlement",
            ));
        }
        if profile.population_size > 0 && profile.age_bands.is_empty() {
            issues.push(ValidationIssue::new(
                format!("{path}.age_bands"),
                "empty_distribution",
                "population age distribution needs at least one band",
            ));
        }
        for (band_index, band) in profile.age_bands.iter().enumerate() {
            if band.maximum_age < band.minimum_age || band.weight == 0 {
                issues.push(ValidationIssue::new(
                    format!("{path}.age_bands[{band_index}]"),
                    "invalid_age_band",
                    "age band needs ordered bounds and positive weight",
                ));
            }
        }
        if profile.population_size > 0
            && profile
                .reproductive_profiles
                .iter()
                .map(|entry| u64::from(entry.weight))
                .sum::<u64>()
                == 0
        {
            issues.push(ValidationIssue::new(
                format!("{path}.reproductive_profiles"),
                "empty_distribution",
                "population profile needs positive reproductive profile weight",
            ));
        }
        if profile
            .reproductive_profiles
            .iter()
            .any(|entry| entry.weight == 0)
        {
            issues.push(ValidationIssue::new(
                format!("{path}.reproductive_profiles"),
                "invalid_weight",
                "reproductive profile weights must be positive",
            ));
        }
        for (key, values) in &profile.trait_distributions {
            let key_path = format!("{path}.trait_distributions.{}::{}", key.namespace, key.name);
            validate_trait_key(key, &key_path, &mut issues);
            let Some(definition) = trait_catalog.get(key) else {
                issues.push(ValidationIssue::new(
                    key_path,
                    "unknown_trait",
                    "initial trait distribution must reference a catalog trait",
                ));
                continue;
            };
            if values.is_empty()
                || values.iter().any(|value| {
                    value.weight == 0
                        || value.value < definition.minimum
                        || value.value > definition.maximum
                })
            {
                issues.push(ValidationIssue::new(
                    key_path,
                    "invalid_distribution",
                    "trait distribution needs positive weights and values within catalog range",
                ));
            }
        }
    }
    let _ = (household_ids, union_ids, link_ids, settlements);
    issues
}

fn validate_policies(policies: &DemographicPolicies, issues: &mut Vec<ValidationIssue>) {
    validate_ppm(
        policies.mortality.default_probability_ppm,
        "policies.mortality.default_probability_ppm",
        issues,
    );
    let mut bands = policies.mortality.age_bands.clone();
    bands.sort_by_key(|band| band.minimum_age);
    for (i, band) in bands.iter().enumerate() {
        validate_ppm(
            band.probability_ppm,
            &format!("policies.mortality.age_bands[{i}].probability_ppm"),
            issues,
        );
        if band.maximum_age.is_some_and(|max| max < band.minimum_age) {
            issues.push(ValidationIssue::new(
                format!("policies.mortality.age_bands[{i}]"),
                "invalid_age_band",
                "maximum age must be at least minimum age",
            ));
        }
        if i > 0
            && bands[i - 1]
                .maximum_age
                .is_none_or(|max| max >= band.minimum_age)
        {
            issues.push(ValidationIssue::new(
                format!("policies.mortality.age_bands[{i}]"),
                "overlapping_age_bands",
                "mortality age bands must not overlap",
            ));
        }
    }
    let union = &policies.unions;
    validate_ppm(
        union.formation_probability_ppm,
        "policies.unions.formation_probability_ppm",
        issues,
    );
    validate_ppm(
        union.separation_probability_ppm,
        "policies.unions.separation_probability_ppm",
        issues,
    );
    if union.candidate_limit == 0 || union.candidate_limit > 1024 {
        issues.push(ValidationIssue::new(
            "policies.unions.candidate_limit",
            "out_of_range",
            "candidate limit must be between 1 and 1024",
        ));
    }
    if union.formation_attempts_per_person == 0 || union.formation_attempts_per_person > 64 {
        issues.push(ValidationIssue::new(
            "policies.unions.formation_attempts_per_person",
            "out_of_range",
            "formation attempts must be between 1 and 64",
        ));
    }
    if union.maximum_age.is_some_and(|max| max < union.minimum_age) {
        issues.push(ValidationIssue::new(
            "policies.unions.maximum_age",
            "invalid_age_range",
            "maximum age must be at least minimum age",
        ));
    }
    if union.max_active_unions_per_person == Some(0) {
        issues.push(ValidationIssue::new(
            "policies.unions.max_active_unions_per_person",
            "out_of_range",
            "a configured union limit must be at least one",
        ));
    }
    let births = &policies.births;
    validate_ppm(
        births.probability_ppm,
        "policies.births.probability_ppm",
        issues,
    );
    validate_ppm(
        births.infant_mortality_probability_ppm,
        "policies.births.infant_mortality_probability_ppm",
        issues,
    );
    if births.maximum_gestational_age < births.minimum_gestational_age {
        issues.push(ValidationIssue::new(
            "policies.births",
            "invalid_age_range",
            "maximum gestational age must be at least minimum age",
        ));
    }
    if births.maximum_births_per_person_per_year == 0 {
        issues.push(ValidationIssue::new(
            "policies.births.maximum_births_per_person_per_year",
            "out_of_range",
            "maximum births per person must be at least one",
        ));
    }
    if births
        .child_reproductive_profiles
        .iter()
        .any(|profile| profile.weight == 0)
    {
        issues.push(ValidationIssue::new(
            "policies.births.child_reproductive_profiles",
            "invalid_weight",
            "reproductive profile weights must be positive",
        ));
    }
    let adoption = &policies.adoption;
    validate_ppm(
        adoption.adoption_probability_ppm,
        "policies.adoption.adoption_probability_ppm",
        issues,
    );
    if adoption.candidate_limit == 0 || adoption.candidate_limit > 1024 {
        issues.push(ValidationIssue::new(
            "policies.adoption.candidate_limit",
            "out_of_range",
            "candidate limit must be between 1 and 1024",
        ));
    }
    if adoption.maximum_caregivers_per_child == 0 {
        issues.push(ValidationIssue::new(
            "policies.adoption.maximum_caregivers_per_child",
            "out_of_range",
            "at least one caregiver slot is required",
        ));
    }
    for (key, policy) in &policies.traits {
        if key.namespace.trim().is_empty() || key.name.trim().is_empty() {
            issues.push(ValidationIssue::new(
                "policies.traits",
                "empty_trait_key",
                "trait namespace and name must not be empty",
            ));
        }
        if policy.parent_weight == 0 && policy.local_weight == 0 {
            issues.push(ValidationIssue::new(
                format!("policies.traits.{}:{}", key.namespace, key.name),
                "zero_trait_weights",
                "at least one inheritance weight must be positive",
            ));
        }
        if policy.minimum > policy.maximum || policy.variation_max < 0 {
            issues.push(ValidationIssue::new(
                format!("policies.traits.{}:{}", key.namespace, key.name),
                "invalid_trait_range",
                "trait bounds or variation are invalid",
            ));
        }
    }
}

fn validate_ppm(value: u32, path: &str, issues: &mut Vec<ValidationIssue>) {
    if value > 1_000_000 {
        issues.push(ValidationIssue::new(
            path,
            "out_of_range",
            "probability must be between 0 and 1,000,000 ppm",
        ));
    }
}

fn validate_initial_union_policies(
    scenario: &Scenario,
    people: &BTreeMap<PersonId, &Person>,
    parents_by_child: &BTreeMap<PersonId, Vec<PersonId>>,
    issues: &mut Vec<ValidationIssue>,
) {
    let policy = &scenario.policies.unions;
    let mut active_count: BTreeMap<PersonId, u16> = BTreeMap::new();
    for (index, union) in scenario.initial_unions.iter().enumerate() {
        for participant_id in &union.participants {
            let Some(person) = people.get(participant_id) else {
                continue;
            };
            let Some(age) = union.validity.start.age_of(person.birth_year) else {
                issues.push(ValidationIssue::new(
                    format!("initial_unions[{index}].participants"),
                    "union_before_birth",
                    "initial union cannot begin before a participant is born",
                ));
                continue;
            };
            if age < u32::from(policy.minimum_age)
                || policy.maximum_age.is_some_and(|max| age > u32::from(max))
            {
                issues.push(ValidationIssue::new(
                    format!("initial_unions[{index}].participants"),
                    "ineligible_age",
                    "initial union participant is outside configured union age limits",
                ));
            }
            if union.validity.contains(scenario.start_year) {
                if !person.alive_at_close(scenario.start_year) {
                    issues.push(ValidationIssue::new(
                        format!("initial_unions[{index}].participants"),
                        "deceased_participant",
                        "active initial union participant must be alive at scenario start",
                    ));
                }
                let count = active_count.entry(*participant_id).or_default();
                *count = count.saturating_add(1);
            }
        }
        let participants: Vec<_> = union.participants.iter().copied().collect();
        for left_index in 0..participants.len() {
            for right_index in left_index + 1..participants.len() {
                if related_within_depth(
                    participants[left_index],
                    participants[right_index],
                    parents_by_child,
                    policy.prohibited_kinship_generations,
                ) {
                    issues.push(ValidationIssue::new(
                        format!("initial_unions[{index}].participants"),
                        "prohibited_kinship",
                        "initial union violates configured kinship restrictions",
                    ));
                }
            }
        }
    }
    if let Some(limit) = policy.max_active_unions_per_person {
        if active_count.values().any(|count| *count > limit) {
            issues.push(ValidationIssue::new(
                "initial_unions",
                "too_many_active_unions",
                "initial relationships exceed the configured active union limit",
            ));
        }
    }
}

fn related_within_depth(
    left: PersonId,
    right: PersonId,
    parents_by_child: &BTreeMap<PersonId, Vec<PersonId>>,
    maximum_generations: u8,
) -> bool {
    if maximum_generations == 0 {
        return false;
    }
    let left_ancestors = ancestor_depths(left, parents_by_child, maximum_generations);
    let right_ancestors = ancestor_depths(right, parents_by_child, maximum_generations);
    left_ancestors.iter().any(|(ancestor, left_depth)| {
        right_ancestors.get(ancestor).is_some_and(|right_depth| {
            left_depth.saturating_add(*right_depth) <= maximum_generations
        })
    })
}

fn ancestor_depths(
    person: PersonId,
    parents_by_child: &BTreeMap<PersonId, Vec<PersonId>>,
    maximum_generations: u8,
) -> BTreeMap<PersonId, u8> {
    let mut result = BTreeMap::new();
    let mut frontier = vec![(person, 0_u8)];
    while let Some((current, depth)) = frontier.pop() {
        if depth >= maximum_generations {
            continue;
        }
        if let Some(parents) = parents_by_child.get(&current) {
            for parent in parents {
                let next_depth = depth + 1;
                if result.get(parent).is_some_and(|known| *known <= next_depth) {
                    continue;
                }
                result.insert(*parent, next_depth);
                frontier.push((*parent, next_depth));
            }
        }
    }
    result
}

fn validate_genetics(genetics: &GeneticCatalog, issues: &mut Vec<ValidationIssue>) {
    let mut locus_ids = BTreeSet::new();
    for (index, locus) in genetics.loci.iter().enumerate() {
        let path = format!("genetics.loci[{index}]");
        if locus.id.0.trim().is_empty() || !locus_ids.insert(locus.id.clone()) {
            issues.push(ValidationIssue::new(
                format!("{path}.id"),
                "invalid_or_duplicate_id",
                "genetic locus IDs must be non-empty and unique",
            ));
        }
        if locus.variants.is_empty() {
            issues.push(ValidationIssue::new(
                format!("{path}.variants"),
                "empty_catalog",
                "a genetic locus must declare at least one variant",
            ));
        }
        let total_weight: u64 = locus
            .founder_distribution
            .iter()
            .map(|entry| u64::from(entry.weight))
            .sum();
        if total_weight == 0 {
            issues.push(ValidationIssue::new(
                format!("{path}.founder_distribution"),
                "empty_distribution",
                "founder distribution must have positive total weight",
            ));
        }
        for entry in &locus.founder_distribution {
            if entry.weight == 0 || !locus.variants.contains(&entry.variant) {
                issues.push(ValidationIssue::new(
                    format!("{path}.founder_distribution"),
                    "invalid_variant",
                    "distribution entries require a known variant and positive weight",
                ));
            }
        }
        match &locus.expression {
            GeneticExpression::Dominant {
                dominant_variants, ..
            } => {
                if dominant_variants
                    .iter()
                    .any(|variant| !locus.variants.contains(variant))
                {
                    issues.push(ValidationIssue::new(
                        format!("{path}.expression.dominant_variants"),
                        "unknown_variant",
                        "dominance rules may only refer to declared variants",
                    ));
                }
            }
            GeneticExpression::Additive { values } => {
                for variant in &locus.variants {
                    if !values.contains_key(variant) {
                        issues.push(ValidationIssue::new(
                            format!("{path}.expression.values"),
                            "missing_expression_value",
                            format!("variant {} lacks an additive value", variant.0),
                        ));
                    }
                }
                if values
                    .keys()
                    .any(|variant| !locus.variants.contains(variant))
                {
                    issues.push(ValidationIssue::new(
                        format!("{path}.expression.values"),
                        "unknown_variant",
                        "additive values may only refer to declared variants",
                    ));
                }
            }
        }
    }
}

fn validate_trait_values(
    values: &BTreeMap<TraitKey, i16>,
    catalog: &BTreeMap<TraitKey, &TraitDefinition>,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    for (key, value) in values {
        let key_path = format!("{path}.{}::{}", key.namespace, key.name);
        validate_trait_key(key, &key_path, issues);
        match catalog.get(key) {
            None => issues.push(ValidationIssue::new(
                key_path,
                "unknown_trait",
                "trait is not declared in the scenario catalog",
            )),
            Some(definition) if *value < definition.minimum || *value > definition.maximum => {
                issues.push(ValidationIssue::new(
                    key_path,
                    "trait_value_out_of_range",
                    "trait value is outside its declared range",
                ))
            }
            Some(_) => {}
        }
    }
}

fn validate_trait_key(key: &TraitKey, path: &str, issues: &mut Vec<ValidationIssue>) {
    if key.namespace.trim().is_empty()
        || key.name.trim().is_empty()
        || key.namespace.contains("::")
        || key.name.contains("::")
    {
        issues.push(ValidationIssue::new(
            path,
            "invalid_trait_key",
            "trait key requires non-empty namespace and name; '::' is reserved",
        ));
    }
}

fn validate_genome(
    genome: &Genome,
    genetics: &GeneticCatalog,
    catalog: &BTreeMap<LocusId, &GeneticLocus>,
    path: &str,
    require_complete: bool,
    issues: &mut Vec<ValidationIssue>,
) {
    for locus_id in genome.loci.keys() {
        if !catalog.contains_key(locus_id) {
            issues.push(ValidationIssue::new(
                format!("{path}.loci.{}", locus_id.0),
                "unknown_locus",
                "genome uses a locus absent from this scenario",
            ));
        }
    }
    for locus in &genetics.loci {
        match genome.loci.get(&locus.id) {
            None if require_complete => issues.push(ValidationIssue::new(
                format!("{path}.loci.{}", locus.id.0),
                "missing_locus",
                "genome is missing a required locus",
            )),
            None => {}
            Some(alleles) => {
                for (i, allele) in alleles.iter().enumerate() {
                    if !locus.variants.contains(&allele.variant) {
                        issues.push(ValidationIssue::new(
                            format!("{path}.loci.{}.alleles[{i}]", locus.id.0),
                            "unknown_variant",
                            "allele variant is absent from the catalog",
                        ));
                    }
                }
            }
        }
    }
}

fn unique_ids<T: Copy + Ord>(
    values: impl Iterator<Item = T>,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) -> BTreeSet<T> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            issues.push(ValidationIssue::new(
                path,
                "duplicate_id",
                "IDs must be unique",
            ));
        }
    }
    seen
}

fn has_cycle(parents_by_child: &BTreeMap<PersonId, Vec<PersonId>>) -> bool {
    fn visit(
        node: PersonId,
        map: &BTreeMap<PersonId, Vec<PersonId>>,
        visiting: &mut BTreeSet<PersonId>,
        done: &mut BTreeSet<PersonId>,
    ) -> bool {
        if done.contains(&node) {
            return false;
        }
        if !visiting.insert(node) {
            return true;
        }
        if let Some(parents) = map.get(&node) {
            for parent in parents {
                if visit(*parent, map, visiting, done) {
                    return true;
                }
            }
        }
        visiting.remove(&node);
        done.insert(node);
        false
    }
    let mut visiting = BTreeSet::new();
    let mut done = BTreeSet::new();
    parents_by_child
        .keys()
        .copied()
        .any(|node| visit(node, parents_by_child, &mut visiting, &mut done))
}

fn validate_state(state: &State, scenario: Option<&Scenario>) -> Result<(), Vec<ValidationIssue>> {
    let mut issues = Vec::new();
    if !matches!(
        state.rules_version,
        Scenario::SUPPORTED_RULES_VERSION
            | Scenario::TERRITORIAL_RULES_VERSION
            | Scenario::EVENT_RULES_VERSION
    ) {
        issues.push(ValidationIssue::new(
            "rules_version",
            "unsupported_version",
            "unsupported state rules version",
        ));
    }
    if state.rules_version < Scenario::EVENT_RULES_VERSION
        && (!state.event_effects.is_empty() || !state.economy.is_empty())
    {
        issues.push(ValidationIssue::new(
            "event_effects",
            "unsupported_event_state",
            "event effects require rules version 3",
        ));
    }
    if state.rules_version == Scenario::SUPPORTED_RULES_VERSION
        && (!state.residences.is_empty()
            || !state.profession_intervals.is_empty()
            || !state.last_year_settlement_balances.is_empty()
            || !state.culture.is_empty()
            || !state.economy.is_empty()
            || !state.event_effects.is_empty()
            || state
                .persons
                .values()
                .any(|person| person.profession.is_some()))
    {
        issues.push(ValidationIssue::new(
            "state",
            "unsupported_e2_state",
            "rules version 1 cannot contain rules version 2 state",
        ));
    }
    let trait_catalog: BTreeMap<_, _> = scenario
        .into_iter()
        .flat_map(|scenario| scenario.trait_catalog.iter())
        .map(|definition| (definition.key.clone(), definition))
        .collect();
    let genetic_catalog: Option<BTreeMap<_, _>> = scenario.map(|scenario| {
        scenario
            .genetics
            .loci
            .iter()
            .map(|locus| (locus.id.clone(), locus))
            .collect()
    });
    let last_year = state
        .current_year
        .unwrap_or_else(|| Year(state.start_year.0.saturating_sub(1)));
    if state
        .current_year
        .is_some_and(|year| year < state.start_year)
    {
        issues.push(ValidationIssue::new(
            "current_year",
            "before_start",
            "current year cannot precede the scenario start",
        ));
    }
    if state.current_year.is_none() && !state.last_year_phases.is_empty() {
        issues.push(ValidationIssue::new(
            "last_year_phases",
            "unexpected_phase_log",
            "an unadvanced state cannot have a completed year phase log",
        ));
    }
    if state.current_year.is_some() && state.last_year_phases.len() != YearPhase::ALL.len() {
        issues.push(ValidationIssue::new(
            "last_year_phases",
            "incomplete_phase_log",
            "a closed year must record A0 through A11",
        ));
    }
    let mut ids = BTreeSet::new();
    for (id, settlement) in &state.settlements {
        if *id != settlement.id {
            issues.push(ValidationIssue::new(
                "settlements",
                "map_key_mismatch",
                "settlement map key must match the entity ID",
            ));
        }
        if !ids.insert(settlement.logical_id) {
            issues.push(ValidationIssue::new(
                "settlements",
                "duplicate_logical_id",
                "logical IDs must be unique within an entity category",
            ));
        }
    }
    ids.clear();
    for (id, person) in &state.persons {
        if *id != person.id {
            issues.push(ValidationIssue::new(
                "persons",
                "map_key_mismatch",
                "person map key must match the entity ID",
            ));
        }
        if !ids.insert(person.logical_id) {
            issues.push(ValidationIssue::new(
                "persons",
                "duplicate_logical_id",
                "person logical IDs must be unique",
            ));
        }
        if !state.settlements.contains_key(&person.settlement_id) {
            issues.push(ValidationIssue::new(
                format!("persons[{}].settlement_id", id.0),
                "unknown_reference",
                "settlement does not exist",
            ));
        }
        if person.birth_year > last_year && state.current_year.is_some() {
            issues.push(ValidationIssue::new(
                format!("persons[{}].birth_year", id.0),
                "future_birth",
                "person cannot exist before their birth year",
            ));
        }
        if person
            .death_year
            .is_some_and(|year| year < person.birth_year)
        {
            issues.push(ValidationIssue::new(
                format!("persons[{}].death_year", id.0),
                "death_before_birth",
                "death cannot precede birth",
            ));
        }
        if let Some(death_year) = person.death_year {
            let unprocessed_year = state.current_year.map_or(state.start_year, |current| {
                current.checked_add(1).unwrap_or(current)
            });
            if death_year >= unprocessed_year {
                issues.push(ValidationIssue::new(
                    format!("persons[{}].death_year", id.0),
                    "future_death",
                    "death must be a realized historical event",
                ));
            }
        }
        if let Some(household_id) = person.household_id {
            match state.households.get(&household_id) {
                None => issues.push(ValidationIssue::new(
                    format!("persons[{}].household_id", id.0),
                    "unknown_reference",
                    "household does not exist",
                )),
                Some(household) if !household.members.contains(id) => {
                    issues.push(ValidationIssue::new(
                        format!("persons[{}].household_id", id.0),
                        "membership_mismatch",
                        "household must contain its person",
                    ))
                }
                Some(household) if household.settlement_id != person.settlement_id => {
                    issues.push(ValidationIssue::new(
                        format!("persons[{}].household_id", id.0),
                        "residence_mismatch",
                        "person and household must reside in the same settlement",
                    ))
                }
                Some(_) => {}
            }
        }
        if let Some(scenario) = scenario {
            validate_trait_values(
                &person.traits,
                &trait_catalog,
                &format!("persons[{}].traits", id.0),
                &mut issues,
            );
            validate_genome(
                &person.genome,
                &scenario.genetics,
                genetic_catalog.as_ref().expect("compiled for scenario"),
                &format!("persons[{}].genome", id.0),
                true,
                &mut issues,
            );
        }
    }
    validate_residence_history(state, &mut issues);
    validate_profession_history(state, scenario, &mut issues);
    validate_settlement_balances(state, &mut issues);
    for (person_id, birth_year) in &state.last_gestational_birth_year {
        let path = format!("last_gestational_birth_year[{}]", person_id.0);
        match state.persons.get(person_id) {
            None => issues.push(ValidationIssue::new(
                path.clone(),
                "unknown_reference",
                "gestational parent does not exist",
            )),
            Some(person) if *birth_year < person.birth_year => issues.push(ValidationIssue::new(
                path.clone(),
                "birth_before_person",
                "recorded birth cannot precede the gestational parent's birth",
            )),
            Some(_) => {}
        }
        if *birth_year < state.start_year {
            issues.push(ValidationIssue::new(
                path.clone(),
                "birth_before_start",
                "recorded gestational birth cannot precede the simulation start",
            ));
        }
        if state
            .current_year
            .is_none_or(|current_year| *birth_year > current_year)
        {
            issues.push(ValidationIssue::new(
                path,
                "future_birth",
                "recorded gestational birth must belong to a closed year",
            ));
        }
    }
    ids.clear();
    for (id, household) in &state.households {
        if *id != household.id {
            issues.push(ValidationIssue::new(
                "households",
                "map_key_mismatch",
                "household map key must match the entity ID",
            ));
        }
        if !ids.insert(household.logical_id) {
            issues.push(ValidationIssue::new(
                "households",
                "duplicate_logical_id",
                "household logical IDs must be unique",
            ));
        }
        if !state.settlements.contains_key(&household.settlement_id) {
            issues.push(ValidationIssue::new(
                format!("households[{}].settlement_id", id.0),
                "unknown_reference",
                "settlement does not exist",
            ));
        }
        for person_id in &household.members {
            match state.persons.get(person_id) {
                None => issues.push(ValidationIssue::new(
                    format!("households[{}].members", id.0),
                    "unknown_reference",
                    "household member does not exist",
                )),
                Some(person) if person.household_id != Some(*id) => {
                    issues.push(ValidationIssue::new(
                        format!("households[{}].members", id.0),
                        "membership_mismatch",
                        "member does not point back to this household",
                    ))
                }
                Some(_) => {}
            }
        }
    }
    ids.clear();
    for (id, union) in &state.unions {
        if *id != union.id {
            issues.push(ValidationIssue::new(
                "unions",
                "map_key_mismatch",
                "union map key must match the entity ID",
            ));
        }
        if !ids.insert(union.logical_id) {
            issues.push(ValidationIssue::new(
                "unions",
                "duplicate_logical_id",
                "union logical IDs must be unique",
            ));
        }
        if union.participants.len() < 2 {
            issues.push(ValidationIssue::new(
                format!("unions[{}].participants", id.0),
                "invalid_participant_count",
                "a union must contain at least two participants",
            ));
        }
        if !union.validity.is_valid() {
            issues.push(ValidationIssue::new(
                format!("unions[{}]", id.0),
                "invalid_interval",
                "union end year must be later than its start year",
            ));
        }
        if state.current_year.is_some_and(|year| {
            union.validity.start > year
                || union.validity.end_exclusive.is_some_and(|end| end > year)
        }) {
            issues.push(ValidationIssue::new(
                format!("unions[{}]", id.0),
                "future_interval",
                "closed state cannot contain future union dates",
            ));
        }
        for participant in &union.participants {
            if !state.persons.contains_key(participant) {
                issues.push(ValidationIssue::new(
                    format!("unions[{}].participants", id.0),
                    "unknown_reference",
                    "union participant does not exist",
                ));
            }
            if state.current_year.is_some_and(|year| {
                union.active_at_close(year) && !state.alive_at_close(*participant, year)
            }) {
                issues.push(ValidationIssue::new(
                    format!("unions[{}].participants", id.0),
                    "deceased_participant",
                    "an active union cannot contain a person who is dead at year close",
                ));
            }
        }
    }
    ids.clear();
    let mut graph: BTreeMap<PersonId, Vec<PersonId>> = BTreeMap::new();
    for (id, link) in &state.family_links {
        if *id != link.id {
            issues.push(ValidationIssue::new(
                "family_links",
                "map_key_mismatch",
                "family link map key must match the entity ID",
            ));
        }
        if !ids.insert(link.logical_id) {
            issues.push(ValidationIssue::new(
                "family_links",
                "duplicate_logical_id",
                "family link logical IDs must be unique",
            ));
        }
        if !state.persons.contains_key(&link.parent) || !state.persons.contains_key(&link.child) {
            issues.push(ValidationIssue::new(
                format!("family_links[{}]", id.0),
                "unknown_reference",
                "family link endpoint does not exist",
            ));
        }
        if link.parent == link.child {
            issues.push(ValidationIssue::new(
                format!("family_links[{}]", id.0),
                "self_relation",
                "person cannot be their own parent or caregiver",
            ));
        }
        if !link.validity.is_valid() {
            issues.push(ValidationIssue::new(
                format!("family_links[{}]", id.0),
                "invalid_interval",
                "family link end year must be later than start year",
            ));
        }
        if state.current_year.is_some_and(|year| {
            link.validity.start > year || link.validity.end_exclusive.is_some_and(|end| end > year)
        }) {
            issues.push(ValidationIssue::new(
                format!("family_links[{}]", id.0),
                "future_interval",
                "closed state cannot contain future family-link dates",
            ));
        }
        if let Some(child) = state.persons.get(&link.child) {
            if link.validity.start < child.birth_year {
                issues.push(ValidationIssue::new(
                    format!("family_links[{}]", id.0),
                    "link_before_birth",
                    "family link cannot predate the child",
                ));
            }
        }
        if let Some(parent) = state.persons.get(&link.parent) {
            if link.validity.start < parent.birth_year {
                issues.push(ValidationIssue::new(
                    format!("family_links[{}]", id.0),
                    "link_before_parent_birth",
                    "family link cannot predate its parent or caregiver",
                ));
            }
        }
        if matches!(link.kind, FamilyLinkKind::BiologicalParent) {
            if let (Some(parent), Some(child)) = (
                state.persons.get(&link.parent),
                state.persons.get(&link.child),
            ) {
                if parent.birth_year >= child.birth_year {
                    issues.push(ValidationIssue::new(
                        format!("family_links[{}]", id.0),
                        "invalid_biological_age",
                        "biological parent must be born before the child",
                    ));
                }
            }
        }
        graph.entry(link.child).or_default().push(link.parent);
    }
    if has_cycle(&graph) {
        issues.push(ValidationIssue::new(
            "family_links",
            "family_cycle",
            "parent and care relationships must be acyclic",
        ));
    }
    if state.next_person_id <= state.persons.keys().map(|id| id.0).max().unwrap_or(0) {
        issues.push(ValidationIssue::new(
            "next_person_id",
            "id_allocator_collision",
            "next person ID must exceed existing storage IDs",
        ));
    }
    if state.next_household_id <= state.households.keys().map(|id| id.0).max().unwrap_or(0) {
        issues.push(ValidationIssue::new(
            "next_household_id",
            "id_allocator_collision",
            "next household ID must exceed existing storage IDs",
        ));
    }
    if state.next_union_id <= state.unions.keys().map(|id| id.0).max().unwrap_or(0) {
        issues.push(ValidationIssue::new(
            "next_union_id",
            "id_allocator_collision",
            "next union ID must exceed existing storage IDs",
        ));
    }
    if state.next_family_link_id <= state.family_links.keys().map(|id| id.0).max().unwrap_or(0) {
        issues.push(ValidationIssue::new(
            "next_family_link_id",
            "id_allocator_collision",
            "next family link ID must exceed existing storage IDs",
        ));
    }
    if let Some(scenario) = scenario {
        if state.world_id != scenario.world_id
            || state.rules_version != scenario.rules_version
            || state.seed != scenario.seed
            || state.start_year != scenario.start_year
        {
            issues.push(ValidationIssue::new(
                "state",
                "scenario_mismatch",
                "state world, seed, rules or start year do not match the validated scenario",
            ));
        }
        for settlement in state.settlements.values() {
            validate_trait_values(
                &settlement.culture_values,
                &trait_catalog,
                "state.settlements.culture_values",
                &mut issues,
            );
        }
        if state.rules_version >= Scenario::TERRITORIAL_RULES_VERSION {
            if scenario.territory.is_none() || scenario.culture.is_none() {
                issues.push(ValidationIssue::new(
                    "state",
                    "missing_e2_configuration",
                    "rules version 2 state requires both territory and culture configuration",
                ));
            }
            if state.rules_version >= Scenario::EVENT_RULES_VERSION
                && (scenario.economy.is_none() || scenario.scheduled_events.is_none())
            {
                issues.push(ValidationIssue::new(
                    "state",
                    "missing_e3_configuration",
                    "rules version 3 state requires economy and scheduled-event configuration",
                ));
            }
            if let Some(territory) = &scenario.territory {
                let year = state.current_year.unwrap_or(state.start_year);
                let mut living_counts: BTreeMap<SettlementId, u64> = BTreeMap::new();
                for person in state.living_people_at(year) {
                    *living_counts.entry(person.settlement_id).or_default() += 1;
                    if let Some(profession) = &person.profession {
                        let offered = territory
                            .profession_options_by_settlement
                            .get(&person.settlement_id)
                            .is_some_and(|options| options.contains(profession));
                        if !territory
                            .professions
                            .iter()
                            .any(|definition| &definition.id == profession)
                            || !offered
                        {
                            issues.push(ValidationIssue::new(
                                format!("persons[{}].profession", person.id.0),
                                "profession_unavailable",
                                "current profession must be cataloged and offered in the person's settlement",
                            ));
                        }
                    }
                }
                for (settlement_id, capacity) in &territory.settlement_capacities {
                    if living_counts
                        .get(settlement_id)
                        .copied()
                        .unwrap_or_default()
                        > u64::from(*capacity)
                    {
                        issues.push(ValidationIssue::new(
                            format!("settlements[{}]", settlement_id.0),
                            "settlement_over_capacity",
                            "living population exceeds settlement capacity",
                        ));
                    }
                }
            }
        }
    }
    let active_person_ids: BTreeSet<_> = state.persons.keys().copied().collect();
    let active_settlement_ids: BTreeSet<_> = state.settlements.keys().copied().collect();
    let mut active_profession_ids: BTreeSet<_> = state
        .persons
        .values()
        .filter_map(|person| person.profession.clone())
        .collect();
    if scenario.is_none() {
        active_profession_ids.extend(state.event_effects.iter().flat_map(|effect| {
            effect
                .profession_ids
                .iter()
                .flat_map(|professions| professions.iter().cloned())
        }));
    }
    let mut known_routes: BTreeSet<_> = scenario
        .into_iter()
        .flat_map(|scenario| scenario.territory.iter())
        .flat_map(|territory| territory.routes.iter().map(|route| route.logical_id))
        .collect();
    if scenario.is_none() {
        known_routes.extend(
            state
                .event_effects
                .iter()
                .flat_map(|effect| effect.route_ids.iter().copied()),
        );
    }
    if let Some(scenario) = scenario {
        active_profession_ids.extend(scenario.territory.iter().flat_map(|territory| {
            territory
                .professions
                .iter()
                .map(|profession| profession.id.clone())
        }));
    }
    issues.extend(validate_active_event_effects(
        &state.event_effects,
        state.current_year,
        &active_person_ids,
        &active_settlement_ids,
        &active_profession_ids,
        &known_routes,
    ));
    if let Some(scenario) = scenario {
        issues.extend(validate_active_event_links(
            &state.event_effects,
            scenario.scheduled_events.as_deref().unwrap_or_default(),
        ));
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

fn validate_residence_history(state: &State, issues: &mut Vec<ValidationIssue>) {
    let at_year = state.current_year.unwrap_or(state.start_year);
    for (person_id, intervals) in &state.residences {
        let path = format!("residences[{}]", person_id.0);
        if !state.persons.contains_key(person_id) {
            issues.push(ValidationIssue::new(
                &path,
                "unknown_reference",
                "residence history references an unknown person",
            ));
        }
        if intervals.is_empty() {
            issues.push(ValidationIssue::new(
                &path,
                "empty_history",
                "residence history cannot be stored as an empty list",
            ));
        }
        let mut previous_end = None;
        let mut ordered = intervals.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|interval| interval.validity.start);
        for (index, interval) in ordered.iter().enumerate() {
            if !interval.validity.is_valid() {
                issues.push(ValidationIssue::new(
                    format!("{path}[{index}].validity"),
                    "invalid_interval",
                    "residence interval end cannot precede its start",
                ));
            }
            if !state.settlements.contains_key(&interval.settlement_id) {
                issues.push(ValidationIssue::new(
                    format!("{path}[{index}].settlement_id"),
                    "unknown_reference",
                    "residence interval references an unknown settlement",
                ));
            }
            if interval.validity.start > at_year
                || interval
                    .validity
                    .end_exclusive
                    .is_some_and(|end| end > at_year)
            {
                issues.push(ValidationIssue::new(
                    format!("{path}[{index}].validity"),
                    "future_interval",
                    "closed residence history cannot contain future dates",
                ));
            }
            if index > 0 && previous_end.is_none_or(|end| end > interval.validity.start) {
                issues.push(ValidationIssue::new(
                    format!("{path}[{index}].validity"),
                    "overlapping_intervals",
                    "a person cannot reside in two places at once",
                ));
            }
            previous_end = interval.validity.end_exclusive;
        }
        if let Some(person) = state.persons.get(person_id) {
            if person.alive_at_close(at_year) {
                let active = intervals
                    .iter()
                    .filter(|interval| interval.validity.contains(at_year))
                    .collect::<Vec<_>>();
                if active.len() != 1 || active[0].settlement_id != person.settlement_id {
                    issues.push(ValidationIssue::new(
                        &path,
                        "current_residence_mismatch",
                        "a living person's active residence must match their current settlement",
                    ));
                }
            }
        }
    }
    if state.rules_version >= Scenario::TERRITORIAL_RULES_VERSION {
        for person in state.living_people_at(at_year) {
            if !state.residences.contains_key(&person.id) {
                issues.push(ValidationIssue::new(
                    format!("residences[{}]", person.id.0),
                    "missing_history",
                    "every living rules version 2 resident must have a residence history",
                ));
            }
        }
    }
}

fn validate_profession_history(
    state: &State,
    _scenario: Option<&Scenario>,
    issues: &mut Vec<ValidationIssue>,
) {
    let at_year = state.current_year.unwrap_or(state.start_year);
    for (person_id, intervals) in &state.profession_intervals {
        let path = format!("profession_intervals[{}]", person_id.0);
        let person = state.persons.get(person_id);
        if person.is_none() {
            issues.push(ValidationIssue::new(
                &path,
                "unknown_reference",
                "profession history references an unknown person",
            ));
        }
        if intervals.is_empty() {
            issues.push(ValidationIssue::new(
                &path,
                "empty_history",
                "profession history cannot be stored as an empty list",
            ));
        }
        let mut ordered = intervals.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|interval| interval.validity.start);
        let mut previous_end = None;
        for (index, interval) in ordered.iter().enumerate() {
            if !interval.validity.is_valid() {
                issues.push(ValidationIssue::new(
                    format!("{path}[{index}].validity"),
                    "invalid_interval",
                    "profession interval end cannot precede its start",
                ));
            }
            if interval.validity.start > at_year
                || interval
                    .validity
                    .end_exclusive
                    .is_some_and(|end| end > at_year)
            {
                issues.push(ValidationIssue::new(
                    format!("{path}[{index}].validity"),
                    "future_interval",
                    "closed profession history cannot contain future dates",
                ));
            }
            if index > 0 && previous_end.is_none_or(|end| end > interval.validity.start) {
                issues.push(ValidationIssue::new(
                    format!("{path}[{index}].validity"),
                    "overlapping_intervals",
                    "a person cannot hold two professions at once",
                ));
            }
            previous_end = interval.validity.end_exclusive;
        }
    }
    if state.rules_version >= Scenario::TERRITORIAL_RULES_VERSION {
        for person in state.living_people_at(at_year) {
            let active = state
                .profession_intervals
                .get(&person.id)
                .into_iter()
                .flatten()
                .filter(|interval| interval.validity.contains(at_year))
                .map(|interval| &interval.profession_id)
                .collect::<Vec<_>>();
            match (&person.profession, active.as_slice()) {
                (Some(profession), [active]) if profession == *active => {}
                (None, []) => {}
                _ => issues.push(ValidationIssue::new(
                    format!("persons[{}].profession", person.id.0),
                    "profession_history_mismatch",
                    "active profession history must match the person's current profession",
                )),
            }
        }
    }
}

fn validate_settlement_balances(state: &State, issues: &mut Vec<ValidationIssue>) {
    for (settlement_id, balance) in &state.last_year_settlement_balances {
        if *settlement_id != balance.settlement_id {
            issues.push(ValidationIssue::new(
                "last_year_settlement_balances",
                "map_key_mismatch",
                "settlement balance key must match settlement ID",
            ));
        }
        if !state.settlements.contains_key(settlement_id) {
            issues.push(ValidationIssue::new(
                format!("last_year_settlement_balances[{}]", settlement_id.0),
                "unknown_reference",
                "settlement balance references an unknown settlement",
            ));
        }
        let expected = balance
            .before
            .checked_add(balance.births)
            .and_then(|value| value.checked_sub(balance.deaths))
            .and_then(|value| value.checked_add(balance.incoming))
            .and_then(|value| value.checked_sub(balance.outgoing));
        if expected != Some(balance.after) {
            issues.push(ValidationIssue::new(
                format!("last_year_settlement_balances[{}]", settlement_id.0),
                "balance_mismatch",
                "before + births - deaths + incoming - outgoing must equal after",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_scenario() -> Scenario {
        let place = SettlementId(1);
        let household = HouseholdId(1);
        let trait_key = TraitKey::new("temperament", "curiosity");
        let p1 = Person {
            id: PersonId(1),
            logical_id: LogicalId(11),
            birth_year: Year(0),
            death_year: None,
            settlement_id: place,
            household_id: Some(household),
            reproductive: ReproductiveProfile::default(),
            traits: [(trait_key.clone(), 3)].into_iter().collect(),
            genome: Genome::default(),
            last_widowed_year: None,
            profession: None,
        };
        let p2 = Person {
            id: PersonId(2),
            logical_id: LogicalId(12),
            birth_year: Year(0),
            death_year: None,
            settlement_id: place,
            household_id: Some(household),
            reproductive: ReproductiveProfile::default(),
            traits: BTreeMap::new(),
            genome: Genome::default(),
            last_widowed_year: None,
            profession: None,
        };
        Scenario {
            world_id: WorldId(1),
            schema_version: 1,
            rules_version: 1,
            seed: 7,
            start_year: Year(10),
            settlements: vec![Settlement {
                id: place,
                logical_id: LogicalId(1),
                name: "A".into(),
                culture_values: [(trait_key.clone(), 2)].into_iter().collect(),
            }],
            initial_people: vec![p1, p2],
            initial_households: vec![Household {
                id: household,
                logical_id: LogicalId(2),
                kind: HouseholdKind::Family,
                settlement_id: place,
                members: [PersonId(1), PersonId(2)].into_iter().collect(),
                closed_year: None,
            }],
            initial_unions: Vec::new(),
            initial_family_links: Vec::new(),
            trait_catalog: vec![TraitDefinition {
                key: trait_key,
                minimum: 0,
                maximum: 5,
                unit: Some("points".into()),
                description: Some("Curiosity".into()),
            }],
            initial_population_profiles: Vec::new(),
            policies: DemographicPolicies::default(),
            genetics: GeneticCatalog::default(),
            territory: None,
            culture: None,
            economy: None,
            scheduled_events: None,
        }
    }

    #[test]
    fn validates_a_consistent_scenario_and_creates_serializable_state() {
        let scenario = tiny_scenario();
        let scenario_json = serde_json::to_string(&scenario).unwrap();
        let decoded_scenario: Scenario = serde_json::from_str(&scenario_json).unwrap();
        assert_eq!(decoded_scenario, scenario);
        let validated = scenario.validate().unwrap();
        let state = validated.initial_state();
        assert_eq!(state.current_year, None);
        assert_eq!(state.persons.len(), 2);
        let json = serde_json::to_string(&state).unwrap();
        let decoded: State = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, state);

        let mut legacy_state_json = serde_json::to_value(&state).unwrap();
        legacy_state_json
            .as_object_mut()
            .unwrap()
            .remove("last_gestational_birth_year");
        assert_eq!(
            serde_json::from_value::<State>(legacy_state_json)
                .unwrap()
                .last_gestational_birth_year,
            BTreeMap::new()
        );

        let mut invalid_state = state;
        invalid_state
            .last_gestational_birth_year
            .insert(PersonId(1), Year(10));
        assert!(invalid_state
            .validate()
            .unwrap_err()
            .iter()
            .any(|issue| issue.path == "last_gestational_birth_year[1]"
                && issue.code == "future_birth"));
    }

    #[test]
    fn rejects_unknown_scenario_and_policy_fields_instead_of_ignoring_them() {
        let mut value = serde_json::to_value(tiny_scenario()).unwrap();
        value["future_option"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Scenario>(value).is_err());

        let mut value = serde_json::to_value(tiny_scenario()).unwrap();
        value["policies"]["births"]
            .as_object_mut()
            .unwrap()
            .remove("minimum_years_between_births");
        assert_eq!(
            serde_json::from_value::<Scenario>(value)
                .unwrap()
                .policies
                .births
                .minimum_years_between_births,
            0
        );

        let mut value = serde_json::to_value(tiny_scenario()).unwrap();
        value["policies"]["mortality"]["invented_rate"] = serde_json::json!(42);
        assert!(serde_json::from_value::<Scenario>(value).is_err());
    }

    #[test]
    fn logical_ids_round_trip_as_decimal_text_without_losing_precision() {
        let id = LogicalId(9_007_199_254_740_993);
        let encoded = serde_json::to_string(&id).unwrap();
        assert_eq!(encoded, "\"9007199254740993\"");
        assert_eq!(serde_json::from_str::<LogicalId>(&encoded).unwrap(), id);
        assert_eq!(
            serde_json::from_str::<LogicalId>("9007199254740993").unwrap(),
            id
        );
    }

    #[test]
    fn same_year_end_is_valid_for_a_link_created_and_closed_in_one_year() {
        assert!(YearInterval {
            start: Year(20),
            end_exclusive: Some(Year(20))
        }
        .is_valid());
    }

    #[test]
    fn genome_expression_supports_dominant_and_additive_catalogs() {
        let amber = VariantId::from("amber");
        let blue = VariantId::from("blue");
        let high = VariantId::from("high");
        let eye = LocusId::from("eye");
        let stature = LocusId::from("stature");
        let catalog = GeneticCatalog {
            loci: vec![
                GeneticLocus {
                    id: eye.clone(),
                    variants: [amber.clone(), blue.clone()].into_iter().collect(),
                    founder_distribution: vec![],
                    expression: GeneticExpression::Dominant {
                        dominant_variants: [amber.clone()].into_iter().collect(),
                        dominant_value: 1,
                        recessive_value: 0,
                    },
                },
                GeneticLocus {
                    id: stature.clone(),
                    variants: [high.clone(), blue.clone()].into_iter().collect(),
                    founder_distribution: vec![],
                    expression: GeneticExpression::Additive {
                        values: [(high.clone(), 3), (blue.clone(), -1)]
                            .into_iter()
                            .collect(),
                    },
                },
            ],
        };
        let genome = Genome {
            loci: [
                (
                    eye.clone(),
                    [
                        Allele {
                            variant: amber,
                            lineage: None,
                            inherited_from: None,
                        },
                        Allele {
                            variant: blue.clone(),
                            lineage: None,
                            inherited_from: None,
                        },
                    ],
                ),
                (
                    stature.clone(),
                    [
                        Allele {
                            variant: high,
                            lineage: None,
                            inherited_from: None,
                        },
                        Allele {
                            variant: blue,
                            lineage: None,
                            inherited_from: None,
                        },
                    ],
                ),
            ]
            .into_iter()
            .collect(),
        };
        let expression = genome.express(&catalog).unwrap();
        assert_eq!(expression[&eye], 1);
        assert_eq!(expression[&stature], 2);
    }

    #[test]
    fn checked_integer_domain_values_detect_overflow_and_negative_stock() {
        assert_eq!(Money(i64::MAX).checked_add(Money(1)), None);
        assert_eq!(Quantity(4).checked_sub(Quantity(7)), Some(Quantity(-3)));
        assert!(!Quantity(-3).is_non_negative());
    }

    #[test]
    fn rejects_missing_household_member_back_reference_and_future_birth() {
        let mut scenario = tiny_scenario();
        scenario.initial_people[0].birth_year = Year(11);
        scenario.initial_households[0].members.remove(&PersonId(2));
        let errors = scenario.validate().unwrap_err();
        assert!(errors.iter().any(|issue| issue.code == "future_birth"));
        assert!(errors
            .iter()
            .any(|issue| issue.code == "membership_mismatch"));
    }

    #[test]
    fn unknown_parent_is_an_empty_recorded_family_view() {
        let state = tiny_scenario().validate().unwrap().initial_state();
        assert_eq!(state.family_links_at(PersonId(1), Year(10)).count(), 0);
    }

    #[test]
    fn rejects_cyclic_genealogy_and_duplicate_ids() {
        let mut scenario = tiny_scenario();
        scenario.initial_family_links = vec![
            FamilyLink {
                id: FamilyLinkId(1),
                logical_id: LogicalId(31),
                parent: PersonId(1),
                child: PersonId(2),
                kind: FamilyLinkKind::Caregiver,
                validity: YearInterval {
                    start: Year(10),
                    end_exclusive: None,
                },
            },
            FamilyLink {
                id: FamilyLinkId(2),
                logical_id: LogicalId(32),
                parent: PersonId(2),
                child: PersonId(1),
                kind: FamilyLinkKind::Caregiver,
                validity: YearInterval {
                    start: Year(10),
                    end_exclusive: None,
                },
            },
        ];
        let errors = scenario.validate().unwrap_err();
        assert!(errors.iter().any(|issue| issue.code == "family_cycle"));
        scenario.initial_people[1].id = PersonId(1);
        assert!(scenario
            .validate()
            .unwrap_err()
            .iter()
            .any(|issue| issue.code == "duplicate_id"));
    }
}
