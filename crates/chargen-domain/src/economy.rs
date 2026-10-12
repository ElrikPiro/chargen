//! Rules and serialized state for the grouped, integer-only E3 economy.
//!
//! The accounting unit for each good is configured by its `unit` string. Money
//! and quantities are the existing transparent `i64` newtypes. Rates use ppm;
//! all conversions use checked integer arithmetic in the engine.

use crate::{
    LogicalId, Money, PersonId, ProfessionId, Quantity, SettlementId, ValidationIssue, Year,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Stable catalog key for a good. The value is a local, scenario-owned ID.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GoodId(pub String);

impl From<&str> for GoodId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// A market group has one shared account and inventory for a place and a
/// profession. `None` is the consumer base for all people without a profession,
/// including dependents and minors; it is not a claim that they are idle.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicGroupId {
    pub settlement_id: SettlementId,
    pub profession_id: Option<ProfessionId>,
}

impl EconomicGroupId {
    pub fn consumer_group(settlement_id: SettlementId) -> Self {
        Self {
            settlement_id,
            profession_id: None,
        }
    }
}

/// One scenario-owned good. Prices are integer money units per configured good
/// unit. Non-storable goods lose every unsold remainder after consumption.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoodDefinition {
    pub name: String,
    /// Human-readable accounting unit, such as `loaf` or `kg`; it has no hidden
    /// conversion to another good.
    pub unit: String,
    /// Cargo capacity units consumed by one unit of this good on a route.
    pub transport_units_per_unit: u32,
    pub storable: bool,
    /// Annual loss applied after consumption, in parts per million. Ignored for
    /// non-storable goods, whose remaining stock is fully lost at year close.
    pub annual_storage_loss_ppm: u32,
    pub initial_price: Money,
    pub minimum_price: Money,
    pub maximum_price: Money,
}

/// One indivisible production run. Each active worker enables the configured
/// number of batches, capped by `maximum_batches_per_year` when supplied.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionRecipe {
    pub batches_per_worker: u32,
    pub maximum_batches_per_year: Option<u64>,
    #[serde(default)]
    pub inputs_per_batch: BTreeMap<GoodId, Quantity>,
    #[serde(default)]
    pub outputs_per_batch: BTreeMap<GoodId, Quantity>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfessionEconomicPolicy {
    pub recipe: Option<ProductionRecipe>,
}

/// Initial profile composition option. `None` creates a consumer-base member;
/// an empty profile distribution retains the legacy all-unemployed default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedProfession {
    pub profession_id: Option<ProfessionId>,
    pub weight: u32,
}

/// Annual physical need for one living person. Lower priority values are
/// consumed and allocated first when several goods compete for group money.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoodNeed {
    pub quantity_per_person_per_year: Quantity,
    pub priority: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportCostAccount {
    /// Route fees are transferred to the configured economy treasury.
    #[default]
    Treasury,
    /// Route fees leave the modeled economy and appear as explicit outflow.
    ExternalSink,
}

