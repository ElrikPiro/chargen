//! Presentation-only projections for historical person, household and settlement views.
//!
//! Names and ornamental descriptors are derived from stable presentation keys. They do not
//! mutate simulation state and never use the engine's random stream.

use crate::{
    person_record, visible_interval, GenealogyRelation, HistoryQuery, PersonRecord,
    ProfessionAtClose, QueryError, QueryResult, SettlementCultureAtClose, SettlementId,
    SettlementView, State,
};
use chargen_domain::{
    Allele, FamilyLink, FamilyLinkKind, GeneticExpression, HistoricalEvent, HouseholdId,
    HouseholdKind, InstitutionId, LocusId, LogicalId, OrganizationId, Person, PersonId, TraitKey,
    UnionId, UnionKind, Year, YearInterval,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Mutex;

pub const PRESENTATION_TEMPLATE_VERSION: u32 = 1;
pub const PRESENTATION_PAGE_LIMIT: usize = 100;
const DEFAULT_PRESENTATION_CACHE_CAPACITY: usize = 256;
const MAX_GENEALOGY_DEPTH: usize = 64;
const MAX_GENEALOGY_RELATIONS: usize = 10_000;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PersonVisibility {
    Alive,
    Deceased {
        death_year: Year,
    },
    /// The person is absent from the selected historical close. This deliberately does not
    /// reveal whether they exist in a later close.
    NotKnownAtYear,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonPresentation {
    pub name: String,
    pub descriptive_traits: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonPresentationRow {
    pub person_id: PersonId,
    pub presentation: PersonPresentation,
    pub status: PersonVisibility,
    pub age_at_close: Option<u32>,
    pub age_at_death: Option<u32>,
    pub settlement_id: SettlementId,
    pub household_id: Option<HouseholdId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonTraitFact {
    pub key: TraitKey,
    pub value: i16,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResidencePresentation {
    pub settlement_id: SettlementId,
    pub settlement_name: Option<String>,
    pub validity: YearInterval,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstitutionAffiliationPresentation {
    pub institution_id: InstitutionId,
    pub institution_name: Option<String>,
    pub settlement_id: SettlementId,
    pub settlement_name: Option<String>,
    pub validity: YearInterval,
    pub active_at_close: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrganizationAffiliationPresentation {
    pub organization_id: OrganizationId,
    pub organization_name: String,
    pub settlement_id: SettlementId,
    pub settlement_name: Option<String>,
    pub founder: bool,
    pub validity: YearInterval,
    pub active_at_close: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonGenomeLocus {
    pub locus_id: LocusId,
    pub alleles: [Allele; 2],
    pub expressed_value: Option<i32>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FamilyLinkDirection {
    Parent,
    Child,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FamilyLinkPresentation {
    pub direction: FamilyLinkDirection,
    pub relative: PersonPresentationRow,
    pub link: FamilyLink,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChronologyEntry {
    pub year: Year,
    pub order: u32,
    pub event_id: LogicalId,
    pub text_es: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonDossier {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub template_version: u32,
    /// Fingerprint of this visible close projection, not a certificate for all historical events.
    pub fact_fingerprint: Option<String>,
    pub person_id: PersonId,
    pub status: PersonVisibility,
    pub record: Option<PersonRecord>,
    pub presentation: Option<PersonPresentation>,
    pub birth_year: Option<Year>,
    /// Set only when the death had happened by this close.
    pub death_year: Option<Year>,
    pub age_at_close: Option<u32>,
    pub age_at_death: Option<u32>,
    pub settlement_id: Option<SettlementId>,
    pub settlement_name: Option<String>,
    pub household_id: Option<HouseholdId>,
    pub profession: Option<chargen_domain::ProfessionId>,
    pub professions: Vec<ProfessionAtClose>,
    pub traits: Vec<PersonTraitFact>,
    pub genome: Vec<PersonGenomeLocus>,
    pub residences: Vec<ResidencePresentation>,
    pub institution_affiliations: Vec<InstitutionAffiliationPresentation>,
    pub organization_affiliations: Vec<OrganizationAffiliationPresentation>,
    pub family_links: Vec<FamilyLinkPresentation>,
    /// Number of visible direct family links before dossier pagination.
    pub family_links_total: usize,
    /// True when direct links remain available through `genealogy_page`.
    pub family_links_truncated: bool,
    pub unions: Vec<PersonUnionAtClose>,
    pub unions_total: usize,
    pub unions_truncated: bool,
    pub timeline: Vec<ChronologyEntry>,
    pub timeline_total: usize,
    pub timeline_truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonEventsPage {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub person_id: PersonId,
    pub total: usize,
    pub offset: usize,
    pub events: Vec<HistoricalEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HouseholdRow {
    pub household_id: HouseholdId,
    pub kind: HouseholdKind,
    pub settlement_id: SettlementId,
    pub active_at_close: bool,
    pub member_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HouseholdsPage {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub total: usize,
    pub offset: usize,
    pub households: Vec<HouseholdRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HouseholdDossier {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub household_id: HouseholdId,
    pub kind: HouseholdKind,
    pub settlement_id: SettlementId,
    pub active_at_close: bool,
    pub closed_year: Option<Year>,
    pub total: usize,
    pub offset: usize,
    pub members: Vec<PersonPresentationRow>,
}

/// Compatibility alias used by API layers that name all close-specific views uniformly.
pub type HouseholdAtClose = HouseholdDossier;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettlementDossier {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub settlement: SettlementView,
    pub culture: Option<SettlementCultureAtClose>,
    pub household_count: usize,
    pub total: usize,
    pub offset: usize,
    pub people: Vec<PersonPresentationRow>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GenealogyDirection {
    Ancestors,
    Descendants,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonUnionAtClose {
    pub union_id: UnionId,
    pub logical_id: LogicalId,
    pub kind: UnionKind,
    pub validity: YearInterval,
    pub active_at_close: bool,
    pub participants: Vec<PersonPresentationRow>,
    pub participants_total: usize,
    pub participants_truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonUnionsPage {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub person_id: PersonId,
    pub offset: usize,
    pub total: usize,
    pub truncated: bool,
    pub unions: Vec<PersonUnionAtClose>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnionParticipantsPage {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub union_id: UnionId,
    pub offset: usize,
    pub total: usize,
    pub truncated: bool,
    pub participants: Vec<PersonPresentationRow>,
}

/// A paginated genealogy row with a presentation-only name, keeping the legacy relation shape
/// untouched for the original unpaged genealogy query.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenealogyPageRelation {
    pub relative: PersonRecord,
    pub presentation: PersonPresentation,
    pub link: FamilyLink,
    pub distance: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenealogyPage {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub subject: PersonRecord,
    pub direction: GenealogyDirection,
    /// An empty vector means all recorded relation types.
    pub relation_kinds: Vec<FamilyLinkKind>,
    pub maximum_depth: usize,
    pub offset: usize,
    pub total: usize,
    pub truncated: bool,
    pub relations: Vec<GenealogyPageRelation>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct PresentationCacheKey {
    world_id: String,
    branch_id: String,
    year: Year,
    person_logical_id: LogicalId,
    template_version: u32,
    fact_fingerprint: u64,
}

struct PersonFactFingerprint<'a> {
    person_id: PersonId,
    projected_person: &'a Person,
    first_events_page: &'a PersonEventsPage,
    settlement_name: Option<&'a str>,
    residences: &'a [ResidencePresentation],
    institution_affiliations: &'a [InstitutionAffiliationPresentation],
    organization_affiliations: &'a [OrganizationAffiliationPresentation],
    unions: &'a PersonUnionsPage,
    genome: &'a [PersonGenomeLocus],
}

/// Bounded LRU cache containing presentation values only. A service may share one cache
/// between query contexts; world, branch and year are always part of each key.
#[derive(Debug)]
pub struct PresentationCache {
    capacity: usize,
    entries: Mutex<VecDeque<(PresentationCacheKey, PersonPresentation)>>,
}

impl Default for PresentationCache {
    fn default() -> Self {
        Self::new(DEFAULT_PRESENTATION_CACHE_CAPACITY)
    }
}

impl PresentationCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: Mutex::new(VecDeque::new()),
        }
    }

    fn get(&self, key: &PresentationCacheKey) -> Option<PersonPresentation> {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let index = entries.iter().position(|(candidate, _)| candidate == key)?;
        let entry = entries.remove(index)?;
        let presentation = entry.1.clone();
        entries.push_back(entry);
        Some(presentation)
    }

    fn insert(&self, key: PresentationCacheKey, value: PersonPresentation) {
        if self.capacity == 0 {
            return;
        }
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(index) = entries.iter().position(|(candidate, _)| candidate == &key) {
            entries.remove(index);
        }
        while entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back((key, value));
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

impl HistoryQuery {
    /// Historical person projection. An ID absent at this close produces an empty, neutral
    /// projection and never checks a later year to infer whether the person was born.
    pub fn person_dossier(&self, person_id: PersonId) -> QueryResult<PersonDossier> {
        let Some(person) = self
            .state
            .persons
            .get(&person_id)
            .filter(|person| person.birth_year <= self.year)
        else {
            return Ok(PersonDossier {
                world_id: self.world_id.clone(),
                branch_id: self.branch_id.clone(),
                year: self.year,
                template_version: PRESENTATION_TEMPLATE_VERSION,
                fact_fingerprint: None,
                person_id,
                status: PersonVisibility::NotKnownAtYear,
                record: None,
                presentation: None,
                birth_year: None,
                death_year: None,
                age_at_close: None,
                age_at_death: None,
                settlement_id: None,
                settlement_name: None,
                household_id: None,
                profession: None,
                professions: Vec::new(),
                traits: Vec::new(),
                genome: Vec::new(),
                residences: Vec::new(),
                institution_affiliations: Vec::new(),
                organization_affiliations: Vec::new(),
                family_links: Vec::new(),
                family_links_total: 0,
                family_links_truncated: false,
                unions: Vec::new(),
                unions_total: 0,
                unions_truncated: false,
                timeline: Vec::new(),
                timeline_total: 0,
                timeline_truncated: false,
            });
        };

        let mut record = person_record(person, self.year);
        record.person.death_year = record
            .person
            .death_year
            .filter(|death_year| *death_year <= self.year);
        let death_year = person
            .death_year
            .filter(|death_year| *death_year <= self.year);
        let age_at_close = if record.alive_at_close {
            record.age_at_close
        } else {
            None
        };
        let age_at_death = death_year.and_then(|died| died.age_of(person.birth_year));
        let status = if record.alive_at_close {
            PersonVisibility::Alive
        } else {
            PersonVisibility::Deceased {
                death_year: death_year.unwrap_or(self.year),
            }
        };
        let events = self.person_events_page(person_id, 0, PRESENTATION_PAGE_LIMIT)?;
        let (links, family_links_total) = self.person_family_links(person_id);
        let settlement_name = self
            .state
            .settlements
            .get(&person.settlement_id)
            .map(|settlement| settlement.name.clone());
        let residences = self.person_residences(person_id);
        let institution_affiliations = self.person_institution_affiliations(person_id);
        let organization_affiliations = self.person_organization_affiliations(person_id);
        let union_page = self.person_unions_page(person_id, 0, PRESENTATION_PAGE_LIMIT)?;
        let genome = self.person_genome(person);
        let fact_fingerprint = self.fingerprint_person_facts(PersonFactFingerprint {
            person_id,
            projected_person: &record.person,
            first_events_page: &events,
            settlement_name: settlement_name.as_deref(),
            residences: &residences,
            institution_affiliations: &institution_affiliations,
            organization_affiliations: &organization_affiliations,
            unions: &union_page,
            genome: &genome,
        });
        let presentation = self.presentation_for(person, fact_fingerprint);
        let traits = person
            .traits
            .iter()
            .map(|(key, value)| PersonTraitFact {
                key: key.clone(),
                value: *value,
                description: self
                    .scenario
                    .trait_catalog
                    .iter()
                    .find(|definition| &definition.key == key)
                    .and_then(|definition| definition.description.clone()),
            })
            .collect();
        let timeline = events
            .events
            .iter()
            .filter_map(|event| chronology_entry(event, person_id))
            .collect::<Vec<_>>();
        let professions = self
            .state
            .profession_intervals
            .get(&person_id)
            .into_iter()
            .flatten()
            .filter(|interval| interval.validity.start <= self.year)
            .map(|interval| ProfessionAtClose {
                profession_id: interval.profession_id.clone(),
                profession_name: self
                    .scenario
                    .territory
                    .as_ref()
                    .into_iter()
                    .flat_map(|territory| territory.professions.iter())
                    .find(|definition| definition.id == interval.profession_id)
                    .map(|definition| definition.name.clone()),
                validity: visible_interval(interval.validity, self.year),
            })
            .collect();

        Ok(PersonDossier {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            template_version: PRESENTATION_TEMPLATE_VERSION,
            fact_fingerprint: Some(format!("fnv1a64:{fact_fingerprint:016x}")),
            person_id,
            status,
            record: Some(record),
            presentation: Some(presentation),
            birth_year: Some(person.birth_year),
            death_year,
            age_at_close,
            age_at_death,
            settlement_id: Some(person.settlement_id),
            settlement_name,
            household_id: person.household_id,
            profession: person.profession.clone(),
            professions,
            traits,
            genome,
            residences,
            institution_affiliations,
            organization_affiliations,
            family_links: links,
            family_links_total,
            family_links_truncated: family_links_total > PRESENTATION_PAGE_LIMIT,
            unions: union_page.unions,
            unions_total: union_page.total,
            unions_truncated: union_page.truncated,
            timeline,
            timeline_total: events.total,
            timeline_truncated: events.total > PRESENTATION_PAGE_LIMIT,
        })
    }

    /// Replace this view's cache with a service-shared bounded presentation cache.
    pub fn with_presentation_cache(mut self, cache: std::sync::Arc<PresentationCache>) -> Self {
        self.presentation_cache = cache;
        self
    }

    /// Bounded page of all realized historical events indexed to a known person through this
    /// branch/year cutoff. The older `events_for_person` query remains available unchanged.
    pub fn person_events_page(
        &self,
        person_id: PersonId,
        offset: usize,
        limit: usize,
    ) -> QueryResult<PersonEventsPage> {
        validate_presentation_page(offset, limit)?;
        if self.person(person_id).is_none() {
            return Err(QueryError::PersonNotFound(person_id.0));
        }
        let page = self.store.events_for_entity_page(
            &self.world_id,
            &self.branch_id,
            self.year,
            "person",
            &person_id.0.to_string(),
            offset,
            limit,
        )?;
        Ok(PersonEventsPage {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            person_id,
            total: page.total,
            offset,
            events: page.events,
        })
    }

    /// Return the unions known for this person at the selected historical close. Future
    /// interval ends are censored and participants born after the cutoff are omitted.
    pub fn person_unions_page(
        &self,
        person_id: PersonId,
        offset: usize,
        limit: usize,
    ) -> QueryResult<PersonUnionsPage> {
        validate_presentation_page(offset, limit)?;
        if self.person(person_id).is_none() {
            return Err(QueryError::PersonNotFound(person_id.0));
        }
        let unions = self.state.unions.values().filter(|union| {
            union.validity.start <= self.year && union.participants.contains(&person_id)
        });
        let total = unions.clone().count();
        let unions = unions
            .skip(offset)
            .take(limit)
            .map(|union| self.person_union_at_close(union))
            .collect::<Vec<_>>();
        Ok(PersonUnionsPage {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            person_id,
            offset,
            total,
            truncated: offset.saturating_add(unions.len()) < total,
            unions,
        })
    }

    /// Continue through the known participants of one union. Pagination is over participants
    /// visible at this year, not raw IDs, so an unborn participant cannot affect the total.
    pub fn union_participants_page(
        &self,
        union_id: UnionId,
        offset: usize,
        limit: usize,
    ) -> QueryResult<UnionParticipantsPage> {
        validate_presentation_page(offset, limit)?;
        let union = self
            .state
            .unions
            .get(&union_id)
            .filter(|union| union.validity.start <= self.year)
            .ok_or(QueryError::UnionNotFound(union_id.0))?;
        let known_participants = union.participants.iter().filter(|person_id| {
            self.state
                .persons
                .get(*person_id)
                .is_some_and(|person| person.birth_year <= self.year)
        });
        let total = known_participants.clone().count();
        let participants = known_participants
            .skip(offset)
            .take(limit)
            .filter_map(|person_id| self.presentation_row(*person_id))
            .collect::<Vec<_>>();
        Ok(UnionParticipantsPage {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            union_id,
            offset,
            total,
            truncated: offset.saturating_add(participants.len()) < total,
            participants,
        })
    }

    /// List historical households in stable ID order, with no member expansion.
    pub fn households(&self, offset: usize, limit: usize) -> QueryResult<HouseholdsPage> {
        self.households_for_settlement(None, offset, limit)
    }

    /// List households after applying an optional settlement filter and before pagination.
    pub fn households_for_settlement(
        &self,
        settlement_id: Option<SettlementId>,
        offset: usize,
        limit: usize,
    ) -> QueryResult<HouseholdsPage> {
        validate_presentation_page(offset, limit)?;
        let total = self
            .state
            .households
            .values()
            .filter(|household| {
                settlement_id.is_none_or(|settlement_id| household.settlement_id == settlement_id)
            })
            .count();
        let households = self
            .state
            .households
            .values()
            .filter(|household| {
                settlement_id.is_none_or(|settlement_id| household.settlement_id == settlement_id)
            })
            .skip(offset)
            .take(limit)
            .map(|household| {
                let member_count = household
                    .members
                    .iter()
                    .filter(|person_id| {
                        self.state
                            .persons
                            .get(*person_id)
                            .is_some_and(|person| person.birth_year <= self.year)
                    })
                    .count();
                HouseholdRow {
                    household_id: household.id,
                    kind: household.kind.clone(),
                    settlement_id: household.settlement_id,
                    active_at_close: household
                        .closed_year
                        .filter(|closed_year| *closed_year <= self.year)
                        .is_none(),
                    member_count,
                }
            })
            .collect::<Vec<_>>();
        Ok(HouseholdsPage {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            total,
            offset,
            households,
        })
    }

    /// Historical household projection with a bounded page of known members.
    pub fn household_dossier(
        &self,
        household_id: HouseholdId,
        offset: usize,
        limit: usize,
    ) -> QueryResult<HouseholdDossier> {
        validate_presentation_page(offset, limit)?;
        let household = self
            .state
            .households
            .get(&household_id)
            .ok_or(QueryError::HouseholdNotFound(household_id.0))?;
        let closed_year = household.closed_year.filter(|closed| *closed <= self.year);
        let total = household
            .members
            .iter()
            .filter(|person_id| {
                self.state
                    .persons
                    .get(*person_id)
                    .is_some_and(|person| person.birth_year <= self.year)
            })
            .count();
        let members = household
            .members
            .iter()
            .filter(|person_id| {
                self.state
                    .persons
                    .get(*person_id)
                    .is_some_and(|person| person.birth_year <= self.year)
            })
            .skip(offset)
            .take(limit)
            .filter_map(|person_id| self.presentation_row(*person_id))
            .collect();
        Ok(HouseholdDossier {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            household_id,
            kind: household.kind.clone(),
            settlement_id: household.settlement_id,
            active_at_close: closed_year.is_none(),
            closed_year,
            total,
            offset,
            members,
        })
    }

    /// Historical settlement projection plus census aggregates and one resident page.
    pub fn settlement_dossier(
        &self,
        settlement_id: SettlementId,
        offset: usize,
        limit: usize,
    ) -> QueryResult<SettlementDossier> {
        validate_presentation_page(offset, limit)?;
        let settlement = self
            .settlements()
            .settlements
            .into_iter()
            .find(|settlement| settlement.settlement_id == settlement_id)
            .ok_or(QueryError::SettlementNotFound(settlement_id.0))?;
        let total = self
            .state
            .persons
            .values()
            .filter(|person| {
                person.settlement_id == settlement_id && person.alive_at_close(self.year)
            })
            .count();
        let people = self
            .state
            .persons
            .values()
            .filter(|person| {
                person.settlement_id == settlement_id && person.alive_at_close(self.year)
            })
            .skip(offset)
            .take(limit)
            .filter_map(|person| self.presentation_row(person.id))
            .collect();
        let culture = self
            .culture(Some(settlement_id))
            .settlements
            .into_iter()
            .next();
        let household_count = self
            .state
            .households
            .values()
            .filter(|household| {
                household.settlement_id == settlement_id
                    && household
                        .closed_year
                        .filter(|closed| *closed <= self.year)
                        .is_none()
            })
            .count();
        Ok(SettlementDossier {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            settlement,
            culture,
            household_count,
            total,
            offset,
            people,
        })
    }

    /// Return one bounded page of relatives, preserving every distinct active link even when
    /// two or more link types connect the same pair of people.
    pub fn genealogy_page(
        &self,
        person_id: PersonId,
        direction: GenealogyDirection,
        relation_kinds: Vec<FamilyLinkKind>,
        maximum_depth: usize,
        offset: usize,
        limit: usize,
    ) -> QueryResult<GenealogyPage> {
        validate_presentation_page(offset, limit)?;
        let subject = self
            .person(person_id)
            .ok_or(QueryError::PersonNotFound(person_id.0))?;
        let max_depth = maximum_depth.min(MAX_GENEALOGY_DEPTH);
        let kinds = relation_kinds.iter().copied().collect::<BTreeSet<_>>();
        let links = self
            .state
            .family_links
            .values()
            .filter(|link| link.active_at_close(self.year))
            .filter(|link| kinds.is_empty() || kinds.contains(&link.kind))
            .filter(|link| {
                self.state
                    .persons
                    .get(&link.parent)
                    .is_some_and(|person| person.birth_year <= self.year)
                    && self
                        .state
                        .persons
                        .get(&link.child)
                        .is_some_and(|person| person.birth_year <= self.year)
            })
            .collect::<Vec<_>>();
        let upward = direction == GenealogyDirection::Ancestors;
        let (mut relations, traversal_truncated) =
            traverse_genealogy_page(&self.state, self.year, person_id, max_depth, &links, upward);
        relations.sort_by_key(|relation| {
            (
                relation.distance,
                relation.relative.person.id,
                relation.link.id,
            )
        });
        let total = relations.len();
        let relations = relations
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|relation| {
                let presentation = self
                    .presentation_row(relation.relative.person.id)
                    .expect("genealogy traversal only includes known people")
                    .presentation;
                GenealogyPageRelation {
                    relative: relation.relative,
                    presentation,
                    link: relation.link,
                    distance: relation.distance,
                }
            })
            .collect();
        Ok(GenealogyPage {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            subject,
            direction,
            relation_kinds,
            maximum_depth: max_depth,
            offset,
            total,
            truncated: traversal_truncated || maximum_depth > MAX_GENEALOGY_DEPTH,
            relations,
        })
    }

    /// Markdown export for a person using escaped user-visible text and facts from this close.
    pub fn export_person_markdown(&self, person_id: PersonId) -> QueryResult<String> {
        let dossier = self.person_dossier(person_id)?;
        Ok(render_person_markdown(&dossier))
    }

    pub fn export_household_markdown(&self, household_id: HouseholdId) -> QueryResult<String> {
        let dossier = self.household_dossier(household_id, 0, PRESENTATION_PAGE_LIMIT)?;
        Ok(render_household_markdown(&dossier))
    }

    /// Settlement exports include only the requested resident page so a large census cannot
    /// produce an unbounded response.
    pub fn export_settlement_markdown(
        &self,
        settlement_id: SettlementId,
        offset: usize,
        limit: usize,
    ) -> QueryResult<String> {
        let dossier = self.settlement_dossier(settlement_id, offset, limit)?;
        Ok(render_settlement_markdown(&dossier))
    }

    fn person_family_links(&self, person_id: PersonId) -> (Vec<FamilyLinkPresentation>, usize) {
        // Keep only references for the first page while counting all visible links. State uses
        // a BTreeMap, so this page has a stable link-ID order without expanding every relative.
        let mut selected = Vec::with_capacity(PRESENTATION_PAGE_LIMIT);
        let mut total = 0;
        for link in self.state.family_links.values() {
            if !link.active_at_close(self.year)
                || (link.parent != person_id && link.child != person_id)
            {
                continue;
            }
            let (direction, relative_id) = if link.child == person_id {
                (FamilyLinkDirection::Parent, link.parent)
            } else {
                (FamilyLinkDirection::Child, link.child)
            };
            if self
                .state
                .persons
                .get(&relative_id)
                .is_none_or(|relative| relative.birth_year > self.year)
            {
                continue;
            }
            total += 1;
            if selected.len() < PRESENTATION_PAGE_LIMIT {
                selected.push((link, direction, relative_id));
            }
        }
        let links = selected
            .into_iter()
            .filter_map(|(link, direction, relative_id)| {
                self.presentation_row(relative_id).map(|relative| {
                    let mut visible_link = link.clone();
                    visible_link.validity = visible_interval(visible_link.validity, self.year);
                    FamilyLinkPresentation {
                        direction,
                        relative,
                        link: visible_link,
                    }
                })
            })
            .collect();
        (links, total)
    }

    fn person_union_at_close(&self, union: &chargen_domain::Union) -> PersonUnionAtClose {
        let known_participants = union.participants.iter().filter(|person_id| {
            self.state
                .persons
                .get(*person_id)
                .is_some_and(|person| person.birth_year <= self.year)
        });
        let participants_total = known_participants.clone().count();
        let participants = known_participants
            .take(PRESENTATION_PAGE_LIMIT)
            .filter_map(|person_id| self.presentation_row(*person_id))
            .collect::<Vec<_>>();
        PersonUnionAtClose {
            union_id: union.id,
            logical_id: union.logical_id,
            kind: union.kind.clone(),
            validity: visible_interval(union.validity, self.year),
            active_at_close: union.active_at_close(self.year),
            participants,
            participants_total,
            participants_truncated: participants_total > PRESENTATION_PAGE_LIMIT,
        }
    }

    fn person_residences(&self, person_id: PersonId) -> Vec<ResidencePresentation> {
        self.state
            .residences
            .get(&person_id)
            .into_iter()
            .flatten()
            .filter(|interval| interval.validity.start <= self.year)
            .map(|interval| ResidencePresentation {
                settlement_id: interval.settlement_id,
                settlement_name: self
                    .state
                    .settlements
                    .get(&interval.settlement_id)
                    .map(|settlement| settlement.name.clone()),
                validity: visible_interval(interval.validity, self.year),
            })
            .collect()
    }

    fn person_institution_affiliations(
        &self,
        person_id: PersonId,
    ) -> Vec<InstitutionAffiliationPresentation> {
        self.state
            .culture
            .institution_memberships
            .iter()
            .filter(|membership| {
                membership.person_id == person_id && membership.validity.start <= self.year
            })
            .map(|membership| InstitutionAffiliationPresentation {
                institution_id: membership.institution_id.clone(),
                institution_name: self
                    .scenario
                    .culture
                    .as_ref()
                    .into_iter()
                    .flat_map(|culture| culture.institutions.iter())
                    .find(|definition| definition.id == membership.institution_id)
                    .map(|definition| definition.name.clone()),
                settlement_id: membership.settlement_id,
                settlement_name: self
                    .state
                    .settlements
                    .get(&membership.settlement_id)
                    .map(|settlement| settlement.name.clone()),
                validity: visible_interval(membership.validity, self.year),
                active_at_close: membership.active_at_close(self.year),
            })
            .collect()
    }

    fn person_organization_affiliations(
        &self,
        person_id: PersonId,
    ) -> Vec<OrganizationAffiliationPresentation> {
        self.state
            .culture
            .organizations
            .values()
            .filter(|organization| organization.founded_year <= self.year)
            .flat_map(|organization| {
                organization
                    .memberships
                    .iter()
                    .filter(move |membership| {
                        membership.person_id == person_id && membership.validity.start <= self.year
                    })
                    .map(move |membership| OrganizationAffiliationPresentation {
                        organization_id: organization.id,
                        organization_name: organization.name.clone(),
                        settlement_id: organization.settlement_id,
                        settlement_name: self
                            .state
                            .settlements
                            .get(&organization.settlement_id)
                            .map(|settlement| settlement.name.clone()),
                        founder: membership.founder,
                        validity: visible_interval(membership.validity, self.year),
                        active_at_close: organization.active_at_close(self.year)
                            && membership.validity.contains(self.year),
                    })
            })
            .collect()
    }

    fn person_genome(&self, person: &Person) -> Vec<PersonGenomeLocus> {
        self.scenario
            .genetics
            .loci
            .iter()
            .filter_map(|locus| {
                let alleles = person.genome.loci.get(&locus.id)?.clone();
                let expressed_value = if alleles
                    .iter()
                    .any(|allele| !locus.variants.contains(&allele.variant))
                {
                    None
                } else {
                    match &locus.expression {
                        GeneticExpression::Dominant {
                            dominant_variants,
                            dominant_value,
                            recessive_value,
                        } => Some(i32::from(
                            if alleles
                                .iter()
                                .any(|allele| dominant_variants.contains(&allele.variant))
                            {
                                *dominant_value
                            } else {
                                *recessive_value
                            },
                        )),
                        GeneticExpression::Additive { values } => Some(
                            i32::from(*values.get(&alleles[0].variant)?)
                                + i32::from(*values.get(&alleles[1].variant)?),
                        ),
                    }
                };
                Some(PersonGenomeLocus {
                    locus_id: locus.id.clone(),
                    alleles,
                    expressed_value,
                })
            })
            .collect()
    }

    fn fingerprint_person_facts(&self, facts: PersonFactFingerprint<'_>) -> u64 {
        let mut hasher = StableFingerprint::default();
        hasher.write_debug(&self.world_id);
        hasher.write_debug(&self.branch_id);
        hasher.write_debug(&self.year);
        hasher.write_debug(&PRESENTATION_TEMPLATE_VERSION);
        hasher.write_debug(&self.state.seed);
        hasher.write_debug(&self.state.rules_version);
        hasher.write_debug(&facts.projected_person);
        hasher.write_debug(&facts.settlement_name);
        hasher.write_debug(&facts.residences);
        hasher.write_debug(&facts.institution_affiliations);
        hasher.write_debug(&facts.organization_affiliations);
        hasher.write_debug(&facts.unions.total);
        hasher.write_debug(&facts.unions.offset);
        hasher.write_debug(&facts.unions.truncated);
        hasher.write_debug(&facts.unions.unions);
        hasher.write_debug(&facts.genome);

        let mut link_total = 0usize;
        for link in self.state.family_links.values() {
            if !link.active_at_close(self.year)
                || (link.parent != facts.person_id && link.child != facts.person_id)
            {
                continue;
            }
            let relative_id = if link.child == facts.person_id {
                link.parent
            } else {
                link.child
            };
            let Some(relative) = self
                .state
                .persons
                .get(&relative_id)
                .filter(|relative| relative.birth_year <= self.year)
            else {
                continue;
            };
            link_total += 1;
            let mut visible_link = link.clone();
            visible_link.validity = visible_interval(visible_link.validity, self.year);
            hasher.write_debug(&visible_link);
            write_person_presentation_row_facts(&mut hasher, relative, self.year);
        }
        hasher.write_debug(&link_total);

        if let Some(intervals) = self.state.profession_intervals.get(&facts.person_id) {
            for interval in intervals
                .iter()
                .filter(|interval| interval.validity.start <= self.year)
            {
                hasher.write_debug(&ProfessionAtClose {
                    profession_id: interval.profession_id.clone(),
                    profession_name: self
                        .scenario
                        .territory
                        .as_ref()
                        .into_iter()
                        .flat_map(|territory| territory.professions.iter())
                        .find(|definition| definition.id == interval.profession_id)
                        .map(|definition| definition.name.clone()),
                    validity: visible_interval(interval.validity, self.year),
                });
            }
        }
        for trait_fact in &facts.projected_person.traits {
            let description = self
                .scenario
                .trait_catalog
                .iter()
                .find(|definition| &definition.key == trait_fact.0)
                .and_then(|definition| definition.description.as_deref());
            hasher.write_debug(&trait_fact.0);
            hasher.write_debug(&trait_fact.1);
            hasher.write_debug(&description);
        }

        hasher.write_debug(&facts.first_events_page.total);
        for event in &facts.first_events_page.events {
            hasher.write_debug(event);
        }
        hasher.finish()
    }

    fn presentation_row(&self, person_id: PersonId) -> Option<PersonPresentationRow> {
        let person = self
            .state
            .persons
            .get(&person_id)
            .filter(|person| person.birth_year <= self.year)?;
        let status = if person.alive_at_close(self.year) {
            PersonVisibility::Alive
        } else {
            PersonVisibility::Deceased {
                death_year: person.death_year.unwrap_or(self.year),
            }
        };
        let projected_person = person_record(person, self.year).person;
        let presentation = self.presentation_for(
            person,
            fingerprint_person_facts(
                self.state.seed,
                self.state.rules_version,
                &projected_person,
                &[],
                &[],
            ),
        );
        Some(PersonPresentationRow {
            person_id,
            presentation,
            status,
            age_at_close: if person.alive_at_close(self.year) {
                person.age_at_close(self.year)
            } else {
                None
            },
            age_at_death: person
                .death_year
                .filter(|death_year| *death_year <= self.year)
                .and_then(|death_year| death_year.age_of(person.birth_year)),
            settlement_id: person.settlement_id,
            household_id: person.household_id,
        })
    }

    fn presentation_for(&self, person: &Person, fact_fingerprint: u64) -> PersonPresentation {
        let key = PresentationCacheKey {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            person_logical_id: person.logical_id,
            template_version: PRESENTATION_TEMPLATE_VERSION,
            fact_fingerprint,
        };
        if let Some(presentation) = self.presentation_cache.get(&key) {
            return presentation;
        }
        let presentation =
            derive_person_presentation(self.state.seed, &self.world_id, person.logical_id);
        self.presentation_cache.insert(key, presentation.clone());
        presentation
    }
}

fn validate_presentation_page(offset: usize, limit: usize) -> QueryResult<()> {
    if limit == 0 || limit > PRESENTATION_PAGE_LIMIT || offset > usize::MAX - limit {
        return Err(QueryError::InvalidPage);
    }
    Ok(())
}

fn derive_person_presentation(
    seed: u64,
    world_id: &str,
    logical_id: LogicalId,
) -> PersonPresentation {
    const GIVEN_NAMES: &[&str] = &[
        "Aldara", "Alonso", "Amara", "Andrés", "Beatriz", "Bruna", "Celia", "Damián", "Elena",
        "Emilio", "Inés", "Iria", "León", "Lucía", "Marina", "Nicolás", "Olalla", "Rafael",
        "Sabela", "Tomás", "Valeria", "Xoel", "Yara", "Zaira",
    ];
    const SURNAMES: &[&str] = &[
        "Aguirre", "Alba", "Benítez", "Campos", "Del Río", "Fuentes", "Gálvez", "Lago", "Mena",
        "Navarro", "Paz", "Ríos", "Salas", "Souto", "Vidal", "Varela",
    ];
    const DESCRIPTORS: &[&str] = &[
        "mirada serena",
        "gesto atento",
        "porte tranquilo",
        "sonrisa fácil",
        "voz pausada",
        "andar ligero",
        "semblante curioso",
        "presencia discreta",
    ];
    let base = stable_mix(seed, hash_bytes(world_id.as_bytes()), logical_id.0);
    let given = GIVEN_NAMES[(base % GIVEN_NAMES.len() as u64) as usize];
    let surname = SURNAMES[(stable_mix(base, 0x6e61_6d65, 1) % SURNAMES.len() as u64) as usize];
    let descriptor_a =
        DESCRIPTORS[(stable_mix(base, 0x6465_7363, 1) % DESCRIPTORS.len() as u64) as usize];
    let descriptor_b =
        DESCRIPTORS[(stable_mix(base, 0x6465_7363, 2) % DESCRIPTORS.len() as u64) as usize];
    let descriptive_traits = if descriptor_a == descriptor_b {
        vec![descriptor_a.to_owned()]
    } else {
        vec![descriptor_a.to_owned(), descriptor_b.to_owned()]
    };
    PersonPresentation {
        name: format!("{given} {surname}"),
        descriptive_traits,
    }
}

fn stable_mix(seed: u64, domain: u64, identity: u64) -> u64 {
    let mut value =
        seed ^ domain.rotate_left(23) ^ identity.rotate_right(11) ^ 0x9e37_79b9_7f4a_7c15;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[derive(Clone, Copy, Debug)]
struct StableFingerprint(u64);

impl Default for StableFingerprint {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl StableFingerprint {
    fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn write_debug<T: std::fmt::Debug>(&mut self, value: &T) {
        let rendered = format!("{value:?}");
        self.write_bytes(&(rendered.len() as u64).to_le_bytes());
        self.write_bytes(rendered.as_bytes());
    }

    fn finish(self) -> u64 {
        self.0
    }
}

fn fingerprint_person_facts(
    seed: u64,
    rules_version: u32,
    person: &Person,
    links: &[FamilyLinkPresentation],
    events: &[HistoricalEvent],
) -> u64 {
    let facts = format!("{seed}|{rules_version}|{person:?}|{links:?}|{events:?}");
    hash_bytes(facts.as_bytes())
}

fn write_person_presentation_row_facts(
    hasher: &mut StableFingerprint,
    person: &Person,
    year: Year,
) {
    hasher.write_debug(&person.id);
    hasher.write_debug(&person.logical_id);
    hasher.write_debug(&person.birth_year);
    let death_year = person.death_year.filter(|death_year| *death_year <= year);
    hasher.write_debug(&death_year);
    let alive_at_close = person.alive_at_close(year);
    hasher.write_debug(&alive_at_close);
    hasher.write_debug(&alive_at_close.then(|| person.age_at_close(year)).flatten());
    hasher.write_debug(&death_year.and_then(|death_year| death_year.age_of(person.birth_year)));
    hasher.write_debug(&person.settlement_id);
    hasher.write_debug(&person.household_id);
}

fn traverse_genealogy_page(
    state: &State,
    year: Year,
    subject: PersonId,
    maximum_depth: usize,
    links: &[&FamilyLink],
    upward: bool,
) -> (Vec<GenealogyRelation>, bool) {
    let mut output = Vec::new();
    let mut seen_people = BTreeSet::from([subject]);
    let mut seen_links = BTreeSet::new();
    let mut frontier = vec![subject];
    let mut adjacency = BTreeMap::<PersonId, Vec<&FamilyLink>>::new();
    for link in links {
        let source = if upward { link.child } else { link.parent };
        adjacency.entry(source).or_default().push(*link);
    }
    let mut truncated = false;
    for distance in 1..=maximum_depth {
        let mut next = Vec::new();
        for current in frontier {
            for link in adjacency.get(&current).into_iter().flatten() {
                let (from, to) = if upward {
                    (link.child, link.parent)
                } else {
                    (link.parent, link.child)
                };
                if from != current || !seen_links.insert(link.id) {
                    continue;
                }
                let Some(person) = state
                    .persons
                    .get(&to)
                    .filter(|person| person.birth_year <= year)
                else {
                    continue;
                };
                if output.len() >= MAX_GENEALOGY_RELATIONS {
                    return (output, true);
                }
                let mut visible_link = (*link).clone();
                visible_link.validity = visible_interval(visible_link.validity, year);
                output.push(GenealogyRelation {
                    relative: person_record(person, year),
                    link: visible_link,
                    distance,
                });
                if seen_people.insert(to) {
                    next.push(to);
                }
            }
        }
        if distance == maximum_depth && !next.is_empty() {
            truncated = next.iter().any(|person_id| {
                adjacency
                    .get(person_id)
                    .into_iter()
                    .flatten()
                    .any(|link| !seen_links.contains(&link.id))
            });
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    (output, truncated)
}

fn chronology_entry(event: &HistoricalEvent, person_id: PersonId) -> Option<ChronologyEntry> {
    use chargen_domain::HistoricalEventKind as Kind;
    let text_es = match &event.kind {
        Kind::PersonBorn {
            person_id: born, ..
        } if *born == person_id => "Nació.".to_owned(),
        Kind::PersonDied {
            person_id: died, ..
        } if *died == person_id => "Falleció.".to_owned(),
        Kind::UnionStarted {
            participants, kind, ..
        } if participants.contains(&person_id) => {
            format!("Inició una unión de tipo {}.", union_kind_es(kind))
        }
        Kind::FamilyLinkEstablished {
            parent,
            child,
            kind,
            ..
        } if *parent == person_id || *child == person_id => {
            format!(
                "Se estableció un vínculo de tipo {}.",
                family_link_kind_es(*kind)
            )
        }
        Kind::FamilyLinkEnded {
            parent,
            child,
            kind,
            ..
        } if *parent == person_id || *child == person_id => {
            format!("Terminó un vínculo de tipo {}.", family_link_kind_es(*kind))
        }
        Kind::HouseholdChanged {
            person_id: changed, ..
        } if *changed == person_id => "Cambió de hogar.".to_owned(),
        Kind::ResidentsMoved { person_ids, .. } if person_ids.contains(&person_id) => {
            "Se trasladó a otro asentamiento.".to_owned()
        }
        Kind::ProfessionChanged {
            person_id: changed, ..
        } if *changed == person_id => "Cambió de profesión.".to_owned(),
        Kind::InstitutionAffiliationStarted {
            person_id: member, ..
        } if *member == person_id => "Comenzó una afiliación institucional.".to_owned(),
        Kind::InstitutionAffiliationEnded {
            person_id: member, ..
        } if *member == person_id => "Terminó una afiliación institucional.".to_owned(),
        Kind::OrganizationFounded { founders, .. } if founders.contains(&person_id) => {
            "Participó en la fundación de una organización.".to_owned()
        }
        Kind::OrganizationMembershipStarted {
            person_id: member, ..
        } if *member == person_id => "Se incorporó a una organización.".to_owned(),
        Kind::OrganizationMembershipEnded {
            person_id: member, ..
        } if *member == person_id => "Dejó una organización.".to_owned(),
        Kind::ScheduledEventOccurred { name, .. }
            if event
                .affected_entities
                .contains(&chargen_domain::EntityRef::Person(person_id)) =>
        {
            format!("Ocurrió el evento programado: {name}.")
        }
        Kind::EventEffectRealized { event_name, .. }
            if event
                .affected_entities
                .contains(&chargen_domain::EntityRef::Person(person_id)) =>
        {
            format!("Se registraron efectos del evento: {event_name}.")
        }
        _ if event
            .affected_entities
            .contains(&chargen_domain::EntityRef::Person(person_id)) =>
        {
            "Se registró un hecho relacionado.".to_owned()
        }
        _ => return None,
    };
    Some(ChronologyEntry {
        year: event.year,
        order: event.order,
        event_id: event.id,
        text_es,
    })
}

fn union_kind_es(kind: &chargen_domain::UnionKind) -> &'static str {
    match kind {
        chargen_domain::UnionKind::Partnership => "pareja",
        chargen_domain::UnionKind::Marriage => "matrimonio",
        chargen_domain::UnionKind::Other(_) => "otro",
    }
}

fn family_link_kind_es(kind: FamilyLinkKind) -> &'static str {
    match kind {
        FamilyLinkKind::BiologicalParent => "parentesco biológico",
        FamilyLinkKind::AdoptiveParent => "adopción",
        FamilyLinkKind::Caregiver => "cuidado",
    }
}

/// Escape plain text before placing it in Markdown headings, lists or table cells.
pub fn escape_markdown(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if character == '\n' || character == '\r' || character == '\0' {
            if character == '\0' {
                escaped.push('�');
            } else {
                escaped.push(' ');
            }
        } else {
            if character.is_ascii_punctuation() {
                escaped.push('\\');
            }
            escaped.push(character);
        }
    }
    escaped
}

/// Escape text for insertion inside a quoted Mermaid node label.
pub fn escape_mermaid_label(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\n' | '\r' => escaped.push(' '),
            '&' => escaped.push_str("&amp;"),
            '"' => escaped.push_str("&quot;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '|' => escaped.push_str("&#124;"),
            '\\' => escaped.push_str("&#92;"),
            '\0' => escaped.push('�'),
            _ => escaped.push(character),
        }
    }
    escaped
}

pub fn render_person_markdown(dossier: &PersonDossier) -> String {
    let mut output = String::new();
    let title = dossier
        .presentation
        .as_ref()
        .map(|presentation| presentation.name.as_str())
        .unwrap_or("Persona no conocida en este año");
    output.push_str(&format!("# Ficha de {}\n\n", escape_markdown(title)));
    append_export_context(
        &mut output,
        &dossier.world_id,
        &dossier.branch_id,
        dossier.year,
    );
    if let Some(birth_year) = dossier.birth_year {
        output.push_str(&format!("Nacimiento registrado: {}.\n\n", birth_year.0));
    }
    match &dossier.status {
        PersonVisibility::Alive => {
            output.push_str("Estado: viva o vivo al cierre.\n\n");
            if let Some(age) = dossier.age_at_close {
                output.push_str(&format!("Edad al cierre: {age}.\n\n"));
            }
        }
        PersonVisibility::Deceased { death_year } => {
            if let Some(age) = dossier.age_at_death {
                output.push_str(&format!(
                    "Estado: falleció a los {age} años, en {}.\n\n",
                    death_year.0
                ));
            } else {
                output.push_str(&format!("Estado: falleció en {}.\n\n", death_year.0));
            }
        }
        PersonVisibility::NotKnownAtYear => {
            output.push_str("Estado: no consta en este corte temporal.\n\n")
        }
    }
    if let Some(presentation) = &dossier.presentation {
        if !presentation.descriptive_traits.is_empty() {
            output.push_str("Descripción de presentación (ornamental): ");
            output.push_str(
                &presentation
                    .descriptive_traits
                    .iter()
                    .map(|trait_text| escape_markdown(trait_text))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            output.push_str(".\n\n");
        }
    }
    if let Some(profession) = &dossier.profession {
        let label = dossier
            .professions
            .iter()
            .find(|entry| {
                entry.profession_id == *profession && entry.validity.contains(dossier.year)
            })
            .and_then(|entry| entry.profession_name.as_deref())
            .unwrap_or(&profession.0);
        output.push_str(&format!(
            "Profesión al cierre: {} (ID {}).\n\n",
            escape_markdown(label),
            escape_markdown(&profession.0),
        ));
    }
    if let Some(settlement_id) = dossier.settlement_id {
        let settlement = dossier
            .settlement_name
            .as_deref()
            .map(escape_markdown)
            .unwrap_or_else(|| "nombre no disponible".to_owned());
        output.push_str(&format!(
            "Asentamiento registrado al corte: {} (ID {}).\n",
            settlement, settlement_id.0
        ));
    }
    if let Some(household_id) = dossier.household_id {
        output.push_str(&format!(
            "Hogar registrado al corte: ID {}.\n",
            household_id.0
        ));
    }
    if dossier.settlement_id.is_some() || dossier.household_id.is_some() {
        output.push('\n');
    }
    if !dossier.residences.is_empty() {
        output.push_str("## Residencias registradas\n\n");
        for residence in &dossier.residences {
            let name = residence
                .settlement_name
                .as_deref()
                .map(escape_markdown)
                .unwrap_or_else(|| "nombre no disponible".to_owned());
            output.push_str(&format!(
                "- {} (ID {}): {} al cierre; {}.\n",
                name,
                residence.settlement_id.0,
                if residence.validity.contains(dossier.year) {
                    "vigente"
                } else {
                    "histórica"
                },
                format_interval_es(residence.validity),
            ));
        }
        output.push('\n');
    }
    if !dossier.professions.is_empty() {
        output.push_str("## Historial profesional\n\n");
        for profession in &dossier.professions {
            output.push_str(&format!(
                "- {} (ID {}) — {} al cierre; {}.\n",
                escape_markdown(
                    profession
                        .profession_name
                        .as_deref()
                        .unwrap_or(&profession.profession_id.0)
                ),
                escape_markdown(&profession.profession_id.0),
                if profession.validity.contains(dossier.year) {
                    "vigente"
                } else {
                    "histórica"
                },
                format_interval_es(profession.validity),
            ));
        }
        output.push('\n');
    }
    if !dossier.traits.is_empty() {
        output.push_str("## Rasgos registrados\n\n");
        for trait_fact in &dossier.traits {
            let label = trait_fact
                .description
                .as_deref()
                .unwrap_or(&trait_fact.key.name);
            output.push_str(&format!(
                "- {}: {}\n",
                escape_markdown(label),
                trait_fact.value
            ));
        }
        output.push('\n');
    }
    if !dossier.genome.is_empty() {
        output.push_str("## Genoma configurado\n\n");
        for locus in &dossier.genome {
            let first = escape_markdown(&locus.alleles[0].variant.0);
            let second = escape_markdown(&locus.alleles[1].variant.0);
            output.push_str(&format!(
                "- Locus {}: alelos {} / {}; expresión configurada: {}.\n",
                escape_markdown(&locus.locus_id.0),
                first,
                second,
                locus
                    .expressed_value
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "no disponible".to_owned()),
            ));
        }
        output.push('\n');
    }
    if !dossier.institution_affiliations.is_empty() {
        output.push_str("## Afiliaciones institucionales registradas\n\n");
        for affiliation in &dossier.institution_affiliations {
            let name = affiliation
                .institution_name
                .as_deref()
                .map(escape_markdown)
                .unwrap_or_else(|| "nombre no disponible".to_owned());
            let settlement = affiliation
                .settlement_name
                .as_deref()
                .map(escape_markdown)
                .unwrap_or_else(|| "nombre no disponible".to_owned());
            output.push_str(&format!(
                "- {} (ID {}), en {} (ID {}): {} al cierre; {}.\n",
                name,
                escape_markdown(&affiliation.institution_id.0),
                settlement,
                affiliation.settlement_id.0,
                if affiliation.active_at_close {
                    "activa"
                } else {
                    "histórica"
                },
                format_interval_es(affiliation.validity),
            ));
        }
        output.push('\n');
    }
    if !dossier.organization_affiliations.is_empty() {
        output.push_str("## Afiliaciones a organizaciones registradas\n\n");
        for affiliation in &dossier.organization_affiliations {
            let settlement = affiliation
                .settlement_name
                .as_deref()
                .map(escape_markdown)
                .unwrap_or_else(|| "nombre no disponible".to_owned());
            output.push_str(&format!(
                "- {} (ID {}), en {} (ID {}): {} al cierre; {}; {}.\n",
                escape_markdown(&affiliation.organization_name),
                affiliation.organization_id.0,
                settlement,
                affiliation.settlement_id.0,
                if affiliation.active_at_close {
                    "activa"
                } else {
                    "histórica"
                },
                if affiliation.founder {
                    "fundadora o fundador"
                } else {
                    "miembro"
                },
                format_interval_es(affiliation.validity),
            ));
        }
        output.push('\n');
    }
    if dossier.unions_total > 0 {
        output.push_str("## Uniones registradas\n\n");
        output.push_str(&format!(
            "Uniones mostradas: {} de {}.\n\n",
            dossier.unions.len(),
            dossier.unions_total
        ));
        for union in &dossier.unions {
            output.push_str(&format!(
                "### Unión {} — {}\n\nEstado al cierre: {}; {}.\n\nParticipantes conocidos mostrados: {} de {}.\n\n",
                union.union_id.0,
                escape_markdown(&union_kind_description(&union.kind)),
                if union.active_at_close { "vigente" } else { "histórica" },
                format_interval_es(union.validity),
                union.participants.len(),
                union.participants_total,
            ));
            if union.participants.is_empty() {
                output.push_str("No hay participantes conocidos en este corte.\n");
            } else {
                for participant in &union.participants {
                    output.push_str(&format!(
                        "- {} (ID {}): {}.\n",
                        escape_markdown(&participant.presentation.name),
                        participant.person_id.0,
                        status_text(&participant.status, participant.age_at_death),
                    ));
                }
            }
            if union.participants_truncated {
                output.push_str("Hay más participantes conocidos; consulta la página de integrantes de la unión.\n");
            }
            output.push('\n');
        }
        if dossier.unions_truncated {
            output.push_str("Hay más uniones; consulta las páginas siguientes.\n\n");
        }
    }
    if dossier.family_links_total > 0 {
        output.push_str("## Relaciones vigentes al cierre\n\n");
        output.push_str(&format!(
            "Se muestran {} de {} relaciones vigentes.\n\n",
            dossier.family_links.len(),
            dossier.family_links_total
        ));
        for relation in &dossier.family_links {
            let direction = match relation.direction {
                FamilyLinkDirection::Parent => "Progenitor o cuidador",
                FamilyLinkDirection::Child => "Descendiente o persona cuidada",
            };
            output.push_str(&format!(
                "- {}: {} ({})\n",
                escape_markdown(direction),
                escape_markdown(&relation.relative.presentation.name),
                escape_markdown(family_link_kind_es(relation.link.kind)),
            ));
        }
        if dossier.family_links_truncated {
            output.push_str("Hay más relaciones; consulta el árbol genealógico con paginación.\n");
        }
        output.push('\n');
    }
    output.push_str("## Cronología\n\n");
    if dossier.timeline.is_empty() {
        output.push_str("No hay hechos narrables registrados hasta este año.\n");
    } else {
        for entry in &dossier.timeline {
            output.push_str(&format!(
                "- Año {}: {}\n",
                entry.year.0,
                escape_markdown(&entry.text_es)
            ));
        }
    }
    if dossier.timeline_truncated {
        output.push_str(&format!(
            "\nSe muestran {} de {} hechos; consulta las páginas siguientes para continuar.\n",
            dossier.timeline.len(),
            dossier.timeline_total
        ));
    }
    output
}

fn format_interval_es(interval: YearInterval) -> String {
    match interval.end_exclusive {
        Some(end) => format!("desde {} hasta antes de {}", interval.start.0, end.0),
        None => format!("desde {}; fin no conocido en este corte", interval.start.0),
    }
}

fn union_kind_description(kind: &UnionKind) -> String {
    match kind {
        UnionKind::Partnership => "pareja".to_owned(),
        UnionKind::Marriage => "matrimonio".to_owned(),
        UnionKind::Other(label) => format!("otra unión ({label})"),
    }
}

pub fn render_household_markdown(dossier: &HouseholdDossier) -> String {
    let mut output = format!("# Hogar {}\n\n", dossier.household_id.0);
    append_export_context(
        &mut output,
        &dossier.world_id,
        &dossier.branch_id,
        dossier.year,
    );
    output.push_str(&format!(
        "Estado: {}.\n\n",
        if dossier.active_at_close {
            "activo"
        } else {
            "cerrado"
        }
    ));
    output.push_str(&format!(
        "Integrantes mostrados: {} de {}.\nPágina de integrantes: desplazamiento {} (base 0).\n\n",
        dossier.members.len(),
        dossier.total,
        dossier.offset,
    ));
    output.push_str("## Integrantes\n\n");
    if dossier.members.is_empty() {
        output.push_str("No hay integrantes conocidos en esta página.\n");
    } else {
        for member in &dossier.members {
            output.push_str(&format!(
                "- {} — {}\n",
                escape_markdown(&member.presentation.name),
                status_text(&member.status, member.age_at_death)
            ));
        }
    }
    output
}

pub fn render_settlement_markdown(dossier: &SettlementDossier) -> String {
    let mut output = format!(
        "# Asentamiento: {}\n\n",
        escape_markdown(&dossier.settlement.settlement_name),
    );
    append_export_context(
        &mut output,
        &dossier.world_id,
        &dossier.branch_id,
        dossier.year,
    );
    output.push_str(&format!(
        "Población viva: {}.\n\nResidentes mostrados: {} de {}.\nPágina de residentes: desplazamiento {} (base 0).\n\nHogares activos: {}.\n\n",
        dossier.settlement.population,
        dossier.people.len(),
        dossier.total,
        dossier.offset,
        dossier.household_count,
    ));
    output.push_str("## Residentes de esta página\n\n");
    if dossier.people.is_empty() {
        output.push_str("No hay residentes en esta página.\n");
    } else {
        for person in &dossier.people {
            if let Some(age) = person.age_at_close {
                output.push_str(&format!(
                    "- {} ({} años)\n",
                    escape_markdown(&person.presentation.name),
                    age
                ));
            } else {
                output.push_str(&format!(
                    "- {}\n",
                    escape_markdown(&person.presentation.name)
                ));
            }
        }
    }
    output
}

fn append_export_context(output: &mut String, world_id: &str, branch_id: &str, year: Year) {
    output.push_str(&format!(
        "Mundo: {}.\nRama: {}.\nAño consultado: {} (cierre histórico).\n\n",
        escape_markdown(world_id),
        escape_markdown(branch_id),
        year.0,
    ));
}

fn status_text(status: &PersonVisibility, age_at_death: Option<u32>) -> String {
    match status {
        PersonVisibility::Alive => "vive al cierre".to_owned(),
        PersonVisibility::Deceased { death_year } => age_at_death
            .map(|age| format!("falleció a los {age} años, en {}", death_year.0))
            .unwrap_or_else(|| format!("falleció en {}", death_year.0)),
        PersonVisibility::NotKnownAtYear => "no consta en este año".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_key(person_logical_id: u64) -> PresentationCacheKey {
        PresentationCacheKey {
            world_id: "cache-test-world".into(),
            branch_id: "main".into(),
            year: Year(3),
            person_logical_id: LogicalId(person_logical_id),
            template_version: PRESENTATION_TEMPLATE_VERSION,
            fact_fingerprint: person_logical_id,
        }
    }

    fn cached_name(name: &str) -> PersonPresentation {
        PersonPresentation {
            name: name.to_owned(),
            descriptive_traits: Vec::new(),
        }
    }

    #[test]
    fn future_interval_end_is_censored_at_the_selected_close() {
        let interval = YearInterval {
            start: Year(-5),
            end_exclusive: Some(Year(5)),
        };
        let projected = visible_interval(interval, Year(0));
        assert_eq!(projected.start, Year(-5));
        assert_eq!(projected.end_exclusive, None);
    }

    #[test]
    fn shared_presentation_cache_is_bounded_and_refreshes_recent_entries() {
        let cache = PresentationCache::new(2);
        let first = cache_key(1);
        let second = cache_key(2);
        let third = cache_key(3);
        cache.insert(first.clone(), cached_name("Primera"));
        cache.insert(second.clone(), cached_name("Segunda"));

        assert_eq!(cache.get(&first).unwrap().name, "Primera");
        cache.insert(third.clone(), cached_name("Tercera"));

        assert_eq!(cache.len(), 2);
        assert!(cache.get(&second).is_none());
        assert_eq!(cache.get(&first).unwrap().name, "Primera");
        assert_eq!(cache.get(&third).unwrap().name, "Tercera");
    }
}
