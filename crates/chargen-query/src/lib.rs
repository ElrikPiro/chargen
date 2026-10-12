//! Read-only projections pinned to one world, branch and confirmed year.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use chargen_domain::{
    CultureAggregate, EconomicGoodLedger, EconomicGroupLedger, EconomicMoneyLedger, EconomicTrade,
    FamilyLink, FamilyLinkKind, HistoricalEvent, InstitutionDefinition, InstitutionMembership,
    LogicalId, OrganizationId, Person, PersonId, Scenario, SettlementEconomicLedger, SettlementId,
    SettlementYearBalance, State, TraitKey, Year, YearInterval,
};
use chargen_store::{HistoryStore, StoreError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QueryError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("stored scenario is invalid JSON: {0}")]
    Scenario(#[from] serde_json::Error),
    #[error("person {0} does not exist in the selected historical state")]
    PersonNotFound(u64),
    #[error("organization {0} does not exist at the selected historical close")]
    OrganizationNotFound(u64),
    #[error("institution {0} does not exist at the selected historical close")]
    InstitutionNotFound(String),
    #[error("historical page must have a limit from 1 to 500")]
    InvalidPage,
}

pub type QueryResult<T> = Result<T, QueryError>;

fn validate_page(_offset: usize, limit: usize) -> QueryResult<()> {
    if limit == 0 || limit > 500 {
        return Err(QueryError::InvalidPage);
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct HistoryQuery {
    world_id: String,
    branch_id: String,
    year: Year,
    state: State,
    scenario: Scenario,
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

/// Grouped economic ledger rows for one committed historical close.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketsAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub enabled: bool,
    pub total: usize,
    pub money: Option<EconomicMoneyLedger>,
    pub treasury_goods: BTreeMap<chargen_domain::GoodId, EconomicGoodLedger>,
    pub groups: Vec<EconomicGroupLedger>,
}

/// Settlement-level physical market summaries from one annual report.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettlementMarketsAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub enabled: bool,
    pub total: usize,
    pub settlements: Vec<SettlementEconomicLedger>,
}

/// Recorded transactions, never reconstructed from current prices or stocks.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketTradesAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub enabled: bool,
    pub total: usize,
    pub trades: Vec<EconomicTrade>,
}

/// Historical realized events through this exact branch/year cutoff.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventsAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub total: usize,
    pub events: Vec<HistoricalEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettlementView {
    pub settlement_id: SettlementId,
    pub settlement_name: String,
    pub population: u64,
    pub capacity: Option<u32>,
    pub opportunity_score: Option<i64>,
    pub balance: Option<SettlementYearBalance>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettlementsAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub settlements: Vec<SettlementView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenRoutesAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub route_revision: Option<u64>,
    pub routes: Vec<OpenRouteAtClose>,
}