/// Deterministic order used when buyers compete for limited local stock or
/// directed-route capacity. The default derives a stable rank from logical
/// group identity; the alternative explicitly uses lexicographic group order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuyerPriorityPolicy {
    #[default]
    SeededLogicalRank,
    GroupKeyOrder,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetContinuity {
    /// Money and stock stay with the origin group when members leave or die.
    #[default]
    RetainGroupAssets,
    /// For living members changing group, transfer their proportional share in
    /// one batch; deaths have no recipient and leave assets at the origin.
    ProportionalShareTransfer,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyGroupDisposition {
    /// Keep the account dormant; it can be reused if the group gains members.
    #[default]
    RetainDormant,
    /// Move all assets into the same settlement's consumer group.
    TransferToConsumerGroup,
    /// Move all assets into the economy treasury.
    TransferToTreasury,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EconomicPricePolicy {
    /// One bounded price step, in ppm of the current price, per annual close.
    /// A positive step changes by at least one money unit when the price can move.
    pub adjustment_step_ppm: u32,
}

impl Default for EconomicPricePolicy {
    fn default() -> Self {
        Self {
            adjustment_step_ppm: 50_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EconomicLaborPolicy {
    pub enabled: bool,
    pub minimum_working_age: u16,
    /// Upper bound per settlement and year. Zero disables automatic changes.
    pub max_changes_per_settlement_per_year: u16,
}

impl Default for EconomicLaborPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            minimum_working_age: 18,
            max_changes_per_settlement_per_year: 0,
        }
    }
}

/// Explicit, optional bridge from prior/current economic scarcity into other
/// annual systems. All-zero leaves demography and migration unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EconomicResponsePolicy {
    /// Added to ordinary mortality probability at full physical scarcity.
    pub mortality_increase_at_full_scarcity_ppm: u32,
    /// Removed from birth probability at full physical scarcity.
    pub birth_reduction_at_full_scarcity_ppm: u32,
    /// Signed opportunity-score adjustment at full scarcity; zero is neutral.
    pub migration_opportunity_at_full_scarcity: i64,
}

/// Optional bounded A6-to-next-A0 production response. Capacity values are
/// ppm multipliers of the configured per-worker recipe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EconomicProductionResponsePolicy {
    pub enabled: bool,
    pub step_ppm: u32,
    pub minimum_capacity_ppm: u32,
    pub maximum_capacity_ppm: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicGroupEndowment {
    pub money: Money,
    #[serde(default)]
    pub goods: BTreeMap<GoodId, Quantity>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicGroupEndowmentEntry {
    pub group_id: EconomicGroupId,
    pub endowment: EconomicGroupEndowment,
}

/// Opt-in E3 economic catalog and explicit monetary/production policies.
/// `initial_group_endowments` are applied once when a world is initialized;
/// later-created groups never receive them automatically.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EconomicConfig {
    pub goods: BTreeMap<GoodId, GoodDefinition>,
    /// Entries reference professions declared by `TerritoryConfig`.
    pub profession_policies: BTreeMap<ProfessionId, ProfessionEconomicPolicy>,
    /// Missing goods have zero physical need.
    pub needs: BTreeMap<GoodId, GoodNeed>,
    /// Per directed route cargo capacity, measured in transport units rather
    /// than resident capacity or quantities of any one good.
    pub route_goods_capacities: BTreeMap<LogicalId, Quantity>,
    /// Money charged for each route-cost point and cargo unit.
    pub transport_money_per_route_cost_unit: Money,
    pub transport_cost_account: TransportCostAccount,
    pub initial_group_endowments: Vec<EconomicGroupEndowmentEntry>,
    /// Explicit starting account; no annual or migration-time cash is created.
    pub initial_treasury: EconomicGroupEndowment,
    pub price_policy: EconomicPricePolicy,
    pub buyer_priority: BuyerPriorityPolicy,
    pub asset_continuity: AssetContinuity,
    pub empty_group_disposition: EmptyGroupDisposition,
    pub labor_policy: EconomicLaborPolicy,
    pub responses: EconomicResponsePolicy,
    pub production_response: EconomicProductionResponsePolicy,
}

impl EconomicConfig {
    /// Allows the containing `Scenario` field to be omitted in legacy rulesets.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicGroupState {
    pub group_id: EconomicGroupId,
    pub member_count: u64,
    pub worker_count: u64,
    pub money: Money,
    #[serde(default)]
    pub goods: BTreeMap<GoodId, Quantity>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomyPriceKey {
    pub settlement_id: SettlementId,
    pub good_id: GoodId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomyState {
    /// Sorted by `group_id`; a vector avoids invalid JSON object keys.
    #[serde(default)]
    pub groups: Vec<EconomicGroupState>,
    /// Active living-person assignment used for batch continuity transfers.
    #[serde(default)]
    pub person_groups: BTreeMap<PersonId, EconomicGroupId>,
    #[serde(default)]
    pub prices_by_settlement: BTreeMap<SettlementId, BTreeMap<GoodId, Money>>,
    /// Price moves decided at A6 and installed at the following A0.
    #[serde(default)]
    pub pending_prices: BTreeMap<SettlementId, BTreeMap<GoodId, Money>>,
    /// Active A4 multipliers; an absent group uses 1,000,000 ppm.
    #[serde(default)]
    pub production_capacity_ppm_by_group: Vec<EconomicProductionCapacity>,
    /// Capacity changes selected in A6 are installed at the next A0.
    #[serde(default)]
    pub pending_production_capacity_ppm_by_group: Vec<EconomicProductionCapacity>,
    #[serde(default)]
    pub treasury: EconomicGroupEndowment,
    /// Only populated within an uncommitted annual transaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_report: Option<EconomicYearReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_year_report: Option<EconomicYearReport>,
    /// Rules selected at A6, effective at the next A0.
    #[serde(default)]
    pub pending_profession_changes: Vec<EconomicProfessionChange>,
}

impl EconomyState {
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
            && self.person_groups.is_empty()
            && self.prices_by_settlement.is_empty()
            && self.pending_prices.is_empty()
            && self.production_capacity_ppm_by_group.is_empty()
            && self.pending_production_capacity_ppm_by_group.is_empty()
            && self.treasury == EconomicGroupEndowment::default()
            && self.working_report.is_none()
            && self.last_year_report.is_none()
            && self.pending_profession_changes.is_empty()
    }

    pub fn group(&self, group_id: &EconomicGroupId) -> Option<&EconomicGroupState> {
        self.groups
            .binary_search_by(|group| group.group_id.cmp(group_id))
            .ok()
            .map(|index| &self.groups[index])
    }

    pub fn group_mut(&mut self, group_id: &EconomicGroupId) -> Option<&mut EconomicGroupState> {
        self.groups
            .binary_search_by(|group| group.group_id.cmp(group_id))
            .ok()
            .map(|index| &mut self.groups[index])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicProductionCapacity {
    pub group_id: EconomicGroupId,
    pub capacity_ppm: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicProfessionChange {
    pub person_id: PersonId,
    pub from: Option<ProfessionId>,
    pub to: Option<ProfessionId>,
    pub effective_year: Year,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicGoodLedger {
    pub opening_stock: Quantity,
    pub produced: Quantity,
    pub input_consumed: Quantity,
    pub input_replenishment_target: Quantity,
    pub unmet_input_demand: Quantity,
    pub transfers_in: Quantity,
    pub transfers_out: Quantity,
    pub bought: Quantity,
    pub sold: Quantity,
    pub consumer_consumed: Quantity,
    pub storage_loss: Quantity,
    pub event_destroyed: Quantity,
    pub closing_stock: Quantity,
    pub physical_need: Quantity,
    pub unmet_physical_need: Quantity,
    pub solvent_demand: Quantity,
    pub unmet_solvent_demand: Quantity,
    pub solvent_input_demand: Quantity,
    pub unmet_solvent_input_demand: Quantity,
    pub bought_for_human_need: Quantity,
    pub bought_for_inputs: Quantity,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicGroupLedger {
    pub group_id: EconomicGroupId,
    pub member_count_before: u64,
    pub worker_count_before: u64,
    pub member_count_after: u64,
    pub worker_count_after: u64,
    pub opening_money: Money,
    pub sales_income: Money,
    pub purchases_spent: Money,
    pub transfers_in: Money,
    pub transfers_out: Money,
    pub closing_money: Money,
    #[serde(default)]
    pub goods: BTreeMap<GoodId, EconomicGoodLedger>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementGoodMarketLedger {
    pub good_id: GoodId,
    pub opening_price: Money,
    pub next_price: Money,
    pub physical_need: Quantity,
    pub unmet_physical_need: Quantity,
    pub solvent_demand: Quantity,
    pub unmet_solvent_demand: Quantity,
    pub input_replenishment_target: Quantity,
    pub unmet_input_demand: Quantity,
    pub solvent_input_demand: Quantity,
    pub unmet_solvent_input_demand: Quantity,
    pub local_traded: Quantity,
    pub imports: Quantity,
    pub exports: Quantity,
    /// Supply offered to the A5 market that remained unmatched after local and route trades.
    pub unsold_offer: Quantity,
    pub ending_stock: Quantity,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementEconomicLedger {
    pub settlement_id: SettlementId,
    pub goods: Vec<SettlementGoodMarketLedger>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicTrade {
    pub source_group: EconomicGroupId,
    pub destination_group: EconomicGroupId,
    pub good_id: GoodId,
    pub quantity: Quantity,
    pub seller_revenue: Money,
    /// Actual buyer debit. For route trades this is seller revenue plus the
    /// explicit transport charge; it can be below the destination reference price.
    pub buyer_payment: Money,
    pub transport_cost: Money,
    pub route_id: Option<LogicalId>,
    pub purpose: TradePurpose,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradePurpose {
    #[default]
    HumanNeed,
    ProductionInput,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicMoneyLedger {
    pub opening_group_money: Money,
    pub opening_treasury_money: Money,
    pub closing_group_money: Money,
    pub closing_treasury_money: Money,
    pub external_transport_outflow: Money,
    pub explicit_emission: Money,
    pub explicit_retirement: Money,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicYearReport {
    pub year: Year,
    pub groups: Vec<EconomicGroupLedger>,
    pub settlements: Vec<SettlementEconomicLedger>,
    pub trades: Vec<EconomicTrade>,
    pub stock_losses: Vec<EconomyStockLossRecord>,
    pub money: EconomicMoneyLedger,
    /// Separate inventory account for one-time and explicit treasury assets.
    #[serde(default)]
    pub treasury_goods: BTreeMap<GoodId, EconomicGoodLedger>,
    pub signals: EconomySignals,
}

/// Compact close event. Full transaction details live in the historical
/// `EconomyState::last_year_report` and remain independently pageable.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomicYearClosedSummary {
    pub year: Year,
    pub settlement_ids: Vec<SettlementId>,
    pub group_ids: Vec<EconomicGroupId>,
    pub money: EconomicMoneyLedger,
    pub physical_shortage_totals: Vec<SettlementPhysicalShortageSummary>,
}

/// Physical shortages for one settlement in a compact annual close event.
/// A row vector keeps numeric settlement IDs out of serialized map keys,
/// which are not reliably round-tripped by serde's tagged-enum content format.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementPhysicalShortageSummary {
    pub settlement_id: SettlementId,
    pub goods: BTreeMap<GoodId, Quantity>,
}

impl From<&EconomicYearReport> for EconomicYearClosedSummary {
    fn from(report: &EconomicYearReport) -> Self {
        let mut physical_shortage_totals = Vec::new();
        for settlement in &report.settlements {
            let shortages = settlement
                .goods
                .iter()
                .filter(|good| good.unmet_physical_need.0 > 0)
                .map(|good| (good.good_id.clone(), good.unmet_physical_need))
                .collect::<BTreeMap<_, _>>();
            if !shortages.is_empty() {
                physical_shortage_totals.push(SettlementPhysicalShortageSummary {
                    settlement_id: settlement.settlement_id,
                    goods: shortages,
                });
            }
        }
        physical_shortage_totals.sort_by_key(|row| row.settlement_id);
        Self {
            year: report.year,
            settlement_ids: report
                .settlements
                .iter()
                .map(|settlement| settlement.settlement_id)
                .collect(),
            group_ids: report
                .groups
                .iter()
                .map(|group| group.group_id.clone())
                .collect(),
            money: report.money.clone(),
            physical_shortage_totals,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomyStockLossRecord {
    pub event_id: LogicalId,
    pub group_id: EconomicGroupId,
    pub good_id: GoodId,
    pub requested_loss_ppm: u32,
    pub destroyed: Quantity,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockDestructionReport {
    pub losses: Vec<EconomyStockLossRecord>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EconomySignals {
    /// Physical unmet need divided by need, in ppm; absent means no configured need.
    pub scarcity_ppm_by_settlement: BTreeMap<SettlementId, u32>,
    /// Solvent unmet demand for each configured profession's output.
    pub profession_opportunities: Vec<ProfessionOpportunity>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfessionOpportunity {
    pub settlement_id: SettlementId,
    pub profession_id: ProfessionId,
    pub unmet_demand_score: Quantity,
}

/// Event-derived effects for the A4 production phase.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EconomyYearModifiers {
    pub incapacitated_person_ids: BTreeSet<PersonId>,
    /// Multiplier by group in ppm. Missing groups have baseline 1,000,000.
    pub production_capacity_ppm_by_group: BTreeMap<EconomicGroupId, u32>,
}

/// Validate economic IDs and fixed-point limits against the scenario catalogs.
pub fn validate_economic_config(
    config: &EconomicConfig,
    profession_ids: &BTreeSet<ProfessionId>,
    settlement_ids: &BTreeSet<SettlementId>,
    route_ids: &BTreeSet<LogicalId>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    for (good_id, good) in &config.goods {
        let path = format!("economy.goods.{}", good_id.0);
        if good_id.0.trim().is_empty() {
            issues.push(ValidationIssue::new(
                &path,
                "empty_id",
                "good ID must not be empty",
            ));
        }
        if good.name.trim().is_empty() || good.unit.trim().is_empty() {
            issues.push(ValidationIssue::new(
                &path,
                "missing_good_description",
                "good name and unit must not be empty",
            ));
        }
        if good.transport_units_per_unit == 0 {
            issues.push(ValidationIssue::new(
                format!("{path}.transport_units_per_unit"),
                "invalid_transport_units",
                "each unit of a good must consume positive route cargo capacity",
            ));
        }
        if good.annual_storage_loss_ppm > 1_000_000 {
            issues.push(ValidationIssue::new(
                format!("{path}.annual_storage_loss_ppm"),
                "probability_out_of_range",
                "storage loss must be between 0 and 1,000,000 ppm",
            ));
        }
        if !good.minimum_price.is_non_negative()
            || !good.initial_price.is_non_negative()
            || !good.maximum_price.is_non_negative()
            || good.minimum_price.0 == 0
            || good.minimum_price.0 > good.initial_price.0
            || good.initial_price.0 > good.maximum_price.0
        {
            issues.push(ValidationIssue::new(
                format!("{path}.price"),
                "invalid_price_bounds",
                "prices must be positive and minimum <= initial <= maximum",
            ));
        }
    }
    for (profession_id, policy) in &config.profession_policies {
        let path = format!("economy.profession_policies.{}", profession_id.0);
        if !profession_ids.contains(profession_id) {
            issues.push(ValidationIssue::new(
                &path,
                "unknown_profession",
                "profession policy must reference a declared profession",
            ));
        }
        if let Some(recipe) = &policy.recipe {
            if recipe.batches_per_worker == 0 {
                issues.push(ValidationIssue::new(
                    format!("{path}.recipe.batches_per_worker"),
                    "invalid_capacity",
                    "production batches per worker must be positive",
                ));
            }
            if recipe.outputs_per_batch.is_empty() {
                issues.push(ValidationIssue::new(
                    format!("{path}.recipe.outputs_per_batch"),
                    "missing_output",
                    "a production recipe must produce at least one good",
                ));
            }
            validate_good_quantities(
                &recipe.inputs_per_batch,
                &config.goods,
                &format!("{path}.recipe.inputs_per_batch"),
                &mut issues,
            );
            validate_good_quantities(
                &recipe.outputs_per_batch,
                &config.goods,
                &format!("{path}.recipe.outputs_per_batch"),
                &mut issues,
            );
        }
    }
    for (good_id, need) in &config.needs {
        if !config.goods.contains_key(good_id) {
            issues.push(ValidationIssue::new(
                format!("economy.needs.{}", good_id.0),
                "unknown_good",
                "needs must reference a configured good",
            ));
        }
        if need.quantity_per_person_per_year.0 < 0 {
            issues.push(ValidationIssue::new(
                format!("economy.needs.{}", good_id.0),
                "negative_quantity",
                "annual physical needs cannot be negative",
            ));
        }
    }
    for (route_id, capacity) in &config.route_goods_capacities {
        if !route_ids.contains(route_id) {
            issues.push(ValidationIssue::new(
                format!("economy.route_goods_capacities.{}", route_id.0),
                "unknown_route",
                "cargo capacity must reference a configured directed route",
            ));
        }
        if capacity.0 < 0 {
            issues.push(ValidationIssue::new(
                format!("economy.route_goods_capacities.{}", route_id.0),
                "negative_capacity",
                "route cargo capacity cannot be negative",
            ));
        }
    }
    if config.transport_money_per_route_cost_unit.0 < 0 {
        issues.push(ValidationIssue::new(
            "economy.transport_money_per_route_cost_unit",
            "negative_transport_cost",
            "transport money rate cannot be negative",
        ));
    }
    if config.price_policy.adjustment_step_ppm > 1_000_000 {
        issues.push(ValidationIssue::new(
            "economy.price_policy.adjustment_step_ppm",
            "probability_out_of_range",
            "price step must be between 0 and 1,000,000 ppm",
        ));
    }
    if config.production_response.step_ppm > 1_000_000
        || config.production_response.minimum_capacity_ppm
            > config.production_response.maximum_capacity_ppm
        || config.production_response.maximum_capacity_ppm > 5_000_000
        || (config.production_response.enabled
            && (config.production_response.step_ppm == 0
                || config.production_response.minimum_capacity_ppm > 1_000_000
                || config.production_response.maximum_capacity_ppm < 1_000_000))
    {
        issues.push(ValidationIssue::new(
            "economy.production_response",
            "invalid_capacity_response",
            "capacity response requires a positive step and ordered bounds up to 5,000,000 ppm",
        ));
    }
    if config.labor_policy.enabled && config.labor_policy.max_changes_per_settlement_per_year == 0 {
        issues.push(ValidationIssue::new(
            "economy.labor_policy.max_changes_per_settlement_per_year",
            "invalid_limit",
            "enabled labor reactions need a positive annual change limit",
        ));
    }
    if config.responses.mortality_increase_at_full_scarcity_ppm > 1_000_000
        || config.responses.birth_reduction_at_full_scarcity_ppm > 1_000_000
    {
        issues.push(ValidationIssue::new(
            "economy.responses",
            "probability_out_of_range",
            "scarcity probability modifiers must be between 0 and 1,000,000 ppm",
        ));
    }
    let mut endowment_groups = BTreeSet::new();
    for (index, entry) in config.initial_group_endowments.iter().enumerate() {
        let path = format!("economy.initial_group_endowments[{index}]");
        if !endowment_groups.insert(entry.group_id.clone()) {
            issues.push(ValidationIssue::new(
                format!("{path}.group_id"),
                "duplicate_group_endowment",
                "initial endowment may be declared only once per group",
            ));
        }
        validate_group_id(
            &entry.group_id,
            profession_ids,
            settlement_ids,
            &path,
            &mut issues,
        );
        validate_endowment(&entry.endowment, &config.goods, &path, &mut issues);
    }
    validate_endowment(
        &config.initial_treasury,
        &config.goods,
        "economy.initial_treasury",
        &mut issues,
    );
    issues
}

fn validate_group_id(
    group_id: &EconomicGroupId,
    profession_ids: &BTreeSet<ProfessionId>,
    settlement_ids: &BTreeSet<SettlementId>,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    if !settlement_ids.contains(&group_id.settlement_id) {
        issues.push(ValidationIssue::new(
            format!("{path}.group_id.settlement_id"),
            "unknown_settlement",
            "economic group must reference a configured settlement",
        ));
    }
    if group_id
        .profession_id
        .as_ref()
        .is_some_and(|profession| !profession_ids.contains(profession))
    {
        issues.push(ValidationIssue::new(
            format!("{path}.group_id.profession_id"),
            "unknown_profession",
            "economic group must reference a configured profession",
        ));
    }
}

fn validate_endowment(
    endowment: &EconomicGroupEndowment,
    goods: &BTreeMap<GoodId, GoodDefinition>,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    if endowment.money.0 < 0 {
        issues.push(ValidationIssue::new(
            format!("{path}.money"),
            "negative_money",
            "initial balances cannot be negative",
        ));
    }
    for (good_id, quantity) in &endowment.goods {
        if !goods.contains_key(good_id) {
            issues.push(ValidationIssue::new(
                format!("{path}.goods.{}", good_id.0),
                "unknown_good",
                "initial stock must reference a configured good",
            ));
        }
        if quantity.0 < 0 {
            issues.push(ValidationIssue::new(
                format!("{path}.goods.{}", good_id.0),
                "negative_quantity",
                "initial stock cannot be negative",
            ));
        }
    }
}

fn validate_good_quantities(
    quantities: &BTreeMap<GoodId, Quantity>,
    goods: &BTreeMap<GoodId, GoodDefinition>,
    path: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    for (good_id, quantity) in quantities {
        if !goods.contains_key(good_id) {
            issues.push(ValidationIssue::new(
                format!("{path}.{}", good_id.0),
                "unknown_good",
                "recipe must reference a configured good",
            ));
        }
        if quantity.0 <= 0 {
            issues.push(ValidationIssue::new(
                format!("{path}.{}", good_id.0),
                "invalid_quantity",
                "recipe quantities must be positive",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> EconomicConfig {
        let bread = GoodId::from("bread");
        EconomicConfig {
            goods: BTreeMap::from([(
                bread.clone(),
                GoodDefinition {
                    name: "Bread".into(),
                    unit: "loaf".into(),
                    transport_units_per_unit: 1,
                    storable: true,
                    annual_storage_loss_ppm: 0,
                    initial_price: Money(2),
                    minimum_price: Money(1),
                    maximum_price: Money(10),
                },
            )]),
            needs: BTreeMap::from([(
                bread,
                GoodNeed {
                    quantity_per_person_per_year: Quantity(1),
                    priority: 0,
                },
            )]),
            ..EconomicConfig::default()
        }
    }

    #[test]
    fn ruleset_one_empty_economy_state_is_omittable() {
        let state = EconomyState::default();
        assert!(state.is_empty());
        let value = serde_json::to_value(&state).unwrap();
        assert_eq!(value["groups"], serde_json::json!([]));
    }

    #[test]
    fn validates_catalog_references_and_units() {
        let mut config = valid_config();
        config
            .route_goods_capacities
            .insert(LogicalId(99), Quantity(2));
        let issues = validate_economic_config(
            &config,
            &BTreeSet::new(),
            &BTreeSet::from([SettlementId(1)]),
            &BTreeSet::new(),
        );
        assert!(issues.iter().any(|issue| issue.code == "unknown_route"));
        assert!(issues
            .iter()
            .all(|issue| issue.code != "missing_good_description"));
    }

    #[test]
    fn group_ids_are_components_not_encoded_map_keys() {
        let group = EconomicGroupId {
            settlement_id: SettlementId(3),
            profession_id: Some(ProfessionId::from("farmer::east")),
        };
        let json = serde_json::to_string(&group).unwrap();
        assert_eq!(
            serde_json::from_str::<EconomicGroupId>(&json).unwrap(),
            group
        );
        assert!(json.contains("farmer::east"));
    }

    #[test]
    fn economic_close_event_round_trips_shortages_with_numeric_settlement_ids() {
        let kind = crate::HistoricalEventKind::EconomicYearClosed {
            summary: EconomicYearClosedSummary {
                year: Year(7),
                settlement_ids: vec![SettlementId(2)],
                group_ids: Vec::new(),
                money: EconomicMoneyLedger::default(),
                physical_shortage_totals: vec![SettlementPhysicalShortageSummary {
                    settlement_id: SettlementId(2),
                    goods: BTreeMap::from([(GoodId::from("grain"), Quantity(4))]),
                }],
            },
        };

        let json = serde_json::to_string(&kind).unwrap();
        let decoded: crate::HistoricalEventKind = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, kind);
    }
}