/// Public route projection for one close. It deliberately omits the complete
/// opening calendar so a route reported as open does not disclose future
/// schedule changes as if they were historical facts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenRouteAtClose {
    pub logical_id: LogicalId,
    pub from: SettlementId,
    pub to: SettlementId,
    pub cost: u32,
    pub capacity_per_year: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub settlements: Vec<SettlementYearBalance>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CultureTraitAtClose {
    pub key: TraitKey,
    pub weighted_sum: i64,
    pub total_weight: u64,
    pub member_count: u64,
    pub current_value: Option<i16>,
    pub minimum: Option<i16>,
    pub maximum: Option<i16>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettlementCultureAtClose {
    pub settlement_id: SettlementId,
    pub settlement_name: String,
    pub traits: Vec<CultureTraitAtClose>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CultureAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub enabled: bool,
    pub settlements: Vec<SettlementCultureAtClose>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstitutionAtClose {
    pub definition: InstitutionDefinition,
    pub active_affiliation_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstitutionsAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub institutions: Vec<InstitutionAtClose>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstitutionAffiliationsAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub institution_id: chargen_domain::InstitutionId,
    pub total: usize,
    pub affiliations: Vec<InstitutionMembership>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrganizationAtClose {
    pub organization: OrganizationRecordAtClose,
    pub active_at_close: bool,
    pub active_member_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrganizationRecordAtClose {
    pub id: OrganizationId,
    pub logical_id: LogicalId,
    pub name: String,
    pub settlement_id: SettlementId,
    pub founded_year: Year,
    pub dissolved_year: Option<Year>,
    pub founders: Vec<PersonId>,
    pub culture_values: Vec<CultureTraitAtClose>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrganizationsAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub organizations: Vec<OrganizationAtClose>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstitutionAffiliationAtClose {
    pub institution_id: chargen_domain::InstitutionId,
    pub settlement_id: SettlementId,
    pub validity: chargen_domain::YearInterval,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrganizationAffiliationAtClose {
    pub organization_id: OrganizationId,
    pub organization_name: String,
    pub settlement_id: SettlementId,
    pub founder: bool,
    pub validity: chargen_domain::YearInterval,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonAffiliationsAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub person_id: PersonId,
    pub institutions: Vec<InstitutionAffiliationAtClose>,
    pub organizations: Vec<OrganizationAffiliationAtClose>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrganizationMembersAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub organization_id: OrganizationId,
    pub total: usize,
    pub member_ids: Vec<PersonId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResidenceAtClose {
    pub settlement_id: SettlementId,
    pub validity: chargen_domain::YearInterval,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfessionAtClose {
    pub profession_id: chargen_domain::ProfessionId,
    pub validity: chargen_domain::YearInterval,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonResidenceAtClose {
    pub world_id: String,
    pub branch_id: String,
    pub year: Year,
    pub person: PersonRecord,
    pub residences: Vec<ResidenceAtClose>,
    pub professions: Vec<ProfessionAtClose>,
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
        let scenario: Scenario =
            serde_json::from_str(&store.scenario_json_at(&world_id, &branch_id, year)?)?;
        Ok(Self {
            world_id,
            branch_id,
            year,
            state,
            scenario,
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

    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    /// Return the saved settlements and their values visible at this close.
    pub fn settlements(&self) -> SettlementsAtClose {
        let census = self.census(None);
        let populations = census
            .settlements
            .into_iter()
            .map(|row| (row.settlement_id, row.population))
            .collect::<BTreeMap<_, _>>();
        let territory = self.scenario.territory.as_ref();
        let settlements = self
            .state
            .settlements
            .values()
            .map(|settlement| SettlementView {
                settlement_id: settlement.id,
                settlement_name: settlement.name.clone(),
                population: populations.get(&settlement.id).copied().unwrap_or_default(),
                capacity: territory
                    .and_then(|config| config.settlement_capacities.get(&settlement.id).copied()),
                opportunity_score: territory
                    .and_then(|config| config.opportunity_scores.get(&settlement.id).copied()),
                balance: self
                    .state
                    .last_year_settlement_balances
                    .get(&settlement.id)
                    .copied(),
            })
            .collect();
        SettlementsAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            settlements,
        }
    }

    /// List only directed routes open in this view's closing year.
    pub fn open_routes(&self) -> OpenRoutesAtClose {
        let territory = self.scenario.territory.as_ref();
        let routes = territory
            .into_iter()
            .flat_map(|config| config.routes.iter())
            .filter(|route| {
                !self.state.route_is_closed(route.logical_id, self.year)
                    && (route.open_intervals.is_empty()
                        || route
                            .open_intervals
                            .iter()
                            .any(|interval| interval.contains(self.year)))
            })
            .map(|route| OpenRouteAtClose {
                logical_id: route.logical_id,
                from: route.from,
                to: route.to,
                cost: route.cost,
                capacity_per_year: route.capacity_per_year,
            })
            .collect();
        OpenRoutesAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            route_revision: territory.map(|config| config.route_revision),
            routes,
        }
    }

    /// Per-settlement movement and demographic balance for the selected close.
    /// Return bounded economic group ledgers from the report stored in this
    /// exact annual close. Rulesets before E3 report the economy as disabled.
    pub fn markets(
        &self,
        settlement_id: Option<SettlementId>,
        offset: usize,
        limit: usize,
    ) -> QueryResult<MarketsAtClose> {
        validate_page(offset, limit)?;
        let report = self
            .state
            .economy
            .last_year_report
            .as_ref()
            .filter(|report| report.year == self.year);
        let mut groups = report
            .map(|report| {
                report
                    .groups
                    .iter()
                    .filter(|group| {
                        settlement_id
                            .is_none_or(|settlement| group.group_id.settlement_id == settlement)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let total = groups.len();
        groups = groups.into_iter().skip(offset).take(limit).collect();
        Ok(MarketsAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            enabled: self.scenario.economy.is_some(),
            total,
            money: report.map(|report| report.money.clone()),
            treasury_goods: report
                .map(|report| report.treasury_goods.clone())
                .unwrap_or_default(),
            groups,
        })
    }

    /// Return bounded settlement-level market ledgers from the committed
    /// report; values and prices are not recomputed from present-day config.
    pub fn market_settlements(
        &self,
        settlement_id: Option<SettlementId>,
        offset: usize,
        limit: usize,
    ) -> QueryResult<SettlementMarketsAtClose> {
        validate_page(offset, limit)?;
        let report = self
            .state
            .economy
            .last_year_report
            .as_ref()
            .filter(|report| report.year == self.year);
        let mut settlements = report
            .map(|report| {
                report
                    .settlements
                    .iter()
                    .filter(|row| settlement_id.is_none_or(|id| row.settlement_id == id))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let total = settlements.len();
        settlements = settlements.into_iter().skip(offset).take(limit).collect();
        Ok(SettlementMarketsAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            enabled: self.scenario.economy.is_some(),
            total,
            settlements,
        })
    }

    /// Return bounded transaction rows. A settlement filter includes trades
    /// whose buyer or seller belongs to that settlement.
    pub fn market_trades(
        &self,
        settlement_id: Option<SettlementId>,
        offset: usize,
        limit: usize,
    ) -> QueryResult<MarketTradesAtClose> {
        validate_page(offset, limit)?;
        let report = self
            .state
            .economy
            .last_year_report
            .as_ref()
            .filter(|report| report.year == self.year);
        let mut trades = report
            .map(|report| {
                report
                    .trades
                    .iter()
                    .filter(|trade| {
                        settlement_id.is_none_or(|settlement| {
                            trade.source_group.settlement_id == settlement
                                || trade.destination_group.settlement_id == settlement
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let total = trades.len();
        trades = trades.into_iter().skip(offset).take(limit).collect();
        Ok(MarketTradesAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            enabled: self.scenario.economy.is_some(),
            total,
            trades,
        })
    }

    /// Read realized events through the cutoff using the branch-aware SQL
    /// entity index. The scheduled-event agenda itself is never returned.
    pub fn events(
        &self,
        settlement_id: Option<SettlementId>,
        offset: usize,
        limit: usize,
    ) -> QueryResult<EventsAtClose> {
        validate_page(offset, limit)?;
        let (entity_type, entity_id) = settlement_id
            .map(|id| ("settlement", id.0.to_string()))
            .unwrap_or_else(|| ("world", self.world_id.clone()));
        let page = self.store.events_for_entity_page(
            &self.world_id,
            &self.branch_id,
            self.year,
            entity_type,
            &entity_id,
            offset,
            limit,
        )?;
        Ok(EventsAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            total: page.total,
            events: page.events,
        })
    }

    pub fn migration(&self, settlement_id: Option<SettlementId>) -> MigrationAtClose {
        MigrationAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            settlements: self
                .state
                .last_year_settlement_balances
                .values()
                .filter(|balance| {
                    settlement_id.is_none_or(|requested| requested == balance.settlement_id)
                })
                .copied()
                .collect(),
        }
    }

    /// Cultural aggregates for every settlement (or one selected settlement).
    /// Missing/zero-weight aggregates have `current_value: None`.
    pub fn culture(&self, settlement_id: Option<SettlementId>) -> CultureAtClose {
        let config = self.scenario.culture.as_ref();
        let settlements = self
            .state
            .settlements
            .values()
            .filter(|settlement| settlement_id.is_none_or(|requested| requested == settlement.id))
            .map(|settlement| {
                let aggregates = self.state.culture.settlements.get(&settlement.id);
                let mut keys = BTreeSet::new();
                if let Some(dimensions) = config.map(|value| &value.dimensions) {
                    keys.extend(dimensions.keys().cloned());
                }
                if let Some(aggregates) = aggregates {
                    keys.extend(aggregates.keys().cloned());
                }
                let traits = keys
                    .into_iter()
                    .map(|key| {
                        let aggregate = aggregates
                            .and_then(|values| values.get(&key))
                            .copied()
                            .unwrap_or_else(CultureAggregate::default);
                        let policy = config.and_then(|value| value.dimensions.get(&key));
                        CultureTraitAtClose {
                            key,
                            weighted_sum: aggregate.weighted_sum,
                            total_weight: aggregate.total_weight,
                            member_count: aggregate.member_count,
                            current_value: aggregate.value(),
                            minimum: policy.map(|policy| policy.minimum),
                            maximum: policy.map(|policy| policy.maximum),
                        }
                    })
                    .collect();
                SettlementCultureAtClose {
                    settlement_id: settlement.id,
                    settlement_name: settlement.name.clone(),
                    traits,
                }
            })
            .collect();
        CultureAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            enabled: config.is_some(),
            settlements,
        }
    }

    /// Configured definitions plus affiliations active at this historical close.
    pub fn institutions(
        &self,
        settlement_id: Option<SettlementId>,
        person_id: Option<PersonId>,
    ) -> InstitutionsAtClose {
        let definitions = self
            .scenario
            .culture
            .as_ref()
            .into_iter()
            .flat_map(|config| config.institutions.iter());
        let institutions = definitions
            .map(|definition| InstitutionAtClose {
                definition: definition.clone(),
                active_affiliation_count: self
                    .state
                    .culture
                    .institution_memberships
                    .iter()
                    .filter(|membership| {
                        self.institution_membership_matches(
                            definition,
                            membership,
                            settlement_id,
                            person_id,
                        )
                    })
                    .count(),
            })
            .collect();
        InstitutionsAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            institutions,
        }
    }

    /// Return one bounded page of current affiliations for a configured
    /// institution. The list endpoint exposes only a count.
    pub fn institution_affiliations(
        &self,
        institution_id: &chargen_domain::InstitutionId,
        settlement_id: Option<SettlementId>,
        person_id: Option<PersonId>,
        offset: usize,
        limit: usize,
    ) -> QueryResult<InstitutionAffiliationsAtClose> {
        let definition = self
            .scenario
            .culture
            .as_ref()
            .into_iter()
            .flat_map(|culture| culture.institutions.iter())
            .find(|definition| &definition.id == institution_id)
            .ok_or_else(|| QueryError::InstitutionNotFound(institution_id.0.clone()))?;
        let total = self
            .state
            .culture
            .institution_memberships
            .iter()
            .filter(|membership| {
                self.institution_membership_matches(
                    definition,
                    membership,
                    settlement_id,
                    person_id,
                )
            })
            .count();
        let affiliations = self
            .state
            .culture
            .institution_memberships
            .iter()
            .filter(|membership| {
                self.institution_membership_matches(
                    definition,
                    membership,
                    settlement_id,
                    person_id,
                )
            })
            .skip(offset)
            .take(limit)
            .map(|membership| InstitutionMembership {
                validity: visible_interval(membership.validity, self.year),
                ..membership.clone()
            })
            .collect();
        Ok(InstitutionAffiliationsAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            institution_id: institution_id.clone(),
            total,
            affiliations,
        })
    }

    fn institution_membership_matches(
        &self,
        definition: &InstitutionDefinition,
        membership: &InstitutionMembership,
        settlement_id: Option<SettlementId>,
        person_id: Option<PersonId>,
    ) -> bool {
        if membership.institution_id != definition.id || !membership.active_at_close(self.year) {
            return false;
        }
        let scope_matches = match definition.scope {
            chargen_domain::InstitutionScope::World => settlement_id.is_none_or(|requested| {
                self.state
                    .persons
                    .get(&membership.person_id)
                    .is_some_and(|person| person.settlement_id == requested)
            }),
            chargen_domain::InstitutionScope::Settlement => {
                settlement_id.is_none_or(|requested| requested == membership.settlement_id)
            }
        };
        scope_matches
            && definition
                .allowed_settlements
                .as_ref()
                .is_none_or(|allowed| allowed.contains(&membership.settlement_id))
            && person_id.is_none_or(|requested| requested == membership.person_id)
    }

    /// Return organizations founded by this cutoff, preserving dissolved
    /// records while calculating active members strictly for this year.
    pub fn organizations(&self, settlement_id: Option<SettlementId>) -> OrganizationsAtClose {
        let organizations = self
            .state
            .culture
            .organizations
            .values()
            .filter(|organization| organization.founded_year <= self.year)
            .filter(|organization| {
                settlement_id.is_none_or(|requested| requested == organization.settlement_id)
            })
            .map(|organization| OrganizationAtClose {
                active_at_close: organization.active_at_close(self.year),
                active_member_count: organization.members_at_close(self.year).count(),
                organization: OrganizationRecordAtClose {
                    id: organization.id,
                    logical_id: organization.logical_id,
                    name: organization.name.clone(),
                    settlement_id: organization.settlement_id,
                    founded_year: organization.founded_year,
                    dissolved_year: organization
                        .dissolved_year
                        .filter(|dissolved| *dissolved <= self.year),
                    founders: organization.founders.iter().copied().collect(),
                    culture_values: organization
                        .culture_values
                        .iter()
                        .map(|(key, aggregate)| CultureTraitAtClose {
                            key: key.clone(),
                            weighted_sum: aggregate.weighted_sum,
                            total_weight: aggregate.total_weight,
                            member_count: aggregate.member_count,
                            current_value: aggregate.value(),
                            minimum: None,
                            maximum: None,
                        })
                        .collect(),
                },
            })
            .collect();
        OrganizationsAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            organizations,
        }
    }

    /// Return only the page of members active in an organization at this close.
    pub fn organization_members(
        &self,
        organization_id: OrganizationId,
        offset: usize,
        limit: usize,
    ) -> QueryResult<OrganizationMembersAtClose> {
        let organization = self
            .state
            .culture
            .organizations
            .get(&organization_id)
            .filter(|organization| organization.founded_year <= self.year)
            .ok_or(QueryError::OrganizationNotFound(organization_id.0))?;
        let total = organization.members_at_close(self.year).count();
        let member_ids = organization
            .members_at_close(self.year)
            .skip(offset)
            .take(limit)
            .collect();
        Ok(OrganizationMembersAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            organization_id,
            total,
            member_ids,
        })
    }

    /// Residence and profession histories visible for a person at this close.
    pub fn residence(&self, person_id: PersonId) -> QueryResult<PersonResidenceAtClose> {
        let person = self
            .person(person_id)
            .ok_or(QueryError::PersonNotFound(person_id.0))?;
        let residences = self
            .state
            .residences
            .get(&person_id)
            .into_iter()
            .flatten()
            .filter(|interval| interval.validity.start <= self.year)
            .map(|interval| ResidenceAtClose {
                settlement_id: interval.settlement_id,
                validity: visible_interval(interval.validity, self.year),
            })
            .collect();
        let professions = self
            .state
            .profession_intervals
            .get(&person_id)
            .into_iter()
            .flatten()
            .filter(|interval| interval.validity.start <= self.year)
            .map(|interval| ProfessionAtClose {
                profession_id: interval.profession_id.clone(),
                validity: visible_interval(interval.validity, self.year),
            })
            .collect();
        Ok(PersonResidenceAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            person,
            residences,
            professions,
        })
    }

    /// Current and historical memberships/affiliations for a person.
    pub fn affiliations(&self, person_id: PersonId) -> QueryResult<PersonAffiliationsAtClose> {
        if self.person(person_id).is_none() {
            return Err(QueryError::PersonNotFound(person_id.0));
        }
        let institutions = self
            .state
            .culture
            .institution_memberships
            .iter()
            .filter(|membership| {
                membership.person_id == person_id && membership.validity.start <= self.year
            })
            .map(|membership| InstitutionAffiliationAtClose {
                institution_id: membership.institution_id.clone(),
                settlement_id: membership.settlement_id,
                validity: visible_interval(membership.validity, self.year),
            })
            .collect();
        let organizations = self
            .state
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
                    .map(move |membership| OrganizationAffiliationAtClose {
                        organization_id: organization.id,
                        organization_name: organization.name.clone(),
                        settlement_id: organization.settlement_id,
                        founder: membership.founder,
                        validity: visible_interval(membership.validity, self.year),
                    })
            })
            .collect();
        Ok(PersonAffiliationsAtClose {
            world_id: self.world_id.clone(),
            branch_id: self.branch_id.clone(),
            year: self.year,
            person_id,
            institutions,
            organizations,
        })
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

fn visible_interval(mut interval: YearInterval, cutoff: Year) -> YearInterval {
    if interval
        .end_exclusive
        .is_some_and(|end_exclusive| end_exclusive > cutoff)
    {
        interval.end_exclusive = None;
    }
    interval
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
        CultureConfig, CultureDimensionPolicy, CultureEligibility, DemographicPolicies,
        DirectedRoute, EntityRef, EventEffect, EventPopulationFilter, FamilyLinkId, FamilyLinkKind,
        GeneticCatalog, Genome, HistoricalEventKind, LogicalId, MigrationPolicy, Person,
        ReproductiveProfile, Scenario, ScheduledEvent, Settlement, TerritoryConfig,
        TraitDefinition, WorldId,
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
            profession: None,
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
            profession: None,
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
            territory: None,
            culture: None,
            economy: None,
            scheduled_events: None,
        }
    }

    fn territorial_scenario() -> Scenario {
        let mut scenario = scenario();
        scenario.schema_version = Scenario::TERRITORIAL_SCHEMA_VERSION;
        scenario.rules_version = Scenario::TERRITORIAL_RULES_VERSION;
        scenario.settlements.push(Settlement {
            id: SettlementId(2),
            logical_id: LogicalId(12),
            name: "Hill".into(),
            culture_values: BTreeMap::new(),
        });
        let key = TraitKey {
            namespace: "culture".into(),
            name: "cohesion".into(),
        };
        scenario.trait_catalog.push(TraitDefinition {
            key: key.clone(),
            minimum: 0,
            maximum: 100,
            unit: None,
            description: None,
        });
        for person in &mut scenario.initial_people {
            person.traits.insert(key.clone(), 50);
        }
        scenario.territory = Some(TerritoryConfig {
            route_revision: 1,
            routes: vec![DirectedRoute {
                logical_id: LogicalId(901),
                from: SettlementId(1),
                to: SettlementId(2),
                cost: 2,
                capacity_per_year: None,
                open_intervals: Vec::new(),
            }],
            migration: MigrationPolicy::default(),
            ..TerritoryConfig::default()
        });
        scenario.culture = Some(CultureConfig {
            dimensions: [(
                key,
                CultureDimensionPolicy {
                    eligibility: CultureEligibility::AllLiving,
                    member_weight: 1,
                    parent_weight: 1,
                    local_weight: 1,
                    variation_max: 0,
                    minimum: 0,
                    maximum: 100,
                    founder_value: Some(50),
                },
            )]
            .into_iter()
            .collect(),
            ..CultureConfig::default()
        });
        scenario
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
            profession: None,
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

    #[test]
    fn e2_child_configuration_does_not_leak_into_its_shared_cutoff() {
        let temp = TempWorkspace::new();
        let parent_scenario = territorial_scenario();
        let parent_validated = parent_scenario.validate().unwrap();
        let initial = Engine::from_scenario(parent_validated.clone())
            .unwrap()
            .state()
            .clone();
        let store = HistoryStore::open(temp.db()).unwrap();
        let parent_json = serde_json::to_string(&parent_scenario).unwrap();
        store
            .create_world("201", "main", &parent_json, &initial)
            .unwrap();

        let mut parent_engine = Engine::new(parent_validated, initial).unwrap();
        {
            let mut writer = store.acquire_writer("201", "main").unwrap();
            writer
                .commit_year(&parent_engine.advance_year(None).unwrap())
                .unwrap();
            writer
                .commit_year(&parent_engine.advance_year(None).unwrap())
                .unwrap();
        }

        let mut child_scenario = parent_scenario.clone();
        child_scenario.territory.as_mut().unwrap().routes[0].open_intervals =
            vec![chargen_domain::YearInterval {
                start: Year(0),
                end_exclusive: Some(Year(1)),
            }];
        child_scenario.territory.as_mut().unwrap().route_revision = 2;
        child_scenario
            .culture
            .as_mut()
            .unwrap()
            .dimensions
            .values_mut()
            .next()
            .unwrap()
            .maximum = 90;
        let child_validated = child_scenario.validate().unwrap();
        let child_json = serde_json::to_string(&child_scenario).unwrap();
        let child_initial = store
            .fork_branch("201", "main", "child-e2", Year(0), &child_json)
            .unwrap();
        let mut child_engine = Engine::new(child_validated, child_initial).unwrap();
        store
            .acquire_writer("201", "child-e2")
            .unwrap()
            .commit_year(&child_engine.advance_year(None).unwrap())
            .unwrap();

        let child_cut = HistoryQuery::open(temp.db(), "201", "child-e2", Year(0)).unwrap();
        assert_eq!(
            child_cut
                .scenario()
                .territory
                .as_ref()
                .unwrap()
                .route_revision,
            1
        );
        assert_eq!(child_cut.open_routes().routes.len(), 1);
        let cutoff_culture = child_cut.culture(Some(SettlementId(1)));
        assert_eq!(cutoff_culture.settlements[0].traits[0].maximum, Some(100));

        let child_next = HistoryQuery::open(temp.db(), "201", "child-e2", Year(1)).unwrap();
        assert_eq!(
            child_next
                .scenario()
                .territory
                .as_ref()
                .unwrap()
                .route_revision,
            2
        );
        assert!(child_next.open_routes().routes.is_empty());
        assert_eq!(
            child_next.culture(Some(SettlementId(1))).settlements[0].traits[0].maximum,
            Some(90)
        );

        let parent_next = HistoryQuery::open(temp.db(), "201", "main", Year(1)).unwrap();
        assert_eq!(parent_next.open_routes().routes.len(), 1);
        assert_eq!(
            parent_next.culture(Some(SettlementId(1))).settlements[0].traits[0].maximum,
            Some(100)
        );
    }

    #[test]
    fn organization_member_projection_is_paged_and_cutoff_specific() {
        let temp = TempWorkspace::new();
        let scenario: Scenario = serde_json::from_str(include_str!(
            "../../../scenarios/demos/e2-territory-culture.json"
        ))
        .unwrap();
        let validated = scenario.validate().unwrap();
        let initial = Engine::from_scenario(validated.clone())
            .unwrap()
            .state()
            .clone();
        let store = HistoryStore::open(temp.db()).unwrap();
        store
            .create_world(
                &scenario.world_id.0.to_string(),
                "main",
                &serde_json::to_string(&scenario).unwrap(),
                &initial,
            )
            .unwrap();
        let mut engine = Engine::new(validated, initial).unwrap();
        store
            .acquire_writer(&scenario.world_id.0.to_string(), "main")
            .unwrap()
            .commit_year(&engine.advance_year(None).unwrap())
            .unwrap();

        let query = HistoryQuery::open(temp.db(), scenario.world_id.0.to_string(), "main", Year(0))
            .unwrap();
        let organization = query.organizations(None).organizations.remove(0);
        let page = query
            .organization_members(organization.organization.id, 1, 1)
            .unwrap();
        assert_eq!(page.total, organization.active_member_count);
        assert_eq!(page.member_ids.len(), 1);
        assert!(organization
            .organization
            .founders
            .contains(&page.member_ids[0]));
    }

    #[test]
    fn open_routes_respect_realized_closures_at_the_selected_close_only() {
        let temp = TempWorkspace::new();
        let mut scenario: Scenario = serde_json::from_str(include_str!(
            "../../../scenarios/demos/e3-economy-events.json"
        ))
        .unwrap();
        let route_id = scenario.territory.as_ref().unwrap().routes[0].logical_id;
        scenario.territory.as_mut().unwrap().routes[0]
            .open_intervals
            .clear();
        scenario
            .scheduled_events
            .as_mut()
            .unwrap()
            .push(ScheduledEvent {
                id: LogicalId(98_101),
                name: "Cierre temporal para consulta histórica".into(),
                start_year: Year(1),
                duration_years: Some(1),
                priority: 100,
                population_filter: EventPopulationFilter::default(),
                effects: vec![EventEffect::RouteClosure {
                    route_ids: BTreeSet::from([route_id]),
                }],
            });
        let validated = scenario.validate().unwrap();
        let initial = Engine::from_scenario(validated.clone())
            .unwrap()
            .state()
            .clone();
        let store = HistoryStore::open(temp.db()).unwrap();
        store
            .create_world(
                &scenario.world_id.0.to_string(),
                "main",
                &serde_json::to_string(&scenario).unwrap(),
                &initial,
            )
            .unwrap();
        let mut engine = Engine::new(validated, initial).unwrap();
        let mut writer = store
            .acquire_writer(&scenario.world_id.0.to_string(), "main")
            .unwrap();
        for _ in 0..=2 {
            writer
                .commit_year(&engine.advance_year(None).unwrap())
                .unwrap();
        }

        let before =
            HistoryQuery::open(temp.db(), scenario.world_id.0.to_string(), "main", Year(0))
                .unwrap();
        let during =
            HistoryQuery::open(temp.db(), scenario.world_id.0.to_string(), "main", Year(1))
                .unwrap();
        let after = HistoryQuery::open(temp.db(), scenario.world_id.0.to_string(), "main", Year(2))
            .unwrap();
        assert_eq!(
            before.open_routes().routes.len(),
            1,
            "future closure leaked"
        );
        assert!(
            during.open_routes().routes.is_empty(),
            "active closure was ignored"
        );
        assert_eq!(
            after.open_routes().routes.len(),
            1,
            "expired closure persisted"
        );
        assert_eq!(after.open_routes().routes[0].logical_id, route_id);
    }

    #[test]
    fn e3_event_pages_round_trip_numeric_summary_keys_and_route_ids() {
        let temp = TempWorkspace::new();
        let scenario = scenario();
        let validated = scenario.validate().unwrap();
        let initial = Engine::from_scenario(validated.clone())
            .unwrap()
            .state()
            .clone();
        let store = HistoryStore::open(temp.db()).unwrap();
        store
            .create_world(
                "201",
                "main",
                &serde_json::to_string(&scenario).unwrap(),
                &initial,
            )
            .unwrap();
        let mut engine = Engine::new(validated, initial).unwrap();
        let mut commit = engine.advance_year(None).unwrap();
        commit.events.clear();

        // The summary contains an explicit settlement ID instead of a JSON
        // object key so tagged-event deserialization remains type-safe.
        let economic_close = serde_json::json!({
            "id": "99001",
            "year": 0,
            "order": 0,
            "affected_entities": [
                {"world": 201},
                {"settlement": 1},
                {"economic_group": {"settlement_id": 1, "profession_id": "farmer"}}
            ],
            "kind": {
                "type": "economic_year_closed",
                "data": {"summary": {
                    "year": 0,
                    "settlement_ids": [1],
                    "group_ids": [{"settlement_id": 1, "profession_id": "farmer"}],
                    "money": {
                        "opening_group_money": "10",
                        "opening_treasury_money": "0",
                        "closing_group_money": "10",
                        "closing_treasury_money": "0",
                        "external_transport_outflow": "0",
                        "explicit_emission": "0",
                        "explicit_retirement": "0"
                    },
                    "physical_shortage_totals": [{"settlement_id": 1, "goods": {"grain": "2"}}]
                }}
            }
        });
        let displacement = serde_json::json!({
            "id": "99002",
            "year": 0,
            "order": 1,
            "affected_entities": [
                {"world": 201},
                {"person": 1},
                {"settlement": 1},
                {"settlement": 2},
                {"route": "901"}
            ],
            "kind": {
                "type": "event_effect_realized",
                "data": {
                    "event_id": "99002",
                    "event_name": "Desplazamiento de prueba",
                    "effect_index": 0,
                    "effect": {"type": "displacement", "data": {
                        "selection": {"mode": "exact", "data": {"count": 1, "overflow": "clamp_with_warning"}},
                        "destination": 2
                    }},
                    "realization": {"kind": "displacement", "data": {
                        "requested_person_ids": [1],
                        "moved_person_ids": [1],
                        "moves": [{"person_ids": [1], "from": 1, "to": 2, "route_ids": ["901"]}],
                        "requested_count": 1,
                        "realized_count": 1,
                        "warning": null
                    }}
                }
            }
        });
        commit.events = vec![
            serde_json::from_value(economic_close).unwrap(),
            serde_json::from_value(displacement).unwrap(),
        ];
        store
            .acquire_writer("201", "main")
            .unwrap()
            .commit_year(&commit)
            .unwrap();

        let view = HistoryQuery::open(temp.db(), "201", "main", Year(0)).unwrap();
        let first = view.events(None, 0, 1).unwrap();
        let second = view.events(None, 1, 1).unwrap();
        assert_eq!(first.total, 2);
        assert_eq!(second.total, 2);
        assert!(matches!(
            first.events[0].kind,
            HistoricalEventKind::EconomicYearClosed { .. }
        ));
        let HistoricalEventKind::EventEffectRealized {
            realization: chargen_domain::EventRealization::Displacement { moves, .. },
            ..
        } = &second.events[0].kind
        else {
            panic!("expected the realized displacement in the second page");
        };
        assert_eq!(moves[0].route_ids, vec![LogicalId(901)]);
        let settlement_events = view.events(Some(SettlementId(2)), 0, 10).unwrap();
        assert_eq!(settlement_events.total, 1);
        assert_eq!(settlement_events.events[0].id, LogicalId(99_002));
    }
}
