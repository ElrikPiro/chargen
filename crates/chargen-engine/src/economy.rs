//! Integer-only F08 economy. The public hooks correspond to annual phases so
//! scheduled event damage and population changes remain inside the engine's
//! annual transaction.

use crate::{keyed_random_u64, DecisionDomain};
use chargen_domain::{
    BuyerPriorityPolicy, EconomicConfig, EconomicGoodLedger, EconomicGroupId, EconomicGroupLedger,
    EconomicGroupState, EconomicLaborPolicy, EconomicPricePolicy, EconomicProductionCapacity,
    EconomicProfessionChange, EconomicTrade, EconomicYearReport, EconomySignals, EconomyState,
    EconomyStockLossRecord, EconomyYearModifiers, EmptyGroupDisposition, EntityRef, GoodDefinition,
    GoodId, HistoricalEvent, HistoricalEventKind, Money, Person, ProfessionChangeReason,
    ProfessionEconomicPolicy, ProfessionId, ProfessionOpportunity, Quantity,
    SettlementEconomicLedger, SettlementGoodMarketLedger, SettlementId, State,
    StockDestructionReport, TerritoryConfig, TradePurpose, TransportCostAccount, Year,
    YearInterval,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const PPM: i64 = 1_000_000;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EconomyError {
    #[error("economic state is invalid: {0}")]
    InvalidState(String),
    #[error("economic configuration is invalid: {0}")]
    InvalidConfig(String),
    #[error("economic arithmetic overflow or underflow in {0}")]
    Arithmetic(&'static str),
    #[error("economic phase order is invalid: {0}")]
    PhaseOrder(&'static str),
}

type EconomyResult<T> = Result<T, EconomyError>;
type MarketSupply = BTreeMap<(EconomicGroupId, GoodId), i64>;
type MarketDemand = BTreeMap<(EconomicGroupId, GoodId, TradePurpose), i64>;

/// Initializes prices, treasury and one-time group endowments after the
/// scenario's initial people and occupations have been materialized.
pub fn initialize_economy(
    config: &EconomicConfig,
    state: &mut State,
    year: Year,
) -> EconomyResult<()> {
    if !state.economy.is_empty() {
        return Err(EconomyError::InvalidState(
            "economy must be empty before initial materialization".into(),
        ));
    }
    if config.transport_money_per_route_cost_unit.0 < 0 {
        return Err(EconomyError::InvalidConfig(
            "transport rate cannot be negative".into(),
        ));
    }
    let mut economy = EconomyState {
        treasury: config.initial_treasury.clone(),
        ..EconomyState::default()
    };
    for settlement_id in state.settlements.keys().copied() {
        let prices = config
            .goods
            .iter()
            .map(|(good_id, definition)| (good_id.clone(), definition.initial_price))
            .collect();
        economy.prices_by_settlement.insert(settlement_id, prices);
    }
    for entry in &config.initial_group_endowments {
        let group = ensure_group(&mut economy, &entry.group_id);
        group.money = entry.endowment.money;
        group.goods = entry.endowment.goods.clone();
    }
    state.economy = economy;
    reconcile_economic_groups(config, state, year)?;
    Ok(())
}

/// Reconciles member and worker counts, assignment indexes, and configured
/// asset continuity in one batch. New groups never receive initial endowments.
pub fn reconcile_economic_groups(
    config: &EconomicConfig,
    state: &mut State,
    year: Year,
) -> EconomyResult<()> {
    let old_assignments = state.economy.person_groups.clone();
    let old_member_counts: BTreeMap<_, _> = state
        .economy
        .groups
        .iter()
        .map(|group| (group.group_id.clone(), group.member_count))
        .collect();

    let mut counts: BTreeMap<EconomicGroupId, GroupCounts> = BTreeMap::new();
    let mut assignments = BTreeMap::new();
    for person in state.living_people_at(year) {
        let group_id = EconomicGroupId {
            settlement_id: person.settlement_id,
            profession_id: person.profession.clone(),
        };
        let entry = counts.entry(group_id.clone()).or_default();
        entry.members = entry
            .members
            .checked_add(1)
            .ok_or(EconomyError::Arithmetic("member counts"))?;
        if is_worker(config.labor_policy, person, year) {
            entry.workers = entry
                .workers
                .checked_add(1)
                .ok_or(EconomyError::Arithmetic("worker counts"))?;
        }
        assignments.insert(person.id, group_id);
    }

    let mut moves: BTreeMap<EconomicGroupId, BTreeMap<EconomicGroupId, u64>> = BTreeMap::new();
    for (person_id, new_group) in &assignments {
        if let Some(old_group) = old_assignments.get(person_id) {
            if old_group != new_group {
                let destination_count = moves
                    .entry(old_group.clone())
                    .or_default()
                    .entry(new_group.clone())
                    .or_default();
                *destination_count = destination_count
                    .checked_add(1)
                    .ok_or(EconomyError::Arithmetic("group movements"))?;
            }
        }
    }

    let mut all_ids: BTreeSet<_> = state
        .economy
        .groups
        .iter()
        .map(|group| group.group_id.clone())
        .collect();
    all_ids.extend(counts.keys().cloned());
    all_ids.extend(
        config
            .initial_group_endowments
            .iter()
            .map(|e| e.group_id.clone()),
    );
    for group_id in assignments.values() {
        if !all_ids.contains(group_id) {
            all_ids.insert(group_id.clone());
        }
    }
    for group_id in &all_ids {
        ensure_group(&mut state.economy, group_id);
    }

    if config.asset_continuity == chargen_domain::AssetContinuity::ProportionalShareTransfer {
        transfer_departing_assets(state, &moves, &old_member_counts)?;
    }

    state.economy.person_groups = assignments;
    for group in &mut state.economy.groups {
        let count = counts.get(&group.group_id).copied().unwrap_or_default();
        group.member_count = count.members;
        group.worker_count = count.workers;
    }

    apply_empty_group_disposition(config, state)?;
    refresh_working_group_counts(state)?;
    debug_assert!(state
        .economy
        .groups
        .windows(2)
        .all(|pair| pair[0].group_id < pair[1].group_id));
    Ok(())
}

/// A0 installs prices and production capacity chosen at the prior A6, then
/// applies the bounded profession changes chosen by prior demand signals.
pub fn begin_economy_year(
    config: &EconomicConfig,
    state: &mut State,
    territory: Option<&TerritoryConfig>,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
) -> EconomyResult<()> {
    if state.economy.working_report.is_some() {
        return Err(EconomyError::PhaseOrder(
            "A0 began with an open annual ledger",
        ));
    }
    for (settlement_id, prices) in std::mem::take(&mut state.economy.pending_prices) {
        let current = state
            .economy
            .prices_by_settlement
            .entry(settlement_id)
            .or_default();
        for (good_id, price) in prices {
            let price = config
                .goods
                .get(&good_id)
                .map_or(price, |definition| clamp_price(price, definition));
            current.insert(good_id, price);
        }
    }
    // A child branch may tighten policy bounds after the parent's last A6.
    // Clamp inherited prices at the first A0 under the new config, before the
    // annual ledger captures opening prices or any market can clear.
    for prices in state.economy.prices_by_settlement.values_mut() {
        for (good_id, definition) in &config.goods {
            if let Some(price) = prices.get_mut(good_id) {
                *price = clamp_price(*price, definition);
            }
        }
    }

    let pending_capacities =
        std::mem::take(&mut state.economy.pending_production_capacity_ppm_by_group);
    if config.production_response.enabled {
        for capacity in &mut state.economy.production_capacity_ppm_by_group {
            capacity.capacity_ppm = capacity
                .capacity_ppm
                .max(config.production_response.minimum_capacity_ppm)
                .min(config.production_response.maximum_capacity_ppm);
        }
    }
    for mut capacity in pending_capacities {
        if config.production_response.enabled {
            capacity.capacity_ppm = capacity
                .capacity_ppm
                .max(config.production_response.minimum_capacity_ppm)
                .min(config.production_response.maximum_capacity_ppm);
        }
        set_capacity(&mut state.economy, capacity);
    }
    apply_pending_profession_changes(config, state, territory, year, events)?;

    let mut report = EconomicYearReport {
        year,
        ..EconomicYearReport::default()
    };
    report.money.opening_group_money = sum_group_money(&state.economy.groups)?;
    report.money.opening_treasury_money = state.economy.treasury.money;
    for good_id in config.goods.keys() {
        let stock = state
            .economy
            .treasury
            .goods
            .get(good_id)
            .copied()
            .unwrap_or_default();
        report.treasury_goods.insert(
            good_id.clone(),
            EconomicGoodLedger {
                opening_stock: stock,
                closing_stock: stock,
                ..EconomicGoodLedger::default()
            },
        );
    }
    for settlement_id in state.settlements.keys().copied() {
        let mut settlement = SettlementEconomicLedger {
            settlement_id,
            goods: Vec::with_capacity(config.goods.len()),
        };
        for (good_id, definition) in &config.goods {
            let price = current_price(&state.economy, settlement_id, good_id, definition);
            settlement.goods.push(SettlementGoodMarketLedger {
                good_id: good_id.clone(),
                opening_price: price,
                next_price: price,
                ..SettlementGoodMarketLedger::default()
            });
        }
        report.settlements.push(settlement);
    }
    for group in &state.economy.groups {
        report.groups.push(new_group_ledger(group, config));
    }
    state.economy.working_report = Some(report);
    Ok(())
}

/// Applies an event's one-time stock loss to raw inventories before A4.
/// Loss is floored per group and good and recorded with its cause.
pub fn apply_stock_destruction(
    config: &EconomicConfig,
    state: &mut State,
    _year: Year,
    event_id: chargen_domain::LogicalId,
    settlement_scope: Option<SettlementId>,
    profession_scope: Option<&BTreeSet<ProfessionId>>,
    loss_ppm: u32,
) -> EconomyResult<StockDestructionReport> {
    if loss_ppm > 1_000_000 {
        return Err(EconomyError::InvalidConfig(
            "stock loss must be at most 1,000,000 ppm".into(),
        ));
    }
    ensure_working_report(state)?;
    let mut losses = Vec::new();
    for index in 0..state.economy.groups.len() {
        let group_id = state.economy.groups[index].group_id.clone();
        if settlement_scope.is_some_and(|scope| group_id.settlement_id != scope)
            || profession_scope.is_some_and(|scope| {
                group_id
                    .profession_id
                    .as_ref()
                    .is_none_or(|profession| !scope.contains(profession))
            })
        {
            continue;
        }
        let good_ids: Vec<_> = state.economy.groups[index]
            .goods
            .keys()
            .filter(|good_id| config.goods.contains_key(*good_id))
            .cloned()
            .collect();
        for good_id in good_ids {
            let stock = state.economy.groups[index]
                .goods
                .get(&good_id)
                .copied()
                .unwrap_or_default();
            let destroyed = Quantity(mul_div_floor(stock.0, i64::from(loss_ppm), PPM)?);
            if destroyed.0 == 0 {
                continue;
            }
            let remaining = sub_quantity(stock, destroyed, "stock destruction")?;
            state.economy.groups[index]
                .goods
                .insert(good_id.clone(), remaining);
            let record = EconomyStockLossRecord {
                event_id,
                group_id: group_id.clone(),
                good_id: good_id.clone(),
                requested_loss_ppm: loss_ppm,
                destroyed,
            };
            if let Some(ledger) = group_ledger_mut(state, &group_id) {
                let good = ledger.goods.entry(good_id).or_default();
                good.event_destroyed =
                    add_quantity(good.event_destroyed, destroyed, "event loss ledger")?;
            }
            state
                .economy
                .working_report
                .as_mut()
                .expect("checked report")
                .stock_losses
                .push(record.clone());
            losses.push(record);
        }
    }
    Ok(StockDestructionReport { losses })
}

/// A4 production: recipe batches are limited by eligible workers, event
/// capacity loss, next-year group response and available integer inputs.
pub fn produce_economy(
    config: &EconomicConfig,
    state: &mut State,
    year: Year,
    modifiers: &EconomyYearModifiers,
) -> EconomyResult<()> {
    ensure_working_report(state)?;
    let group_indexes = index_groups(&state.economy.groups);
    let mut incapacitated_workers = BTreeMap::<EconomicGroupId, u64>::new();
    for person_id in &modifiers.incapacitated_person_ids {
        let Some(person) = state.persons.get(person_id) else {
            continue;
        };
        if !is_worker(config.labor_policy, person, year) || !person.alive_at_close(year) {
            continue;
        }
        if let Some(group_id) = state.economy.person_groups.get(person_id) {
            let workers = incapacitated_workers.entry(group_id.clone()).or_default();
            *workers = workers
                .checked_add(1)
                .ok_or(EconomyError::Arithmetic("incapacitated worker count"))?;
        }
    }
    let group_ids: Vec<_> = group_indexes.keys().cloned().collect();
    for group_id in group_ids {
        let Some(profession_id) = group_id.profession_id.clone() else {
            continue;
        };
        let Some(policy) = config.profession_policies.get(&profession_id) else {
            continue;
        };
        let Some(recipe) = &policy.recipe else {
            continue;
        };
        let index = group_indexes[&group_id];
        let workers = state.economy.groups[index].worker_count.saturating_sub(
            incapacitated_workers
                .get(&group_id)
                .copied()
                .unwrap_or_default(),
        );
        if workers == 0 {
            continue;
        }
        let event_capacity = modifiers
            .production_capacity_ppm_by_group
            .get(&group_id)
            .copied()
            .unwrap_or(1_000_000);
        if event_capacity > 1_000_000 {
            return Err(EconomyError::InvalidConfig(
                "event production multiplier cannot exceed 1,000,000 ppm".into(),
            ));
        }
        let response_capacity = capacity_for(&state.economy, &group_id);
        let capacity_ppm =
            mul_div_floor(i64::from(event_capacity), i64::from(response_capacity), PPM)?;
        let mut batches = u64::try_from(mul_div_floor(
            i64::try_from(
                workers
                    .checked_mul(u64::from(recipe.batches_per_worker))
                    .ok_or(EconomyError::Arithmetic("production capacity"))?,
            )
            .map_err(|_| EconomyError::Arithmetic("production capacity"))?,
            capacity_ppm,
            PPM,
        )?)
        .map_err(|_| EconomyError::Arithmetic("production capacity"))?;
        if let Some(maximum) = recipe.maximum_batches_per_year {
            batches = batches.min(maximum);
        }
        for (good_id, per_batch) in &recipe.inputs_per_batch {
            let stock = state.economy.groups[index]
                .goods
                .get(good_id)
                .copied()
                .unwrap_or_default()
                .0;
            let possible = u64::try_from(stock / per_batch.0)
                .map_err(|_| EconomyError::Arithmetic("input-limited production"))?;
            batches = batches.min(possible);
        }
        if batches == 0 {
            continue;
        }
        for (good_id, per_batch) in &recipe.inputs_per_batch {
            let amount = multiply_quantity(*per_batch, batches, "production inputs")?;
            let old = state.economy.groups[index]
                .goods
                .get(good_id)
                .copied()
                .unwrap_or_default();
            state.economy.groups[index].goods.insert(
                good_id.clone(),
                sub_quantity(old, amount, "production inputs")?,
            );
            let good_ledger = group_ledger_mut(state, &group_id)
                .ok_or(EconomyError::PhaseOrder("missing group ledger"))?
                .goods
                .entry(good_id.clone())
                .or_default();
            good_ledger.input_consumed = add_quantity(
                good_ledger.input_consumed,
                amount,
                "production input ledger",
            )?;
        }
        for (good_id, per_batch) in &recipe.outputs_per_batch {
            let amount = multiply_quantity(*per_batch, batches, "production output")?;
            let old = state.economy.groups[index]
                .goods
                .get(good_id)
                .copied()
                .unwrap_or_default();
            state.economy.groups[index].goods.insert(
                good_id.clone(),
                add_quantity(old, amount, "production output")?,
            );
            let good_ledger = group_ledger_mut(state, &group_id)
                .ok_or(EconomyError::PhaseOrder("missing group ledger"))?
                .goods
                .entry(good_id.clone())
                .or_default();
            good_ledger.produced =
                add_quantity(good_ledger.produced, amount, "production output ledger")?;
        }
    }
    let _ = year;
    Ok(())
}

/// A5 local clearing and directed-route trade. Human needs have priority over
/// input replenishment; all buyer budgets, seller stocks, and route cargo are
/// reserved exactly once in deterministic order.
pub fn trade_economy(
    config: &EconomicConfig,
    state: &mut State,
    territory: Option<&TerritoryConfig>,
    year: Year,
) -> EconomyResult<()> {
    ensure_working_report(state)?;
    let (mut supply, mut demand) = market_orders(config, state, year)?;
    let mut group_order: Vec<_> = state
        .economy
        .groups
        .iter()
        .map(|group| group.group_id.clone())
        .collect();
    if config.buyer_priority == BuyerPriorityPolicy::GroupKeyOrder {
        group_order.sort();
    } else {
        group_order.sort_by_key(|id| buyer_rank(config, state, id, year).unwrap_or(u64::MAX));
    }
    let rank: BTreeMap<_, _> = group_order
        .iter()
        .enumerate()
        .map(|(index, group)| (group.clone(), index))
        .collect();

    for (settlement_id, good_id) in all_settlement_goods(state, config) {
        let sellers: Vec<_> = supply
            .keys()
            .filter(|(group, good)| group.settlement_id == settlement_id && *good == good_id)
            .map(|(group, _)| group.clone())
            .collect();
        let buyers: Vec<_> = demand
            .keys()
            .filter(|(group, good, _)| group.settlement_id == settlement_id && *good == good_id)
            .map(|(group, _, purpose)| (group.clone(), *purpose))
            .collect();
        let mut sellers = sellers;
        let mut buyers = buyers;
        sellers.sort_by_key(|group| rank.get(group).copied().unwrap_or(usize::MAX));
        buyers.sort_by_key(|(group, purpose)| {
            (
                rank.get(group).copied().unwrap_or(usize::MAX),
                purpose_order(*purpose),
            )
        });
        for (buyer_id, purpose) in buyers {
            for seller_id in &sellers {
                if seller_id == &buyer_id {
                    continue;
                }
                let available = supply
                    .get(&(seller_id.clone(), good_id.clone()))
                    .copied()
                    .unwrap_or_default();
                let wanted = demand
                    .get(&(buyer_id.clone(), good_id.clone(), purpose))
                    .copied()
                    .unwrap_or_default();
                if available <= 0 || wanted <= 0 {
                    continue;
                }
                let price = current_price(
                    &state.economy,
                    settlement_id,
                    &good_id,
                    &config.goods[&good_id],
                );
                let buyer_money = group_ref(state, &buyer_id)
                    .ok_or(EconomyError::InvalidState("buyer group missing".into()))?
                    .money;
                let affordable = if price.0 == 0 {
                    wanted
                } else {
                    buyer_money.0 / price.0
                };
                let quantity = available.min(wanted).min(affordable).max(0);
                if quantity == 0 {
                    continue;
                }
                let revenue = money_product(price, quantity, "local trade value")?;
                execute_trade(
                    state,
                    TradePlan {
                        seller_id,
                        buyer_id: &buyer_id,
                        good_id: &good_id,
                        quantity,
                        seller_revenue: revenue,
                        buyer_payment: revenue,
                        transport_cost: Money(0),
                        route_id: None,
                        purpose,
                        transport_account: config.transport_cost_account,
                    },
                )?;
                *supply
                    .get_mut(&(seller_id.clone(), good_id.clone()))
                    .expect("seller order") -= quantity;
                *demand
                    .get_mut(&(buyer_id.clone(), good_id.clone(), purpose))
                    .expect("buyer order") -= quantity;
            }
        }
    }

    let Some(territory) = territory else {
        record_unsold_offers(state, &supply)?;
        record_unmet_input_targets(config, state, year)?;
        return Ok(());
    };
    let mut routes: Vec<_> = territory.routes.iter().collect();
    routes.sort_by_key(|route| route.logical_id);
    for route in routes {
        if !route.open_intervals.is_empty()
            && !route
                .open_intervals
                .iter()
                .any(|interval| interval.contains(year))
        {
            continue;
        }
        if state.route_is_closed(route.logical_id, year) {
            continue;
        }
        let mut cargo_left = config
            .route_goods_capacities
            .get(&route.logical_id)
            .copied()
            .unwrap_or_default()
            .0;
        if cargo_left <= 0 {
            continue;
        }
        for (good_id, definition) in &config.goods {
            let sellers: Vec<_> = supply
                .keys()
                .filter(|(group, good)| group.settlement_id == route.from && *good == *good_id)
                .map(|(group, _)| group.clone())
                .collect();
            let buyers: Vec<_> = demand
                .keys()
                .filter(|(group, good, _)| group.settlement_id == route.to && *good == *good_id)
                .map(|(group, _, purpose)| (group.clone(), *purpose))
                .collect();
            let mut sellers = sellers;
            let mut buyers = buyers;
            sellers.sort_by_key(|group| rank.get(group).copied().unwrap_or(usize::MAX));
            buyers.sort_by_key(|(group, purpose)| {
                (
                    rank.get(group).copied().unwrap_or(usize::MAX),
                    purpose_order(*purpose),
                )
            });
            let from_price = current_price(&state.economy, route.from, good_id, definition);
            let to_price = current_price(&state.economy, route.to, good_id, definition);
            let cargo_per_unit = i64::from(definition.transport_units_per_unit);
            let fee_per_unit = i64::from(route.cost)
                .checked_mul(config.transport_money_per_route_cost_unit.0)
                .and_then(|fee| fee.checked_mul(cargo_per_unit))
                .ok_or(EconomyError::Arithmetic("route transport price"))?;
            let landed = from_price
                .0
                .checked_add(fee_per_unit)
                .ok_or(EconomyError::Arithmetic("landed price"))?;
            if landed >= to_price.0 {
                continue;
            }
            for (buyer_id, purpose) in buyers {
                if cargo_left < cargo_per_unit {
                    break;
                }
                for seller_id in &sellers {
                    if seller_id == &buyer_id || cargo_left < cargo_per_unit {
                        continue;
                    }
                    let available = supply
                        .get(&(seller_id.clone(), good_id.clone()))
                        .copied()
                        .unwrap_or_default();
                    let wanted = demand
                        .get(&(buyer_id.clone(), good_id.clone(), purpose))
                        .copied()
                        .unwrap_or_default();
                    if available <= 0 || wanted <= 0 {
                        continue;
                    }
                    let max_by_cargo = cargo_left / cargo_per_unit;
                    let buyer_money = group_ref(state, &buyer_id)
                        .ok_or(EconomyError::InvalidState("buyer group missing".into()))?
                        .money;
                    let max_by_money = if landed == 0 {
                        wanted
                    } else {
                        buyer_money.0 / landed
                    };
                    let quantity = available
                        .min(wanted)
                        .min(max_by_cargo)
                        .min(max_by_money)
                        .max(0);
                    if quantity == 0 {
                        continue;
                    }
                    let seller_revenue =
                        money_product(from_price, quantity, "route seller income")?;
                    let transport_cost =
                        money_product(Money(fee_per_unit), quantity, "route transport charges")?;
                    let buyer_payment =
                        add_money(seller_revenue, transport_cost, "route buyer payment")?;
                    execute_trade(
                        state,
                        TradePlan {
                            seller_id,
                            buyer_id: &buyer_id,
                            good_id,
                            quantity,
                            seller_revenue,
                            buyer_payment,
                            transport_cost,
                            route_id: Some(route.logical_id),
                            purpose,
                            transport_account: config.transport_cost_account,
                        },
                    )?;
                    *supply
                        .get_mut(&(seller_id.clone(), good_id.clone()))
                        .expect("seller order") -= quantity;
                    *demand
                        .get_mut(&(buyer_id.clone(), good_id.clone(), purpose))
                        .expect("buyer order") -= quantity;
                    let reserved_cargo = quantity
                        .checked_mul(cargo_per_unit)
                        .ok_or(EconomyError::Arithmetic("route cargo reservation"))?;
                    cargo_left = cargo_left
                        .checked_sub(reserved_cargo)
                        .ok_or(EconomyError::Arithmetic("route cargo reservation"))?;
                }
            }
        }
    }
    record_unsold_offers(state, &supply)?;
    record_unmet_input_targets(config, state, year)?;
    Ok(())
}

/// A6 consumes human needs, records physical and solvent shortages separately,
/// applies storage loss, and schedules prices/capacity for the following year.
pub fn consume_and_adjust_economy(
    config: &EconomicConfig,
    state: &mut State,
    territory: Option<&TerritoryConfig>,
    year: Year,
) -> EconomyResult<EconomySignals> {
    ensure_working_report(state)?;
    let group_ids: Vec<_> = state
        .economy
        .groups
        .iter()
        .map(|group| group.group_id.clone())
        .collect();
    let mut total_need: BTreeMap<(SettlementId, GoodId), i64> = BTreeMap::new();
    let mut total_unmet: BTreeMap<(SettlementId, GoodId), i64> = BTreeMap::new();
    let mut affordable_human_demand: BTreeMap<(SettlementId, GoodId), i64> = BTreeMap::new();
    let mut purchased_human: BTreeMap<(SettlementId, GoodId), i64> = BTreeMap::new();

    for group_id in &group_ids {
        let group = group_ref(state, group_id)
            .ok_or(EconomyError::InvalidState("group disappeared".into()))?
            .clone();
        let group_index = state
            .economy
            .groups
            .binary_search_by(|entry| entry.group_id.cmp(group_id))
            .map_err(|_| EconomyError::InvalidState("unsorted economic groups".into()))?;
        for (good_id, need) in &config.needs {
            let requested = multiply_count(need.quantity_per_person_per_year, group.member_count)?;
            let stock = state.economy.groups[group_index]
                .goods
                .get(good_id)
                .copied()
                .unwrap_or_default();
            let consumed = Quantity(requested.0.min(stock.0));
            let shortage = sub_quantity(requested, consumed, "human need shortage")?;
            let remaining = sub_quantity(stock, consumed, "human consumption")?;
            state.economy.groups[group_index]
                .goods
                .insert(good_id.clone(), remaining);
            let ledger = group_ledger_mut(state, group_id)
                .ok_or(EconomyError::PhaseOrder("missing consumer ledger"))?
                .goods
                .entry(good_id.clone())
                .or_default();
            ledger.physical_need = add_quantity(ledger.physical_need, requested, "need ledger")?;
            ledger.consumer_consumed =
                add_quantity(ledger.consumer_consumed, consumed, "consumption ledger")?;
            ledger.unmet_physical_need =
                add_quantity(ledger.unmet_physical_need, shortage, "shortage ledger")?;
            let need_total = total_need
                .entry((group_id.settlement_id, good_id.clone()))
                .or_default();
            *need_total = need_total
                .checked_add(requested.0)
                .ok_or(EconomyError::Arithmetic("settlement need"))?;
            let unmet_total = total_unmet
                .entry((group_id.settlement_id, good_id.clone()))
                .or_default();
            *unmet_total = unmet_total
                .checked_add(shortage.0)
                .ok_or(EconomyError::Arithmetic("settlement shortage"))?;
        }
        for (good_id, ledger) in &group_ledger_ref(state, group_id)
            .ok_or(EconomyError::PhaseOrder("missing group ledger"))?
            .goods
        {
            let purchased = ledger.bought.0;
            if config.needs.contains_key(good_id) {
                let purchases = purchased_human
                    .entry((group_id.settlement_id, good_id.clone()))
                    .or_default();
                *purchases = purchases
                    .checked_add(purchased.min(ledger.solvent_demand.0))
                    .ok_or(EconomyError::Arithmetic("human purchases"))?;
                let solvent = affordable_human_demand
                    .entry((group_id.settlement_id, good_id.clone()))
                    .or_default();
                *solvent = solvent
                    .checked_add(ledger.solvent_demand.0)
                    .ok_or(EconomyError::Arithmetic("human solvent demand"))?;
            }
        }
        let _ = group;
    }

    apply_storage_loss(config, state)?;
    let mut signals = EconomySignals::default();
    for ((settlement_id, good_id), physical_need) in &total_need {
        let shortage = total_unmet
            .get(&(*settlement_id, good_id.clone()))
            .copied()
            .unwrap_or_default();
        let settlement_ledger = settlement_good_ledger_mut(state, *settlement_id, good_id)
            .ok_or(EconomyError::PhaseOrder("missing market ledger"))?;
        settlement_ledger.physical_need = Quantity(*physical_need);
        settlement_ledger.unmet_physical_need = Quantity(shortage);
        settlement_ledger.solvent_demand = Quantity(
            affordable_human_demand
                .get(&(*settlement_id, good_id.clone()))
                .copied()
                .unwrap_or_default(),
        );
        settlement_ledger.unmet_solvent_demand = Quantity(
            settlement_ledger.solvent_demand.0.saturating_sub(
                purchased_human
                    .get(&(*settlement_id, good_id.clone()))
                    .copied()
                    .unwrap_or_default(),
            ),
        );
    }
    for settlement_id in state.settlements.keys().copied() {
        let need_sum: i128 = total_need
            .iter()
            .filter(|((settlement, _), _)| *settlement == settlement_id)
            .map(|(_, amount)| i128::from(*amount))
            .sum();
        let shortage_sum: i128 = total_unmet
            .iter()
            .filter(|((settlement, _), _)| *settlement == settlement_id)
            .map(|(_, amount)| i128::from(*amount))
            .sum();
        let scarcity = if need_sum == 0 {
            0
        } else {
            u32::try_from((shortage_sum * i128::from(PPM) / need_sum).clamp(0, i128::from(PPM)))
                .map_err(|_| EconomyError::Arithmetic("scarcity signal"))?
        };
        if need_sum > 0 {
            signals
                .scarcity_ppm_by_settlement
                .insert(settlement_id, scarcity);
        }
    }
    let settlements: Vec<_> = state.settlements.keys().copied().collect();
    let goods: Vec<_> = config.goods.keys().cloned().collect();
    for settlement_id in settlements {
        for good_id in &goods {
            update_settlement_ending_stock(state, settlement_id, good_id)?;
        }
    }
    set_price_reactions(config, state)?;
    set_capacity_reactions(config, state)?;
    set_profession_reactions(config, state, territory, year, &mut signals)?;
    update_closing_balances(state)?;
    state
        .economy
        .working_report
        .as_mut()
        .expect("checked report")
        .signals = signals.clone();
    Ok(signals)
}

/// Final A11 accounting after births, movement and any associated continuity
/// transfer. Validates each group's goods and world-money conservation.
pub fn finalize_economy_year(
    config: &EconomicConfig,
    state: &mut State,
    year: Year,
) -> EconomyResult<EconomicYearReport> {
    ensure_working_report(state)?;
    if state.economy.working_report.as_ref().unwrap().year != year {
        return Err(EconomyError::PhaseOrder("final year differs from A0"));
    }
    refresh_working_group_counts(state)?;
    let groups = state.economy.groups.clone();
    for group in &groups {
        let ledger = group_ledger_mut(state, &group.group_id)
            .ok_or(EconomyError::PhaseOrder("missing final group ledger"))?;
        ledger.closing_money = group.money;
        for good_id in config.goods.keys() {
            let ending = group.goods.get(good_id).copied().unwrap_or_default();
            ledger
                .goods
                .entry(good_id.clone())
                .or_default()
                .closing_stock = ending;
        }
    }
    let settlement_ids: Vec<_> = state.settlements.keys().copied().collect();
    let good_ids: Vec<_> = config.goods.keys().cloned().collect();
    for settlement_id in settlement_ids {
        for good_id in &good_ids {
            update_settlement_ending_stock(state, settlement_id, good_id)?;
        }
    }
    for good_id in config.goods.keys() {
        let stock = state
            .economy
            .treasury
            .goods
            .get(good_id)
            .copied()
            .unwrap_or_default();
        let ledger = state
            .economy
            .working_report
            .as_mut()
            .ok_or(EconomyError::PhaseOrder("missing treasury ledger"))?
            .treasury_goods
            .entry(good_id.clone())
            .or_default();
        ledger.closing_stock = stock;
    }
    update_closing_balances(state)?;
    let report = state
        .economy
        .working_report
        .take()
        .ok_or(EconomyError::PhaseOrder("missing annual report at A11"))?;
    state.economy.last_year_report = Some(report.clone());
    validate_economy_state(config, state, year)?;
    Ok(report)
}

/// Validates sorted groups, assignments, non-negative raw balances, configured
/// capacities, and the closed year's independent goods/money ledgers.
pub fn validate_economy_state(
    config: &EconomicConfig,
    state: &State,
    year: Year,
) -> EconomyResult<()> {
    if !state
        .economy
        .groups
        .windows(2)
        .all(|pair| pair[0].group_id < pair[1].group_id)
    {
        return Err(EconomyError::InvalidState(
            "economic groups must be unique and sorted by ID".into(),
        ));
    }
    let mut member_counts = BTreeMap::<EconomicGroupId, u64>::new();
    for person in state.living_people_at(year) {
        let expected = EconomicGroupId {
            settlement_id: person.settlement_id,
            profession_id: person.profession.clone(),
        };
        if state.economy.person_groups.get(&person.id) != Some(&expected) {
            return Err(EconomyError::InvalidState(format!(
                "person {} has a stale economic assignment",
                person.id.0
            )));
        }
        let count = member_counts.entry(expected).or_default();
        *count = count
            .checked_add(1)
            .ok_or(EconomyError::Arithmetic("validation member count"))?;
    }
    for group in &state.economy.groups {
        if group.money.0 < 0 || group.goods.values().any(|amount| amount.0 < 0) {
            return Err(EconomyError::InvalidState(format!(
                "group at settlement {} has a negative balance",
                group.group_id.settlement_id.0
            )));
        }
        if group.member_count
            != member_counts
                .get(&group.group_id)
                .copied()
                .unwrap_or_default()
        {
            return Err(EconomyError::InvalidState(
                "stored group member count does not match living assignments".into(),
            ));
        }
        if group
            .goods
            .keys()
            .any(|good| !config.goods.contains_key(good))
        {
            return Err(EconomyError::InvalidState(
                "group inventory references an unknown good".into(),
            ));
        }
    }
    if state.economy.treasury.money.0 < 0
        || state
            .economy
            .treasury
            .goods
            .values()
            .any(|amount| amount.0 < 0)
    {
        return Err(EconomyError::InvalidState(
            "economy treasury has a negative balance".into(),
        ));
    }
    if state
        .economy
        .treasury
        .goods
        .keys()
        .any(|good| !config.goods.contains_key(good))
    {
        return Err(EconomyError::InvalidState(
            "treasury inventory references an unknown good".into(),
        ));
    }
    if state.economy.working_report.is_none() {
        if let Some(report) = &state.economy.last_year_report {
            if report.year != year {
                return Ok(());
            }
            validate_closed_report(report)?;
        }
    }
    Ok(())
}

/// Convenience runner for isolated tests; production code should call each
/// phase hook at its corresponding A4/A5/A6 boundary.
pub fn run_economy_year(
    config: &EconomicConfig,
    state: &mut State,
    territory: Option<&TerritoryConfig>,
    year: Year,
    modifiers: &EconomyYearModifiers,
) -> EconomyResult<EconomicYearReport> {
    begin_economy_year(config, state, territory, year, &mut Vec::new())?;
    reconcile_economic_groups(config, state, year)?;
    produce_economy(config, state, year, modifiers)?;
    trade_economy(config, state, territory, year)?;
    consume_and_adjust_economy(config, state, territory, year)?;
    finalize_economy_year(config, state, year)
}

/// Probability adjustment from the prior A6 scarcity report. The returned
/// value is signed and must be clamped with the base probability by the caller.
pub fn mortality_scarcity_adjustment(
    config: &EconomicConfig,
    state: &State,
    settlement: SettlementId,
) -> u32 {
    let scarcity = state
        .economy
        .last_year_report
        .as_ref()
        .and_then(|report| report.signals.scarcity_ppm_by_settlement.get(&settlement))
        .copied()
        .unwrap_or_default();
    ((u64::from(config.responses.mortality_increase_at_full_scarcity_ppm) * u64::from(scarcity))
        / 1_000_000) as u32
}

pub fn birth_scarcity_reduction(
    config: &EconomicConfig,
    state: &State,
    settlement: SettlementId,
) -> u32 {
    let scarcity = state
        .economy
        .last_year_report
        .as_ref()
        .and_then(|report| report.signals.scarcity_ppm_by_settlement.get(&settlement))
        .copied()
        .unwrap_or_default();
    ((u64::from(config.responses.birth_reduction_at_full_scarcity_ppm) * u64::from(scarcity))
        / 1_000_000) as u32
}

/// Current A6 scarcity for A7 migration or A9 births within the open year.
pub fn current_birth_scarcity_reduction(
    config: &EconomicConfig,
    state: &State,
    settlement: SettlementId,
) -> u32 {
    let scarcity = state
        .economy
        .working_report
        .as_ref()
        .and_then(|report| report.signals.scarcity_ppm_by_settlement.get(&settlement))
        .copied()
        .unwrap_or_default();
    ((u64::from(config.responses.birth_reduction_at_full_scarcity_ppm) * u64::from(scarcity))
        / 1_000_000) as u32
}

pub fn current_migration_scarcity_adjustment(
    config: &EconomicConfig,
    state: &State,
    settlement: SettlementId,
) -> i64 {
    let scarcity = state
        .economy
        .working_report
        .as_ref()
        .and_then(|report| report.signals.scarcity_ppm_by_settlement.get(&settlement))
        .copied()
        .unwrap_or_default();
    scale_signed(
        config.responses.migration_opportunity_at_full_scarcity,
        scarcity,
    )
}

pub fn migration_scarcity_adjustment(
    config: &EconomicConfig,
    state: &State,
    settlement: SettlementId,
) -> i64 {
    let scarcity = state
        .economy
        .last_year_report
        .as_ref()
        .and_then(|report| report.signals.scarcity_ppm_by_settlement.get(&settlement))
        .copied()
        .unwrap_or_default();
    scale_signed(
        config.responses.migration_opportunity_at_full_scarcity,
        scarcity,
    )
}

fn scale_signed(value: i64, factor_ppm: u32) -> i64 {
    i64::try_from(i128::from(value).saturating_mul(i128::from(factor_ppm)) / i128::from(PPM))
        .unwrap_or(if value < 0 { i64::MIN } else { i64::MAX })
}

/// Applies current year's response policy to a future ordinary mortality rate.
pub fn adjust_mortality_probability(
    config: &EconomicConfig,
    state: &State,
    settlement: SettlementId,
    base_probability_ppm: u32,
) -> u32 {
    base_probability_ppm
        .saturating_add(mortality_scarcity_adjustment(config, state, settlement))
        .min(1_000_000)
}

pub fn adjust_birth_probability(
    config: &EconomicConfig,
    state: &State,
    settlement: SettlementId,
    base_probability_ppm: u32,
) -> u32 {
    base_probability_ppm.saturating_sub(birth_scarcity_reduction(config, state, settlement))
}

#[derive(Clone, Copy, Debug, Default)]
struct GroupCounts {
    members: u64,
    workers: u64,
}

fn market_orders(
    config: &EconomicConfig,
    state: &mut State,
    year: Year,
) -> EconomyResult<(MarketSupply, MarketDemand)> {
    let mut supplies = BTreeMap::new();
    let mut demands = BTreeMap::new();
    let ids: Vec<_> = state
        .economy
        .groups
        .iter()
        .map(|group| group.group_id.clone())
        .collect();
    for group_id in ids {
        let group = group_ref(state, &group_id)
            .ok_or(EconomyError::InvalidState("market group missing".into()))?
            .clone();
        if group.member_count == 0 {
            continue;
        }
        let mut need_orders = Vec::<(GoodId, TradePurpose, u16, i64)>::new();
        for (good_id, need) in &config.needs {
            let desired = multiply_count(need.quantity_per_person_per_year, group.member_count)?.0;
            need_orders.push((
                good_id.clone(),
                TradePurpose::HumanNeed,
                need.priority,
                desired,
            ));
        }
        let input_targets = next_year_input_targets(config, &group)?;
        for (good_id, target) in &input_targets {
            let ledger = group_ledger_mut(state, &group_id)
                .ok_or(EconomyError::PhaseOrder("missing group ledger"))?
                .goods
                .entry(good_id.clone())
                .or_default();
            ledger.input_replenishment_target = *target;
            need_orders.push((
                good_id.clone(),
                TradePurpose::ProductionInput,
                u16::MAX,
                target.0,
            ));
        }
        need_orders.sort_by(|left, right| {
            (left.2, purpose_order(left.1), left.0.clone()).cmp(&(
                right.2,
                purpose_order(right.1),
                right.0.clone(),
            ))
        });
        let mut budget = group.money.0;
        let mut stock_reserved = BTreeMap::<GoodId, i64>::new();
        for (good_id, purpose, _, wanted) in need_orders {
            let stock = group.goods.get(&good_id).copied().unwrap_or_default().0;
            let available_stock = stock
                .saturating_sub(stock_reserved.get(&good_id).copied().unwrap_or_default())
                .max(0);
            let reserved = wanted.min(available_stock).max(0);
            let reserve = stock_reserved.entry(good_id.clone()).or_default();
            *reserve = reserve
                .checked_add(reserved)
                .ok_or(EconomyError::Arithmetic("market stock reserve"))?;
            let desired = wanted.saturating_sub(reserved).max(0);
            let price = current_price(
                &state.economy,
                group_id.settlement_id,
                &good_id,
                &config.goods[&good_id],
            );
            let affordable = if price.0 <= 0 {
                desired
            } else {
                budget / price.0
            };
            let solvent = desired.min(affordable).max(0);
            if price.0 > 0 {
                budget = budget
                    .checked_sub(money_product(price, solvent, "solvent demand budget")?.0)
                    .ok_or(EconomyError::Arithmetic("solvent demand budget"))?;
            }
            demands.insert((group_id.clone(), good_id.clone(), purpose), solvent);
            if let Some(ledger) = group_ledger_mut(state, &group_id) {
                let good = ledger.goods.entry(good_id).or_default();
                if purpose == TradePurpose::HumanNeed {
                    good.solvent_demand = Quantity(solvent);
                    good.physical_need = Quantity(wanted);
                } else {
                    good.solvent_input_demand = Quantity(solvent);
                }
            }
        }
        for good_id in config.goods.keys() {
            let stock = group.goods.get(good_id).copied().unwrap_or_default().0;
            let human_reserve = multiply_count(
                config
                    .needs
                    .get(good_id)
                    .copied()
                    .unwrap_or_default()
                    .quantity_per_person_per_year,
                group.member_count,
            )?
            .0;
            let input_reserve = input_targets.get(good_id).copied().unwrap_or_default().0;
            let reserve = human_reserve
                .checked_add(input_reserve)
                .ok_or(EconomyError::Arithmetic("stock reserve"))?;
            let offer = stock.saturating_sub(reserve).max(0);
            if offer > 0 {
                supplies.insert((group_id.clone(), good_id.clone()), offer);
            }
        }
    }
    let _ = year;
    Ok((supplies, demands))
}

fn next_year_input_targets(
    config: &EconomicConfig,
    group: &EconomicGroupState,
) -> EconomyResult<BTreeMap<GoodId, Quantity>> {
    let mut targets = BTreeMap::new();
    let Some(profession) = &group.group_id.profession_id else {
        return Ok(targets);
    };
    let Some(ProfessionEconomicPolicy {
        recipe: Some(recipe),
    }) = config.profession_policies.get(profession)
    else {
        return Ok(targets);
    };
    let mut batches = group
        .worker_count
        .checked_mul(u64::from(recipe.batches_per_worker))
        .ok_or(EconomyError::Arithmetic("input replenishment target"))?;
    if let Some(maximum) = recipe.maximum_batches_per_year {
        batches = batches.min(maximum);
    }
    for (good_id, per_batch) in &recipe.inputs_per_batch {
        targets.insert(
            good_id.clone(),
            multiply_quantity(*per_batch, batches, "input replenishment target")?,
        );
    }
    Ok(targets)
}

struct TradePlan<'a> {
    seller_id: &'a EconomicGroupId,
    buyer_id: &'a EconomicGroupId,
    good_id: &'a GoodId,
    quantity: i64,
    seller_revenue: Money,
    buyer_payment: Money,
    transport_cost: Money,
    route_id: Option<chargen_domain::LogicalId>,
    purpose: TradePurpose,
    transport_account: TransportCostAccount,
}

fn execute_trade(state: &mut State, plan: TradePlan<'_>) -> EconomyResult<()> {
    let TradePlan {
        seller_id,
        buyer_id,
        good_id,
        quantity,
        seller_revenue,
        buyer_payment,
        transport_cost,
        route_id,
        purpose,
        transport_account,
    } = plan;
    if quantity <= 0 || buyer_payment.0 < 0 || seller_revenue.0 < 0 {
        return Err(EconomyError::InvalidState("invalid market trade".into()));
    }
    let seller_index = state
        .economy
        .groups
        .binary_search_by(|group| group.group_id.cmp(seller_id))
        .map_err(|_| EconomyError::InvalidState("seller missing".into()))?;
    let buyer_index = state
        .economy
        .groups
        .binary_search_by(|group| group.group_id.cmp(buyer_id))
        .map_err(|_| EconomyError::InvalidState("buyer missing".into()))?;
    let seller_stock = state.economy.groups[seller_index]
        .goods
        .get(good_id)
        .copied()
        .unwrap_or_default();
    if seller_stock.0 < quantity || state.economy.groups[buyer_index].money.0 < buyer_payment.0 {
        return Err(EconomyError::InvalidState(
            "market reservation exceeded available assets".into(),
        ));
    }
    state.economy.groups[seller_index]
        .goods
        .insert(good_id.clone(), Quantity(seller_stock.0 - quantity));
    let buyer_stock = state.economy.groups[buyer_index]
        .goods
        .get(good_id)
        .copied()
        .unwrap_or_default();
    state.economy.groups[buyer_index].goods.insert(
        good_id.clone(),
        add_quantity(buyer_stock, Quantity(quantity), "trade stock")?,
    );
    state.economy.groups[seller_index].money = add_money(
        state.economy.groups[seller_index].money,
        seller_revenue,
        "seller revenue",
    )?;
    state.economy.groups[buyer_index].money = sub_money(
        state.economy.groups[buyer_index].money,
        buyer_payment,
        "buyer payment",
    )?;

    if let Some(ledger) = group_ledger_mut(state, seller_id) {
        let good = ledger.goods.entry(good_id.clone()).or_default();
        good.sold = add_quantity(good.sold, Quantity(quantity), "sold ledger")?;
        ledger.sales_income = add_money(ledger.sales_income, seller_revenue, "sales ledger")?;
    }
    if let Some(ledger) = group_ledger_mut(state, buyer_id) {
        let good = ledger.goods.entry(good_id.clone()).or_default();
        good.bought = add_quantity(good.bought, Quantity(quantity), "bought ledger")?;
        ledger.purchases_spent =
            add_money(ledger.purchases_spent, buyer_payment, "purchase ledger")?;
        if purpose == TradePurpose::HumanNeed {
            good.bought_for_human_need = add_quantity(
                good.bought_for_human_need,
                Quantity(quantity),
                "human need purchase ledger",
            )?;
        } else {
            good.bought_for_inputs = add_quantity(
                good.bought_for_inputs,
                Quantity(quantity),
                "input purchase ledger",
            )?;
        }
    }
    if route_id.is_some() {
        if state.economy.working_report.is_none() {
            return Err(EconomyError::PhaseOrder(
                "route trade outside annual ledger",
            ));
        }
        let report = state.economy.working_report.as_mut().unwrap();
        if let Some(source) = settlement_good_ledger_mut_from_report(
            &mut report.settlements,
            seller_id.settlement_id,
            good_id,
        ) {
            source.exports = add_quantity(source.exports, Quantity(quantity), "export ledger")?;
        }
        if let Some(destination) = settlement_good_ledger_mut_from_report(
            &mut report.settlements,
            buyer_id.settlement_id,
            good_id,
        ) {
            destination.imports =
                add_quantity(destination.imports, Quantity(quantity), "import ledger")?;
        }
    } else if let Some(ledger) = settlement_good_ledger_mut(state, seller_id.settlement_id, good_id)
    {
        ledger.local_traded = add_quantity(
            ledger.local_traded,
            Quantity(quantity),
            "local trade ledger",
        )?;
    }
    if transport_cost.0 > 0 {
        match transport_account {
            TransportCostAccount::Treasury => {
                state.economy.treasury.money = add_money(
                    state.economy.treasury.money,
                    transport_cost,
                    "transport treasury credit",
                )?;
            }
            TransportCostAccount::ExternalSink => {
                let report = state
                    .economy
                    .working_report
                    .as_mut()
                    .ok_or(EconomyError::PhaseOrder("trade outside annual ledger"))?;
                report.money.external_transport_outflow = add_money(
                    report.money.external_transport_outflow,
                    transport_cost,
                    "transport sink ledger",
                )?;
            }
        }
    }
    let report = state
        .economy
        .working_report
        .as_mut()
        .ok_or(EconomyError::PhaseOrder("trade outside annual ledger"))?;
    report.trades.push(EconomicTrade {
        source_group: seller_id.clone(),
        destination_group: buyer_id.clone(),
        good_id: good_id.clone(),
        quantity: Quantity(quantity),
        seller_revenue,
        buyer_payment,
        transport_cost,
        route_id,
        purpose,
    });
    Ok(())
}

fn apply_storage_loss(config: &EconomicConfig, state: &mut State) -> EconomyResult<()> {
    let group_ids: Vec<_> = state
        .economy
        .groups
        .iter()
        .map(|group| group.group_id.clone())
        .collect();
    for group_id in group_ids {
        let index = state
            .economy
            .groups
            .binary_search_by(|group| group.group_id.cmp(&group_id))
            .map_err(|_| EconomyError::InvalidState("unsorted economic groups".into()))?;
        let good_ids: Vec<_> = config.goods.keys().cloned().collect();
        for good_id in good_ids {
            let stock = state.economy.groups[index]
                .goods
                .get(&good_id)
                .copied()
                .unwrap_or_default();
            let definition = &config.goods[&good_id];
            let loss = if definition.storable {
                Quantity(mul_div_floor(
                    stock.0,
                    i64::from(definition.annual_storage_loss_ppm),
                    PPM,
                )?)
            } else {
                stock
            };
            if loss.0 > 0 {
                state.economy.groups[index]
                    .goods
                    .insert(good_id.clone(), sub_quantity(stock, loss, "storage loss")?);
                let ledger = group_ledger_mut(state, &group_id)
                    .ok_or(EconomyError::PhaseOrder("missing storage ledger"))?
                    .goods
                    .entry(good_id)
                    .or_default();
                ledger.storage_loss =
                    add_quantity(ledger.storage_loss, loss, "storage loss ledger")?;
            }
        }
    }
    Ok(())
}

fn set_price_reactions(config: &EconomicConfig, state: &mut State) -> EconomyResult<()> {
    let settlements = state
        .economy
        .working_report
        .as_ref()
        .ok_or(EconomyError::PhaseOrder("price adjustment before A6"))?
        .settlements
        .clone();
    let mut pending = BTreeMap::new();
    for settlement in &settlements {
        let mut settlement_pending = BTreeMap::new();
        for market in &settlement.goods {
            let definition = &config.goods[&market.good_id];
            let solvent_shortage =
                market.unmet_solvent_demand.0 > 0 || market.unmet_solvent_input_demand.0 > 0;
            let price = current_price(
                &state.economy,
                settlement.settlement_id,
                &market.good_id,
                definition,
            );
            let new_price = adjust_price(
                price,
                definition,
                config.price_policy,
                solvent_shortage,
                market.unsold_offer.0 > 0 && !solvent_shortage,
            )?;
            settlement_pending.insert(market.good_id.clone(), new_price);
            if let Some(ledger) =
                settlement_good_ledger_mut(state, settlement.settlement_id, &market.good_id)
            {
                ledger.next_price = new_price;
            }
        }
        pending.insert(settlement.settlement_id, settlement_pending);
    }
    state.economy.pending_prices = pending;
    Ok(())
}

fn set_capacity_reactions(config: &EconomicConfig, state: &mut State) -> EconomyResult<()> {
    let policy = config.production_response;
    if !policy.enabled {
        state
            .economy
            .pending_production_capacity_ppm_by_group
            .clear();
        return Ok(());
    }
    let report = state
        .economy
        .working_report
        .as_ref()
        .ok_or(EconomyError::PhaseOrder("capacity adjustment before A6"))?;
    let mut targets = Vec::new();
    for group in &state.economy.groups {
        let Some(profession_id) = &group.group_id.profession_id else {
            continue;
        };
        let Some(Some(recipe)) = config
            .profession_policies
            .get(profession_id)
            .map(|policy| policy.recipe.as_ref())
        else {
            continue;
        };
        let outputs: BTreeSet<_> = recipe.outputs_per_batch.keys().cloned().collect();
        let shortage = report
            .settlements
            .iter()
            .find(|settlement| settlement.settlement_id == group.group_id.settlement_id)
            .into_iter()
            .flat_map(|settlement| &settlement.goods)
            .filter(|good| outputs.contains(&good.good_id))
            .map(|good| {
                good.unmet_solvent_demand
                    .0
                    .checked_add(good.unmet_solvent_input_demand.0)
                    .ok_or(EconomyError::Arithmetic("production response shortage"))
            })
            .try_fold(0_i64, |sum, amount| {
                sum.checked_add(amount?)
                    .ok_or(EconomyError::Arithmetic("production response shortage"))
            })?;
        let surplus = report
            .settlements
            .iter()
            .find(|settlement| settlement.settlement_id == group.group_id.settlement_id)
            .into_iter()
            .flat_map(|settlement| &settlement.goods)
            .filter(|good| outputs.contains(&good.good_id))
            .any(|good| good.unsold_offer.0 > 0);
        let current = capacity_for(&state.economy, &group.group_id);
        let capacity = if shortage > 0 {
            current
                .saturating_add(policy.step_ppm)
                .min(policy.maximum_capacity_ppm)
        } else if surplus {
            current
                .saturating_sub(policy.step_ppm)
                .max(policy.minimum_capacity_ppm)
        } else {
            current
        };
        targets.push(EconomicProductionCapacity {
            group_id: group.group_id.clone(),
            capacity_ppm: capacity,
        });
    }
    state.economy.pending_production_capacity_ppm_by_group = targets;
    Ok(())
}

fn set_profession_reactions(
    config: &EconomicConfig,
    state: &mut State,
    territory: Option<&TerritoryConfig>,
    year: Year,
    signals: &mut EconomySignals,
) -> EconomyResult<()> {
    let settlements = state
        .economy
        .working_report
        .as_ref()
        .ok_or(EconomyError::PhaseOrder("labor adjustment before A6"))?
        .settlements
        .clone();
    let mut scores: BTreeMap<(SettlementId, ProfessionId), i64> = BTreeMap::new();
    let mut output_professions: BTreeMap<GoodId, Vec<ProfessionId>> = BTreeMap::new();
    for (profession_id, policy) in &config.profession_policies {
        if let Some(recipe) = &policy.recipe {
            for good_id in recipe.outputs_per_batch.keys() {
                output_professions
                    .entry(good_id.clone())
                    .or_default()
                    .push(profession_id.clone());
            }
        }
    }
    for settlement in &settlements {
        for good in &settlement.goods {
            let unmet = good
                .unmet_solvent_demand
                .0
                .checked_add(good.unmet_solvent_input_demand.0)
                .ok_or(EconomyError::Arithmetic("profession demand score"))?;
            if unmet <= 0 {
                continue;
            }
            for profession in output_professions.get(&good.good_id).into_iter().flatten() {
                let score = scores
                    .entry((settlement.settlement_id, profession.clone()))
                    .or_default();
                *score = score
                    .checked_add(unmet)
                    .ok_or(EconomyError::Arithmetic("profession demand score"))?;
            }
        }
    }
    for ((settlement_id, profession_id), score) in &scores {
        signals
            .profession_opportunities
            .push(ProfessionOpportunity {
                settlement_id: *settlement_id,
                profession_id: profession_id.clone(),
                unmet_demand_score: Quantity(*score),
            });
    }
    if !config.labor_policy.enabled {
        return Ok(());
    }
    let Some(territory) = territory else {
        return Ok(());
    };
    let mut candidates = Vec::new();
    for person in state.living_people_at(year) {
        if person.age_at_close(year).unwrap_or_default()
            < u32::from(config.labor_policy.minimum_working_age)
        {
            continue;
        }
        let current_score = person
            .profession
            .as_ref()
            .and_then(|profession| scores.get(&(person.settlement_id, profession.clone())))
            .copied()
            .unwrap_or_default();
        let allowed = territory
            .profession_options_by_settlement
            .get(&person.settlement_id)
            .into_iter()
            .flatten();
        let best = allowed
            .filter_map(|profession| {
                if !config.profession_policies.contains_key(profession) {
                    return None;
                }
                scores
                    .get(&(person.settlement_id, profession.clone()))
                    .map(|score| (profession, *score))
            })
            .max_by(|left, right| (left.1, &left.0).cmp(&(right.1, &right.0)));
        if let Some((profession, score)) = best {
            if score > current_score && person.profession.as_ref() != Some(profession) {
                candidates.push((
                    state.seed,
                    state.rules_version,
                    person.logical_id,
                    person.id,
                    person.settlement_id,
                    person.profession.clone(),
                    profession.clone(),
                ));
            }
        }
    }
    candidates.sort_by_key(|candidate| {
        keyed_random_u64(
            candidate.0,
            candidate.1,
            DecisionDomain::EconomicProfessionChange,
            year,
            candidate.2,
            0,
        )
    });
    let mut per_settlement = BTreeMap::<SettlementId, u16>::new();
    let mut changes = Vec::new();
    for (_, _, _, person_id, settlement_id, from, to) in candidates {
        let count = per_settlement.entry(settlement_id).or_default();
        if *count >= config.labor_policy.max_changes_per_settlement_per_year {
            continue;
        }
        *count += 1;
        changes.push(EconomicProfessionChange {
            person_id,
            from,
            to: Some(to),
            effective_year: year
                .checked_add(1)
                .ok_or(EconomyError::Arithmetic("profession year"))?,
        });
    }
    state.economy.pending_profession_changes = changes;
    Ok(())
}

fn apply_pending_profession_changes(
    config: &EconomicConfig,
    state: &mut State,
    territory: Option<&TerritoryConfig>,
    year: Year,
    events: &mut Vec<HistoricalEvent>,
) -> EconomyResult<()> {
    let mut pending = Vec::new();
    for change in std::mem::take(&mut state.economy.pending_profession_changes) {
        if change.effective_year > year {
            pending.push(change);
            continue;
        }
        let Some(person) = state.persons.get(&change.person_id) else {
            continue;
        };
        if !person.alive_at_close(year) || person.profession != change.from {
            continue;
        }
        let person_logical = person.logical_id;
        let settlement_id = person.settlement_id;
        if let Some(profession) = change.to.as_ref() {
            let is_allowed = config.profession_policies.contains_key(profession)
                && territory.is_some_and(|territory| {
                    territory
                        .profession_options_by_settlement
                        .get(&settlement_id)
                        .is_some_and(|options| options.contains(profession))
                });
            if !is_allowed {
                continue;
            }
        }
        if let Some(person) = state.persons.get_mut(&change.person_id) {
            person.profession = change.to.clone();
        }
        let intervals = state
            .profession_intervals
            .entry(change.person_id)
            .or_default();
        if let Some(active) = intervals
            .iter_mut()
            .rev()
            .find(|interval| interval.validity.end_exclusive.is_none())
        {
            active.validity.end_exclusive = Some(year);
        }
        if let Some(profession_id) = &change.to {
            intervals.push(chargen_domain::ProfessionInterval {
                profession_id: profession_id.clone(),
                validity: YearInterval {
                    start: year,
                    end_exclusive: None,
                },
            });
        }
        let event_id = chargen_domain::LogicalId(keyed_random_u64(
            state.seed,
            state.rules_version,
            DecisionDomain::EconomicProfessionChange,
            year,
            person_logical,
            1,
        ));
        events.push(HistoricalEvent {
            id: event_id,
            year,
            order: u32::try_from(events.len())
                .map_err(|_| EconomyError::Arithmetic("historical event order"))?,
            affected_entities: vec![
                EntityRef::World(state.world_id),
                EntityRef::Person(change.person_id),
                EntityRef::Settlement(settlement_id),
            ],
            kind: HistoricalEventKind::ProfessionChanged {
                person_id: change.person_id,
                from: change.from,
                to: change.to,
                reason: ProfessionChangeReason::EconomicOpportunity,
            },
        });
    }
    state.economy.pending_profession_changes = pending;
    Ok(())
}

fn transfer_departing_assets(
    state: &mut State,
    moves: &BTreeMap<EconomicGroupId, BTreeMap<EconomicGroupId, u64>>,
    old_member_counts: &BTreeMap<EconomicGroupId, u64>,
) -> EconomyResult<()> {
    for (source_id, destinations) in moves {
        let Some(source_index) = state
            .economy
            .groups
            .binary_search_by(|group| group.group_id.cmp(source_id))
            .ok()
        else {
            continue;
        };
        let denominator = old_member_counts
            .get(source_id)
            .copied()
            .unwrap_or_default();
        if denominator == 0 {
            continue;
        }
        let source_money = state.economy.groups[source_index].money;
        let source_goods = state.economy.groups[source_index].goods.clone();
        let mut total_money = 0_i64;
        let mut total_goods = BTreeMap::<GoodId, i64>::new();
        for (destination_id, count) in destinations {
            let share_money = proportional_share(source_money.0, *count, denominator)?;
            total_money = total_money
                .checked_add(share_money)
                .ok_or(EconomyError::Arithmetic("asset transfer money"))?;
            for (good_id, stock) in &source_goods {
                let share = proportional_share(stock.0, *count, denominator)?;
                let total = total_goods.entry(good_id.clone()).or_default();
                *total = total
                    .checked_add(share)
                    .ok_or(EconomyError::Arithmetic("asset transfer stock"))?;
                let destination_index = state
                    .economy
                    .groups
                    .binary_search_by(|group| group.group_id.cmp(destination_id))
                    .map_err(|_| EconomyError::InvalidState("transfer target missing".into()))?;
                let target = state.economy.groups[destination_index]
                    .goods
                    .get(good_id)
                    .copied()
                    .unwrap_or_default();
                state.economy.groups[destination_index].goods.insert(
                    good_id.clone(),
                    add_quantity(target, Quantity(share), "asset transfer")?,
                );
            }
            let destination_index = state
                .economy
                .groups
                .binary_search_by(|group| group.group_id.cmp(destination_id))
                .map_err(|_| EconomyError::InvalidState("transfer target missing".into()))?;
            state.economy.groups[destination_index].money = add_money(
                state.economy.groups[destination_index].money,
                Money(share_money),
                "asset transfer",
            )?;
            if let Some(ledger) = group_ledger_mut(state, destination_id) {
                ledger.transfers_in =
                    add_money(ledger.transfers_in, Money(share_money), "transfer ledger")?;
                for (good_id, stock) in &source_goods {
                    let share = proportional_share(stock.0, *count, denominator)?;
                    let good = ledger.goods.entry(good_id.clone()).or_default();
                    good.transfers_in =
                        add_quantity(good.transfers_in, Quantity(share), "transfer ledger")?;
                }
            }
        }
        let source_index = state
            .economy
            .groups
            .binary_search_by(|group| group.group_id.cmp(source_id))
            .map_err(|_| EconomyError::InvalidState("transfer source missing".into()))?;
        state.economy.groups[source_index].money = sub_money(
            state.economy.groups[source_index].money,
            Money(total_money),
            "asset transfer",
        )?;
        for (good_id, amount) in total_goods {
            let old = state.economy.groups[source_index]
                .goods
                .get(&good_id)
                .copied()
                .unwrap_or_default();
            state.economy.groups[source_index].goods.insert(
                good_id.clone(),
                sub_quantity(old, Quantity(amount), "asset transfer")?,
            );
            if let Some(ledger) = group_ledger_mut(state, source_id) {
                let good = ledger.goods.entry(good_id).or_default();
                good.transfers_out =
                    add_quantity(good.transfers_out, Quantity(amount), "transfer ledger")?;
            }
        }
        if let Some(ledger) = group_ledger_mut(state, source_id) {
            ledger.transfers_out =
                add_money(ledger.transfers_out, Money(total_money), "transfer ledger")?;
        }
    }
    Ok(())
}

fn apply_empty_group_disposition(config: &EconomicConfig, state: &mut State) -> EconomyResult<()> {
    let empty_ids: Vec<_> = state
        .economy
        .groups
        .iter()
        .filter(|group| group.member_count == 0)
        .map(|group| group.group_id.clone())
        .collect();
    for source_id in empty_ids {
        let source_index = state
            .economy
            .groups
            .binary_search_by(|group| group.group_id.cmp(&source_id))
            .map_err(|_| EconomyError::InvalidState("empty group missing".into()))?;
        let has_assets = state.economy.groups[source_index].money.0 > 0
            || state.economy.groups[source_index]
                .goods
                .values()
                .any(|quantity| quantity.0 > 0);
        if !has_assets || config.empty_group_disposition == EmptyGroupDisposition::RetainDormant {
            continue;
        }
        match config.empty_group_disposition {
            EmptyGroupDisposition::RetainDormant => {}
            EmptyGroupDisposition::TransferToConsumerGroup => {
                if source_id.profession_id.is_none() {
                    continue;
                }
                let consumer_id = EconomicGroupId::consumer_group(source_id.settlement_id);
                ensure_group(&mut state.economy, &consumer_id);
                transfer_all_assets(state, &source_id, &consumer_id)?;
            }
            EmptyGroupDisposition::TransferToTreasury => {
                let source_index = state
                    .economy
                    .groups
                    .binary_search_by(|group| group.group_id.cmp(&source_id))
                    .map_err(|_| EconomyError::InvalidState("empty source missing".into()))?;
                let money = state.economy.groups[source_index].money;
                state.economy.treasury.money = add_money(
                    state.economy.treasury.money,
                    money,
                    "empty group treasury transfer",
                )?;
                state.economy.groups[source_index].money = Money(0);
                let goods = std::mem::take(&mut state.economy.groups[source_index].goods);
                for (good_id, amount) in goods {
                    let old = state
                        .economy
                        .treasury
                        .goods
                        .get(&good_id)
                        .copied()
                        .unwrap_or_default();
                    state.economy.treasury.goods.insert(
                        good_id.clone(),
                        add_quantity(old, amount, "empty group treasury transfer")?,
                    );
                    if let Some(report) = state.economy.working_report.as_mut() {
                        let treasury_good =
                            report.treasury_goods.entry(good_id.clone()).or_default();
                        treasury_good.transfers_in = add_quantity(
                            treasury_good.transfers_in,
                            amount,
                            "treasury stock transfer ledger",
                        )?;
                    }
                    if let Some(ledger) = group_ledger_mut(state, &source_id) {
                        let good = ledger.goods.entry(good_id).or_default();
                        good.transfers_out = add_quantity(
                            good.transfers_out,
                            amount,
                            "group stock transfer ledger",
                        )?;
                    }
                }
                if let Some(ledger) = group_ledger_mut(state, &source_id) {
                    ledger.transfers_out =
                        add_money(ledger.transfers_out, money, "treasury transfer ledger")?;
                }
            }
        }
    }
    Ok(())
}

fn transfer_all_assets(
    state: &mut State,
    source_id: &EconomicGroupId,
    destination_id: &EconomicGroupId,
) -> EconomyResult<()> {
    let source_index = state
        .economy
        .groups
        .binary_search_by(|group| group.group_id.cmp(source_id))
        .map_err(|_| EconomyError::InvalidState("asset source missing".into()))?;
    let destination_index = state
        .economy
        .groups
        .binary_search_by(|group| group.group_id.cmp(destination_id))
        .map_err(|_| EconomyError::InvalidState("asset destination missing".into()))?;
    if source_index == destination_index {
        return Ok(());
    }
    let money = state.economy.groups[source_index].money;
    let goods = state.economy.groups[source_index].goods.clone();
    state.economy.groups[source_index].money = Money(0);
    state.economy.groups[destination_index].money = add_money(
        state.economy.groups[destination_index].money,
        money,
        "empty group transfer",
    )?;
    for (good_id, amount) in &goods {
        let old = state.economy.groups[destination_index]
            .goods
            .get(good_id)
            .copied()
            .unwrap_or_default();
        state.economy.groups[destination_index].goods.insert(
            good_id.clone(),
            add_quantity(old, *amount, "empty group transfer")?,
        );
    }
    state.economy.groups[source_index].goods.clear();
    if let Some(ledger) = group_ledger_mut(state, source_id) {
        ledger.transfers_out = add_money(ledger.transfers_out, money, "empty transfer ledger")?;
        for (good_id, amount) in &goods {
            let good = ledger.goods.entry(good_id.clone()).or_default();
            good.transfers_out =
                add_quantity(good.transfers_out, *amount, "empty transfer ledger")?;
        }
    }
    if let Some(ledger) = group_ledger_mut(state, destination_id) {
        ledger.transfers_in = add_money(ledger.transfers_in, money, "empty transfer ledger")?;
        for (good_id, amount) in goods {
            let good = ledger.goods.entry(good_id).or_default();
            good.transfers_in = add_quantity(good.transfers_in, amount, "empty transfer ledger")?;
        }
    }
    Ok(())
}

fn record_unmet_input_targets(
    config: &EconomicConfig,
    state: &mut State,
    _year: Year,
) -> EconomyResult<()> {
    let group_ids: Vec<_> = state
        .economy
        .groups
        .iter()
        .map(|group| group.group_id.clone())
        .collect();
    for group_id in group_ids {
        let group = group_ref(state, &group_id)
            .ok_or(EconomyError::InvalidState("input group missing".into()))?
            .clone();
        let targets = next_year_input_targets(config, &group)?;
        for (good_id, target) in targets {
            let stock = group.goods.get(&good_id).copied().unwrap_or_default();
            let human_reserve = multiply_count(
                config
                    .needs
                    .get(&good_id)
                    .copied()
                    .unwrap_or_default()
                    .quantity_per_person_per_year,
                group.member_count,
            )?
            .0;
            let input_available = stock.0.saturating_sub(human_reserve).max(0);
            let unmet = target.0.saturating_sub(input_available).max(0);
            let ledger = group_ledger_mut(state, &group_id)
                .ok_or(EconomyError::PhaseOrder("missing input group ledger"))?
                .goods
                .entry(good_id)
                .or_default();
            ledger.input_replenishment_target = target;
            ledger.unmet_input_demand = Quantity(unmet);
            ledger.unmet_solvent_input_demand = Quantity(
                ledger
                    .solvent_input_demand
                    .0
                    .saturating_sub(ledger.bought_for_inputs.0),
            );
        }
    }
    let summary: Vec<_> = state
        .economy
        .working_report
        .as_ref()
        .ok_or(EconomyError::PhaseOrder("missing input report"))?
        .groups
        .iter()
        .flat_map(|group| {
            group.goods.iter().map(move |(good_id, ledger)| {
                (
                    group.group_id.settlement_id,
                    good_id.clone(),
                    ledger.input_replenishment_target,
                    ledger.unmet_input_demand,
                    ledger.solvent_input_demand,
                    ledger.unmet_solvent_input_demand,
                )
            })
        })
        .collect();
    for (settlement, good, target, unmet, solvent, unmet_solvent) in summary {
        if let Some(ledger) = settlement_good_ledger_mut(state, settlement, &good) {
            ledger.input_replenishment_target = add_quantity(
                ledger.input_replenishment_target,
                target,
                "settlement input target",
            )?;
            ledger.unmet_input_demand =
                add_quantity(ledger.unmet_input_demand, unmet, "settlement unmet input")?;
            ledger.solvent_input_demand = add_quantity(
                ledger.solvent_input_demand,
                solvent,
                "settlement solvent input",
            )?;
            ledger.unmet_solvent_input_demand = add_quantity(
                ledger.unmet_solvent_input_demand,
                unmet_solvent,
                "settlement unmet solvent input",
            )?;
        }
    }
    Ok(())
}

fn record_unsold_offers(
    state: &mut State,
    supplies: &BTreeMap<(EconomicGroupId, GoodId), i64>,
) -> EconomyResult<()> {
    let remaining: Vec<_> = supplies
        .iter()
        .filter(|(_, quantity)| **quantity > 0)
        .map(|((group_id, good_id), quantity)| {
            (group_id.settlement_id, good_id.clone(), Quantity(*quantity))
        })
        .collect();
    for (settlement_id, good_id, quantity) in remaining {
        if let Some(ledger) = settlement_good_ledger_mut(state, settlement_id, &good_id) {
            ledger.unsold_offer =
                add_quantity(ledger.unsold_offer, quantity, "unsold offer ledger")?;
        }
    }
    Ok(())
}

fn refresh_working_group_counts(state: &mut State) -> EconomyResult<()> {
    let Some(report) = state.economy.working_report.as_mut() else {
        // Initial materialization also reconciles groups before the first A0.
        // Its raw member/worker counts are authoritative; there is no annual
        // ledger to update until a year transaction has started.
        return Ok(());
    };
    for group in &state.economy.groups {
        let ledger = report_group_mut(report, &group.group_id);
        ledger.member_count_after = group.member_count;
        ledger.worker_count_after = group.worker_count;
    }
    Ok(())
}

fn update_closing_balances(state: &mut State) -> EconomyResult<()> {
    let report = state
        .economy
        .working_report
        .as_mut()
        .ok_or(EconomyError::PhaseOrder("closing balance outside year"))?;
    report.money.closing_group_money = sum_group_money(&state.economy.groups)?;
    report.money.closing_treasury_money = state.economy.treasury.money;
    for group in &state.economy.groups {
        report_group_mut(report, &group.group_id).closing_money = group.money;
    }
    Ok(())
}

fn update_settlement_ending_stock(
    state: &mut State,
    settlement_id: SettlementId,
    good_id: &GoodId,
) -> EconomyResult<()> {
    let mut total = 0_i64;
    for group in state
        .economy
        .groups
        .iter()
        .filter(|group| group.group_id.settlement_id == settlement_id)
    {
        total = total
            .checked_add(group.goods.get(good_id).copied().unwrap_or_default().0)
            .ok_or(EconomyError::Arithmetic("settlement ending stock"))?;
    }
    if let Some(ledger) = settlement_good_ledger_mut(state, settlement_id, good_id) {
        ledger.ending_stock = Quantity(total);
    }
    Ok(())
}

fn validate_closed_report(report: &EconomicYearReport) -> EconomyResult<()> {
    let expected_money = i128::from(report.money.opening_group_money.0)
        + i128::from(report.money.opening_treasury_money.0)
        + i128::from(report.money.explicit_emission.0)
        - i128::from(report.money.explicit_retirement.0)
        - i128::from(report.money.external_transport_outflow.0);
    let actual_money = i128::from(report.money.closing_group_money.0)
        + i128::from(report.money.closing_treasury_money.0);
    if expected_money != actual_money {
        return Err(EconomyError::InvalidState(
            "world money ledger does not balance".into(),
        ));
    }
    for group in &report.groups {
        let expected_group_money = i128::from(group.opening_money.0)
            + i128::from(group.sales_income.0)
            + i128::from(group.transfers_in.0)
            - i128::from(group.purchases_spent.0)
            - i128::from(group.transfers_out.0);
        if expected_group_money != i128::from(group.closing_money.0) {
            return Err(EconomyError::InvalidState(
                "group money ledger does not balance".into(),
            ));
        }
        for (good_id, ledger) in &group.goods {
            let expected = i128::from(ledger.opening_stock.0)
                + i128::from(ledger.produced.0)
                + i128::from(ledger.bought.0)
                + i128::from(ledger.transfers_in.0)
                - i128::from(ledger.input_consumed.0)
                - i128::from(ledger.sold.0)
                - i128::from(ledger.consumer_consumed.0)
                - i128::from(ledger.storage_loss.0)
                - i128::from(ledger.event_destroyed.0)
                - i128::from(ledger.transfers_out.0);
            if expected != i128::from(ledger.closing_stock.0) {
                return Err(EconomyError::InvalidState(format!(
                    "group goods ledger does not balance for {}",
                    good_id.0
                )));
            }
        }
    }
    for (good_id, ledger) in &report.treasury_goods {
        let expected = i128::from(ledger.opening_stock.0) + i128::from(ledger.transfers_in.0)
            - i128::from(ledger.transfers_out.0)
            - i128::from(ledger.event_destroyed.0)
            - i128::from(ledger.storage_loss.0);
        if expected != i128::from(ledger.closing_stock.0) {
            return Err(EconomyError::InvalidState(format!(
                "treasury goods ledger does not balance for {}",
                good_id.0
            )));
        }
    }
    Ok(())
}

fn new_group_ledger(group: &EconomicGroupState, config: &EconomicConfig) -> EconomicGroupLedger {
    let mut ledger = EconomicGroupLedger {
        group_id: group.group_id.clone(),
        member_count_before: group.member_count,
        worker_count_before: group.worker_count,
        member_count_after: group.member_count,
        worker_count_after: group.worker_count,
        opening_money: group.money,
        closing_money: group.money,
        ..EconomicGroupLedger::default()
    };
    for good_id in config.goods.keys() {
        ledger.goods.insert(
            good_id.clone(),
            EconomicGoodLedger {
                opening_stock: group.goods.get(good_id).copied().unwrap_or_default(),
                closing_stock: group.goods.get(good_id).copied().unwrap_or_default(),
                ..EconomicGoodLedger::default()
            },
        );
    }
    ledger
}

fn ensure_group<'a>(
    economy: &'a mut EconomyState,
    group_id: &EconomicGroupId,
) -> &'a mut EconomicGroupState {
    match economy
        .groups
        .binary_search_by(|group| group.group_id.cmp(group_id))
    {
        Ok(index) => &mut economy.groups[index],
        Err(index) => {
            economy.groups.insert(
                index,
                EconomicGroupState {
                    group_id: group_id.clone(),
                    ..EconomicGroupState::default()
                },
            );
            &mut economy.groups[index]
        }
    }
}

fn index_groups(groups: &[EconomicGroupState]) -> BTreeMap<EconomicGroupId, usize> {
    groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.group_id.clone(), index))
        .collect()
}

fn group_ref<'a>(state: &'a State, group_id: &EconomicGroupId) -> Option<&'a EconomicGroupState> {
    state.economy.group(group_id)
}

fn group_ledger_ref<'a>(
    state: &'a State,
    group_id: &EconomicGroupId,
) -> Option<&'a EconomicGroupLedger> {
    state
        .economy
        .working_report
        .as_ref()?
        .groups
        .binary_search_by(|entry| entry.group_id.cmp(group_id))
        .ok()
        .map(|index| &state.economy.working_report.as_ref().unwrap().groups[index])
}

fn group_ledger_mut<'a>(
    state: &'a mut State,
    group_id: &EconomicGroupId,
) -> Option<&'a mut EconomicGroupLedger> {
    state.economy.group(group_id)?;
    let report = state.economy.working_report.as_mut()?;
    match report
        .groups
        .binary_search_by(|entry| entry.group_id.cmp(group_id))
    {
        Ok(index) => Some(&mut report.groups[index]),
        Err(index) => {
            report.groups.insert(
                index,
                EconomicGroupLedger {
                    group_id: group_id.clone(),
                    ..EconomicGroupLedger::default()
                },
            );
            Some(&mut report.groups[index])
        }
    }
}

fn report_group_mut<'a>(
    report: &'a mut EconomicYearReport,
    group_id: &EconomicGroupId,
) -> &'a mut EconomicGroupLedger {
    match report
        .groups
        .binary_search_by(|entry| entry.group_id.cmp(group_id))
    {
        Ok(index) => &mut report.groups[index],
        Err(index) => {
            report.groups.insert(
                index,
                EconomicGroupLedger {
                    group_id: group_id.clone(),
                    ..EconomicGroupLedger::default()
                },
            );
            &mut report.groups[index]
        }
    }
}

fn settlement_good_ledger_mut<'a>(
    state: &'a mut State,
    settlement_id: SettlementId,
    good_id: &GoodId,
) -> Option<&'a mut SettlementGoodMarketLedger> {
    let report = state.economy.working_report.as_mut()?;
    settlement_good_ledger_mut_from_report(&mut report.settlements, settlement_id, good_id)
}

fn settlement_good_ledger_mut_from_report<'a>(
    settlements: &'a mut [SettlementEconomicLedger],
    settlement_id: SettlementId,
    good_id: &GoodId,
) -> Option<&'a mut SettlementGoodMarketLedger> {
    settlements
        .iter_mut()
        .find(|entry| entry.settlement_id == settlement_id)?
        .goods
        .iter_mut()
        .find(|entry| entry.good_id == *good_id)
}

fn ensure_working_report(state: &State) -> EconomyResult<()> {
    if state.economy.working_report.is_some() {
        Ok(())
    } else {
        Err(EconomyError::PhaseOrder("phase requires A0 working report"))
    }
}

fn current_price(
    economy: &EconomyState,
    settlement_id: SettlementId,
    good_id: &GoodId,
    definition: &GoodDefinition,
) -> Money {
    economy
        .prices_by_settlement
        .get(&settlement_id)
        .and_then(|prices| prices.get(good_id))
        .copied()
        .unwrap_or(definition.initial_price)
}

fn clamp_price(price: Money, definition: &GoodDefinition) -> Money {
    Money(
        price
            .0
            .max(definition.minimum_price.0)
            .min(definition.maximum_price.0),
    )
}

fn all_settlement_goods(state: &State, config: &EconomicConfig) -> Vec<(SettlementId, GoodId)> {
    state
        .settlements
        .keys()
        .flat_map(|settlement_id| {
            config
                .goods
                .keys()
                .map(move |good_id| (*settlement_id, good_id.clone()))
        })
        .collect()
}

fn buyer_rank(
    config: &EconomicConfig,
    state: &State,
    group: &EconomicGroupId,
    year: Year,
) -> EconomyResult<u64> {
    if config.buyer_priority == chargen_domain::BuyerPriorityPolicy::GroupKeyOrder {
        return Ok(((group.settlement_id.0 as u128) << 64
            | profession_rank(&group.profession_id) as u128) as u64);
    }
    let settlement_logical = state
        .settlements
        .get(&group.settlement_id)
        .ok_or(EconomyError::InvalidState(
            "buyer settlement is missing".into(),
        ))?
        .logical_id;
    Ok(keyed_random_u64(
        state.seed,
        state.rules_version,
        DecisionDomain::EconomicMarketPriority,
        year,
        settlement_logical,
        profession_rank(&group.profession_id),
    ))
}

fn profession_rank(profession: &Option<ProfessionId>) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in profession
        .as_ref()
        .map(|id| id.0.as_bytes())
        .unwrap_or(b"<consumer>")
    {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn purpose_order(purpose: TradePurpose) -> u8 {
    match purpose {
        TradePurpose::HumanNeed => 0,
        TradePurpose::ProductionInput => 1,
    }
}

fn capacity_for(economy: &EconomyState, group_id: &EconomicGroupId) -> u32 {
    economy
        .production_capacity_ppm_by_group
        .binary_search_by(|entry| entry.group_id.cmp(group_id))
        .ok()
        .map(|index| economy.production_capacity_ppm_by_group[index].capacity_ppm)
        .unwrap_or(1_000_000)
}

fn set_capacity(economy: &mut EconomyState, capacity: EconomicProductionCapacity) {
    match economy
        .production_capacity_ppm_by_group
        .binary_search_by(|entry| entry.group_id.cmp(&capacity.group_id))
    {
        Ok(index) => economy.production_capacity_ppm_by_group[index] = capacity,
        Err(index) => economy
            .production_capacity_ppm_by_group
            .insert(index, capacity),
    }
}

fn is_worker(policy: EconomicLaborPolicy, person: &Person, year: Year) -> bool {
    person.profession.is_some()
        && person
            .age_at_close(year)
            .is_some_and(|age| age >= u32::from(policy.minimum_working_age))
}

fn adjust_price(
    price: Money,
    definition: &GoodDefinition,
    policy: EconomicPricePolicy,
    shortage: bool,
    surplus: bool,
) -> EconomyResult<Money> {
    if policy.adjustment_step_ppm == 0 || shortage == surplus {
        return Ok(price);
    }
    let delta = mul_div_floor(price.0, i64::from(policy.adjustment_step_ppm), PPM)?.max(1);
    let changed = if shortage {
        price.0.saturating_add(delta)
    } else {
        price.0.saturating_sub(delta)
    };
    Ok(Money(changed.clamp(
        definition.minimum_price.0,
        definition.maximum_price.0,
    )))
}

fn multiply_count(amount: Quantity, count: u64) -> EconomyResult<Quantity> {
    let value = i128::from(amount.0)
        .checked_mul(i128::from(count))
        .ok_or(EconomyError::Arithmetic("per-person quantity"))?;
    Ok(Quantity(i64::try_from(value).map_err(|_| {
        EconomyError::Arithmetic("per-person quantity")
    })?))
}

fn multiply_quantity(
    amount: Quantity,
    count: u64,
    context: &'static str,
) -> EconomyResult<Quantity> {
    let value = i128::from(amount.0)
        .checked_mul(i128::from(count))
        .ok_or(EconomyError::Arithmetic(context))?;
    Ok(Quantity(
        i64::try_from(value).map_err(|_| EconomyError::Arithmetic(context))?,
    ))
}

fn money_product(price: Money, quantity: i64, context: &'static str) -> EconomyResult<Money> {
    let value = i128::from(price.0)
        .checked_mul(i128::from(quantity))
        .ok_or(EconomyError::Arithmetic(context))?;
    Ok(Money(
        i64::try_from(value).map_err(|_| EconomyError::Arithmetic(context))?,
    ))
}

fn mul_div_floor(value: i64, numerator: i64, denominator: i64) -> EconomyResult<i64> {
    if value < 0 || numerator < 0 || denominator <= 0 {
        return Err(EconomyError::InvalidState(
            "economic fixed point operands must be non-negative".into(),
        ));
    }
    let result = i128::from(value)
        .checked_mul(i128::from(numerator))
        .ok_or(EconomyError::Arithmetic("fixed-point multiplication"))?
        / i128::from(denominator);
    i64::try_from(result).map_err(|_| EconomyError::Arithmetic("fixed-point conversion"))
}

fn proportional_share(value: i64, count: u64, denominator: u64) -> EconomyResult<i64> {
    if denominator == 0 {
        return Ok(0);
    }
    let result = i128::from(value)
        .checked_mul(i128::from(count))
        .ok_or(EconomyError::Arithmetic("proportional asset transfer"))?
        / i128::from(denominator);
    i64::try_from(result).map_err(|_| EconomyError::Arithmetic("proportional asset transfer"))
}

fn add_quantity(left: Quantity, right: Quantity, context: &'static str) -> EconomyResult<Quantity> {
    left.checked_add(right)
        .filter(|value| value.0 >= 0)
        .ok_or(EconomyError::Arithmetic(context))
}

fn sub_quantity(left: Quantity, right: Quantity, context: &'static str) -> EconomyResult<Quantity> {
    left.checked_sub(right)
        .filter(|value| value.0 >= 0)
        .ok_or(EconomyError::Arithmetic(context))
}

fn add_money(left: Money, right: Money, context: &'static str) -> EconomyResult<Money> {
    left.checked_add(right)
        .filter(|value| value.0 >= 0)
        .ok_or(EconomyError::Arithmetic(context))
}

fn sub_money(left: Money, right: Money, context: &'static str) -> EconomyResult<Money> {
    left.checked_sub(right)
        .filter(|value| value.0 >= 0)
        .ok_or(EconomyError::Arithmetic(context))
}

fn sum_group_money(groups: &[EconomicGroupState]) -> EconomyResult<Money> {
    let mut total = 0_i64;
    for group in groups {
        total = total
            .checked_add(group.money.0)
            .ok_or(EconomyError::Arithmetic("group money total"))?;
    }
    Ok(Money(total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chargen_domain::{
        DirectedRoute, EconomicGroupEndowment, EconomicGroupEndowmentEntry,
        EconomicProductionResponsePolicy, Genome, PersonId, ProductionRecipe, ProfessionDefinition,
        ReproductiveProfile, Settlement, WorldId,
    };

    const YEAR: Year = Year(20);

    fn state_with_people(settlements: &[u64], people: &[(u64, u64, Option<&str>)]) -> State {
        let mut state = State {
            world_id: WorldId(800),
            rules_version: 3,
            seed: 73,
            start_year: YEAR,
            current_year: None,
            next_person_id: 1,
            next_household_id: 1,
            next_union_id: 1,
            next_family_link_id: 1,
            settlements: BTreeMap::new(),
            persons: BTreeMap::new(),
            households: BTreeMap::new(),
            unions: BTreeMap::new(),
            family_links: BTreeMap::new(),
            last_gestational_birth_year: BTreeMap::new(),
            residences: BTreeMap::new(),
            profession_intervals: BTreeMap::new(),
            last_year_settlement_balances: BTreeMap::new(),
            culture: chargen_domain::CultureState::default(),
            economy: EconomyState::default(),
            event_effects: Vec::new(),
            last_year_phases: Vec::new(),
        };
        for settlement in settlements {
            let id = SettlementId(*settlement);
            state.settlements.insert(
                id,
                Settlement {
                    id,
                    logical_id: chargen_domain::LogicalId(10_000 + *settlement),
                    name: format!("Settlement {settlement}"),
                    culture_values: BTreeMap::new(),
                },
            );
        }
        for (person, settlement, profession) in people {
            let id = PersonId(*person);
            state.persons.insert(
                id,
                Person {
                    id,
                    logical_id: chargen_domain::LogicalId(20_000 + *person),
                    birth_year: Year(0),
                    death_year: None,
                    settlement_id: SettlementId(*settlement),
                    household_id: None,
                    reproductive: ReproductiveProfile::default(),
                    traits: BTreeMap::new(),
                    genome: Genome::default(),
                    last_widowed_year: None,
                    profession: profession.map(ProfessionId::from),
                },
            );
        }
        state
    }

    fn definition(price: i64, storable: bool, storage_loss_ppm: u32) -> GoodDefinition {
        GoodDefinition {
            name: "test good".into(),
            unit: "unit".into(),
            transport_units_per_unit: 1,
            storable,
            annual_storage_loss_ppm: storage_loss_ppm,
            initial_price: Money(price),
            minimum_price: Money(1),
            maximum_price: Money(100),
        }
    }

    fn group(settlement_id: u64, profession_id: Option<&str>) -> EconomicGroupId {
        EconomicGroupId {
            settlement_id: SettlementId(settlement_id),
            profession_id: profession_id.map(ProfessionId::from),
        }
    }

    fn endowment(
        group_id: EconomicGroupId,
        money: i64,
        goods: &[(&str, i64)],
    ) -> EconomicGroupEndowmentEntry {
        EconomicGroupEndowmentEntry {
            group_id,
            endowment: EconomicGroupEndowment {
                money: Money(money),
                goods: goods
                    .iter()
                    .map(|(good, quantity)| (GoodId::from(*good), Quantity(*quantity)))
                    .collect(),
            },
        }
    }

    fn initialize(config: &EconomicConfig, state: &mut State) {
        initialize_economy(config, state, YEAR).expect("initial economy");
    }

    fn close_year(
        config: &EconomicConfig,
        state: &mut State,
        territory: Option<&TerritoryConfig>,
        year: Year,
        modifiers: &EconomyYearModifiers,
    ) -> EconomicYearReport {
        run_economy_year(config, state, territory, year, modifiers).expect("closed economy year")
    }

    fn group_ledger<'a>(
        report: &'a EconomicYearReport,
        group_id: &EconomicGroupId,
    ) -> &'a EconomicGroupLedger {
        report
            .groups
            .iter()
            .find(|ledger| &ledger.group_id == group_id)
            .expect("group ledger")
    }

    fn group_good<'a>(
        report: &'a EconomicYearReport,
        group_id: &EconomicGroupId,
        good_id: &GoodId,
    ) -> &'a EconomicGoodLedger {
        group_ledger(report, group_id)
            .goods
            .get(good_id)
            .expect("group good ledger")
    }

    fn settlement_good<'a>(
        report: &'a EconomicYearReport,
        settlement_id: SettlementId,
        good_id: &GoodId,
    ) -> &'a SettlementGoodMarketLedger {
        report
            .settlements
            .iter()
            .find(|settlement| settlement.settlement_id == settlement_id)
            .expect("settlement ledger")
            .goods
            .iter()
            .find(|good| &good.good_id == good_id)
            .expect("settlement good ledger")
    }

    #[test]
    fn local_and_route_markets_reserve_capacity_and_separate_physical_from_solvent_need() {
        let mut state = state_with_people(
            &[1, 2, 3, 4],
            &[
                (1, 1, Some("farmer")),
                (2, 1, None),
                (3, 2, Some("farmer")),
                (4, 3, None),
                (5, 3, None),
                (6, 4, None),
            ],
        );
        let food = GoodId::from("food");
        let mut config = EconomicConfig {
            goods: BTreeMap::from([(food.clone(), definition(2, true, 0))]),
            needs: BTreeMap::from([(
                food.clone(),
                chargen_domain::GoodNeed {
                    quantity_per_person_per_year: Quantity(1),
                    priority: 0,
                },
            )]),
            initial_group_endowments: vec![
                endowment(group(1, Some("farmer")), 0, &[("food", 3)]),
                endowment(group(1, None), 2, &[]),
                endowment(group(2, Some("farmer")), 0, &[("food", 3)]),
                endowment(group(3, None), 3, &[]),
                endowment(group(4, None), 0, &[]),
            ],
            ..EconomicConfig::default()
        };
        let route_id = chargen_domain::LogicalId(501);
        config.route_goods_capacities.insert(route_id, Quantity(1));
        let territory = TerritoryConfig {
            routes: vec![DirectedRoute {
                logical_id: route_id,
                from: SettlementId(2),
                to: SettlementId(3),
                cost: 0,
                capacity_per_year: None,
                open_intervals: Vec::new(),
            }],
            ..TerritoryConfig::default()
        };
        initialize(&config, &mut state);
        // Directed commerce requires a strictly cheaper landed source price.
        state
            .economy
            .prices_by_settlement
            .get_mut(&SettlementId(3))
            .expect("destination prices")
            .insert(food.clone(), Money(3));

        let report = close_year(
            &config,
            &mut state,
            Some(&territory),
            YEAR,
            &EconomyYearModifiers::default(),
        );
        assert_eq!(report.trades.len(), 2);
        assert!(report.trades.iter().any(|trade| trade.route_id.is_none()));
        let route_trade = report
            .trades
            .iter()
            .find(|trade| trade.route_id == Some(route_id))
            .expect("one directed shipment fits cargo capacity");
        assert_eq!(route_trade.quantity, Quantity(1));
        assert_eq!(
            settlement_good(&report, SettlementId(3), &food).imports,
            Quantity(1)
        );
        let route_buyer = settlement_good(&report, SettlementId(3), &food);
        assert_eq!(route_buyer.physical_need, Quantity(2));
        assert_eq!(route_buyer.unmet_physical_need, Quantity(1));
        assert_eq!(route_buyer.solvent_demand, Quantity(1));
        assert_eq!(route_buyer.unmet_solvent_demand, Quantity(0));
        let insolvent = settlement_good(&report, SettlementId(4), &food);
        assert_eq!(insolvent.physical_need, Quantity(1));
        assert_eq!(insolvent.unmet_physical_need, Quantity(1));
        assert_eq!(insolvent.solvent_demand, Quantity(0));
    }

    #[test]
    fn insolvent_physical_shortage_does_not_raise_prices_or_production_opportunities() {
        let mut state = state_with_people(&[1, 2], &[(1, 1, Some("farmer")), (2, 1, None)]);
        let food = GoodId::from("food");
        let grain = GoodId::from("grain");
        let config = EconomicConfig {
            goods: BTreeMap::from([
                (food.clone(), definition(2, true, 0)),
                (grain.clone(), definition(2, true, 0)),
            ]),
            needs: BTreeMap::from([
                (
                    food.clone(),
                    chargen_domain::GoodNeed {
                        quantity_per_person_per_year: Quantity(1),
                        priority: 0,
                    },
                ),
                (
                    grain.clone(),
                    chargen_domain::GoodNeed {
                        quantity_per_person_per_year: Quantity(1),
                        priority: 1,
                    },
                ),
            ]),
            profession_policies: BTreeMap::from([(
                ProfessionId::from("farmer"),
                ProfessionEconomicPolicy {
                    recipe: Some(ProductionRecipe {
                        batches_per_worker: 1,
                        maximum_batches_per_year: None,
                        inputs_per_batch: BTreeMap::new(),
                        outputs_per_batch: BTreeMap::from([(food.clone(), Quantity(1))]),
                    }),
                },
            )]),
            price_policy: EconomicPricePolicy {
                adjustment_step_ppm: 500_000,
            },
            production_response: EconomicProductionResponsePolicy {
                enabled: true,
                step_ppm: 500_000,
                minimum_capacity_ppm: 500_000,
                maximum_capacity_ppm: 2_000_000,
            },
            initial_group_endowments: vec![
                endowment(group(1, Some("farmer")), 0, &[("food", 2)]),
                endowment(group(1, None), 0, &[]),
            ],
            ..EconomicConfig::default()
        };
        initialize(&config, &mut state);

        let report = close_year(
            &config,
            &mut state,
            None,
            YEAR,
            &EconomyYearModifiers::default(),
        );
        let food_market = settlement_good(&report, SettlementId(1), &food);
        assert_eq!(food_market.physical_need, Quantity(2));
        assert_eq!(food_market.unmet_physical_need, Quantity(1));
        assert_eq!(food_market.unmet_solvent_demand, Quantity(0));
        assert!(food_market.unsold_offer.0 > 0);
        assert_eq!(food_market.next_price, Money(1));

        let grain_market = settlement_good(&report, SettlementId(1), &grain);
        assert_eq!(grain_market.unmet_physical_need, Quantity(2));
        assert_eq!(grain_market.unmet_solvent_demand, Quantity(0));
        assert_eq!(grain_market.next_price, Money(2));
        assert_eq!(
            state
                .economy
                .pending_production_capacity_ppm_by_group
                .iter()
                .find(|entry| entry.group_id == group(1, Some("farmer")))
                .unwrap()
                .capacity_ppm,
            500_000
        );
        assert!(report.signals.profession_opportunities.is_empty());
    }

    #[test]
    fn production_inputs_bought_after_a4_are_consumed_only_in_the_next_a4() {
        let mut state = state_with_people(&[1], &[(1, 1, Some("baker")), (2, 1, Some("mill"))]);
        let grain = GoodId::from("grain");
        let bread = GoodId::from("bread");
        let baker = ProfessionId::from("baker");
        let mut config = EconomicConfig {
            goods: BTreeMap::from([
                (grain.clone(), definition(2, true, 0)),
                (bread.clone(), definition(2, true, 0)),
            ]),
            profession_policies: BTreeMap::from([(
                baker,
                ProfessionEconomicPolicy {
                    recipe: Some(ProductionRecipe {
                        batches_per_worker: 1,
                        maximum_batches_per_year: None,
                        inputs_per_batch: BTreeMap::from([(grain.clone(), Quantity(1))]),
                        outputs_per_batch: BTreeMap::from([(bread.clone(), Quantity(2))]),
                    }),
                },
            )]),
            initial_group_endowments: vec![
                endowment(group(1, Some("baker")), 10, &[]),
                endowment(group(1, Some("mill")), 0, &[("grain", 1)]),
            ],
            ..EconomicConfig::default()
        };
        config.needs.clear();
        initialize(&config, &mut state);

        let first = close_year(
            &config,
            &mut state,
            None,
            YEAR,
            &EconomyYearModifiers::default(),
        );
        let baker_group = group(1, Some("baker"));
        assert_eq!(first.trades.len(), 1);
        assert_eq!(first.trades[0].purpose, TradePurpose::ProductionInput);
        assert_eq!(
            group_good(&first, &baker_group, &grain).bought_for_inputs,
            Quantity(1)
        );
        assert_eq!(
            group_good(&first, &baker_group, &bread).produced,
            Quantity(0)
        );

        let second_year = Year(21);
        let second = close_year(
            &config,
            &mut state,
            None,
            second_year,
            &EconomyYearModifiers::default(),
        );
        assert_eq!(
            group_good(&second, &baker_group, &grain).input_consumed,
            Quantity(1)
        );
        assert_eq!(
            group_good(&second, &baker_group, &bread).produced,
            Quantity(2)
        );
    }

    #[test]
    fn scarcity_drives_next_year_price_capacity_and_profession_change() {
        let mut state = state_with_people(&[1], &[(1, 1, Some("baker")), (2, 1, None)]);
        let bread = GoodId::from("bread");
        let recipe = || {
            Some(ProductionRecipe {
                batches_per_worker: 1,
                maximum_batches_per_year: None,
                inputs_per_batch: BTreeMap::new(),
                outputs_per_batch: BTreeMap::from([(bread.clone(), Quantity(1))]),
            })
        };
        let config = EconomicConfig {
            goods: BTreeMap::from([(bread.clone(), definition(2, true, 0))]),
            needs: BTreeMap::from([(
                bread.clone(),
                chargen_domain::GoodNeed {
                    quantity_per_person_per_year: Quantity(1),
                    priority: 0,
                },
            )]),
            profession_policies: BTreeMap::from([
                (
                    ProfessionId::from("baker"),
                    ProfessionEconomicPolicy { recipe: recipe() },
                ),
                (
                    ProfessionId::from("farmer"),
                    ProfessionEconomicPolicy { recipe: recipe() },
                ),
            ]),
            price_policy: EconomicPricePolicy {
                adjustment_step_ppm: 500_000,
            },
            production_response: EconomicProductionResponsePolicy {
                enabled: true,
                step_ppm: 1_000_000,
                minimum_capacity_ppm: 500_000,
                maximum_capacity_ppm: 2_000_000,
            },
            labor_policy: EconomicLaborPolicy {
                enabled: true,
                minimum_working_age: 18,
                max_changes_per_settlement_per_year: 1,
            },
            initial_group_endowments: vec![endowment(group(1, None), 2, &[])],
            ..EconomicConfig::default()
        };
        let baker = ProfessionId::from("baker");
        let farmer = ProfessionId::from("farmer");
        let territory = TerritoryConfig {
            professions: vec![
                ProfessionDefinition {
                    id: baker.clone(),
                    name: "Baker".into(),
                },
                ProfessionDefinition {
                    id: farmer.clone(),
                    name: "Farmer".into(),
                },
            ],
            profession_options_by_settlement: BTreeMap::from([(
                SettlementId(1),
                BTreeSet::from([baker, farmer.clone()]),
            )]),
            ..TerritoryConfig::default()
        };
        initialize(&config, &mut state);

        let first = close_year(
            &config,
            &mut state,
            Some(&territory),
            YEAR,
            &EconomyYearModifiers::default(),
        );
        assert_eq!(
            settlement_good(&first, SettlementId(1), &bread).next_price,
            Money(3)
        );
        assert_eq!(
            capacity_for(&state.economy, &group(1, Some("baker"))),
            1_000_000
        );
        assert_eq!(
            state
                .economy
                .pending_production_capacity_ppm_by_group
                .iter()
                .find(|entry| entry.group_id == group(1, Some("baker")))
                .unwrap()
                .capacity_ppm,
            2_000_000
        );
        assert_eq!(state.economy.pending_profession_changes.len(), 1);
        assert_eq!(
            state.economy.pending_profession_changes[0].to,
            Some(farmer.clone())
        );

        let next_year = Year(21);
        let mut events = Vec::new();
        begin_economy_year(
            &config,
            &mut state,
            Some(&territory),
            next_year,
            &mut events,
        )
        .unwrap();
        assert_eq!(
            state.economy.prices_by_settlement[&SettlementId(1)][&bread],
            Money(3)
        );
        assert_eq!(
            capacity_for(&state.economy, &group(1, Some("baker"))),
            2_000_000
        );
        assert_eq!(state.persons[&PersonId(2)].profession, Some(farmer));
        assert!(events
            .iter()
            .any(|event| matches!(&event.kind, HistoricalEventKind::ProfessionChanged { .. })));
        reconcile_economic_groups(&config, &mut state, next_year).unwrap();
        produce_economy(
            &config,
            &mut state,
            next_year,
            &EconomyYearModifiers::default(),
        )
        .unwrap();
        let report = state.economy.working_report.as_ref().unwrap();
        assert_eq!(
            group_good(report, &group(1, Some("baker")), &bread).produced,
            Quantity(2)
        );
    }

    #[test]
    fn a0_clamps_inherited_prices_and_deferred_capacity_to_revised_limits() {
        let mut state = state_with_people(&[1], &[(1, 1, None)]);
        let grain = GoodId::from("grain");
        let mut grain_definition = definition(5, true, 0);
        grain_definition.minimum_price = Money(5);
        grain_definition.maximum_price = Money(10);
        let config = EconomicConfig {
            goods: BTreeMap::from([(grain.clone(), grain_definition)]),
            initial_group_endowments: vec![
                endowment(group(1, None), 8, &[("grain", 7)]),
                endowment(group(1, Some("farmer")), 0, &[]),
            ],
            production_response: EconomicProductionResponsePolicy {
                enabled: true,
                step_ppm: 100_000,
                minimum_capacity_ppm: 1_000_000,
                maximum_capacity_ppm: 2_000_000,
            },
            ..EconomicConfig::default()
        };
        initialize(&config, &mut state);

        state
            .economy
            .prices_by_settlement
            .get_mut(&SettlementId(1))
            .unwrap()
            .insert(grain.clone(), Money(2));
        state
            .economy
            .pending_prices
            .insert(SettlementId(1), BTreeMap::from([(grain.clone(), Money(2))]));
        set_capacity(
            &mut state.economy,
            EconomicProductionCapacity {
                group_id: group(1, Some("farmer")),
                capacity_ppm: 3_000_000,
            },
        );
        state
            .economy
            .pending_production_capacity_ppm_by_group
            .push(EconomicProductionCapacity {
                group_id: group(1, None),
                capacity_ppm: 500_000,
            });
        let money_before = state.economy.groups[0].money;
        let goods_before = state.economy.groups[0].goods.clone();

        begin_economy_year(&config, &mut state, None, Year(21), &mut Vec::new()).unwrap();

        assert_eq!(
            state.economy.prices_by_settlement[&SettlementId(1)][&grain],
            Money(5)
        );
        assert_eq!(
            settlement_good(
                state.economy.working_report.as_ref().unwrap(),
                SettlementId(1),
                &grain
            )
            .opening_price,
            Money(5)
        );
        assert_eq!(
            capacity_for(&state.economy, &group(1, Some("farmer"))),
            2_000_000
        );
        assert_eq!(capacity_for(&state.economy, &group(1, None)), 1_000_000);
        assert_eq!(state.economy.groups[0].money, money_before);
        assert_eq!(state.economy.groups[0].goods, goods_before);
    }

    #[test]
    fn perishable_unsold_market_offer_sets_price_but_reserved_inputs_do_not() {
        let mut state = state_with_people(&[1, 2], &[(1, 1, Some("fisher")), (2, 2, None)]);
        let fish = GoodId::from("fish");
        let bread = GoodId::from("bread");
        let mut config = EconomicConfig {
            goods: BTreeMap::from([
                (fish.clone(), definition(2, false, 0)),
                (bread.clone(), definition(2, true, 0)),
            ]),
            profession_policies: BTreeMap::from([(
                ProfessionId::from("fisher"),
                ProfessionEconomicPolicy {
                    recipe: Some(ProductionRecipe {
                        batches_per_worker: 1,
                        maximum_batches_per_year: None,
                        inputs_per_batch: BTreeMap::from([(fish.clone(), Quantity(1))]),
                        outputs_per_batch: BTreeMap::from([(bread, Quantity(1))]),
                    }),
                },
            )]),
            initial_group_endowments: vec![
                endowment(group(1, Some("fisher")), 0, &[("fish", 1)]),
                endowment(group(2, None), 0, &[("fish", 3)]),
            ],
            ..EconomicConfig::default()
        };
        config.needs.clear();
        initialize(&config, &mut state);

        let modifiers = EconomyYearModifiers {
            incapacitated_person_ids: BTreeSet::from([PersonId(1)]),
            ..EconomyYearModifiers::default()
        };
        let report = close_year(&config, &mut state, None, YEAR, &modifiers);
        let reserved = settlement_good(&report, SettlementId(1), &fish);
        assert_eq!(reserved.unsold_offer, Quantity(0));
        assert_eq!(reserved.next_price, Money(2));
        assert_eq!(
            group_good(&report, &group(1, Some("fisher")), &fish).storage_loss,
            Quantity(1)
        );
        let perishable_surplus = settlement_good(&report, SettlementId(2), &fish);
        assert_eq!(perishable_surplus.unsold_offer, Quantity(3));
        assert_eq!(perishable_surplus.ending_stock, Quantity(0));
        assert_eq!(perishable_surplus.next_price, Money(1));
    }

    #[test]
    fn proportional_migration_then_empty_group_treasury_transfer_conserves_goods_and_money() {
        let mut state =
            state_with_people(&[1, 2], &[(1, 1, Some("farmer")), (2, 1, Some("farmer"))]);
        let grain = GoodId::from("grain");
        let mut config = EconomicConfig {
            goods: BTreeMap::from([(grain.clone(), definition(2, true, 0))]),
            initial_group_endowments: vec![endowment(group(1, Some("farmer")), 9, &[("grain", 5)])],
            asset_continuity: chargen_domain::AssetContinuity::ProportionalShareTransfer,
            empty_group_disposition: EmptyGroupDisposition::TransferToTreasury,
            ..EconomicConfig::default()
        };
        config.needs.clear();
        initialize(&config, &mut state);
        begin_economy_year(&config, &mut state, None, YEAR, &mut Vec::new()).unwrap();

        state.persons.get_mut(&PersonId(1)).unwrap().settlement_id = SettlementId(2);
        reconcile_economic_groups(&config, &mut state, YEAR).unwrap();
        let source = group(1, Some("farmer"));
        let destination = group(2, Some("farmer"));
        assert_eq!(state.economy.group(&source).unwrap().money, Money(5));
        assert_eq!(
            state.economy.group(&source).unwrap().goods[&grain],
            Quantity(3)
        );
        assert_eq!(state.economy.group(&destination).unwrap().money, Money(4));
        assert_eq!(
            state.economy.group(&destination).unwrap().goods[&grain],
            Quantity(2)
        );

        state.persons.get_mut(&PersonId(2)).unwrap().death_year = Some(YEAR);
        reconcile_economic_groups(&config, &mut state, YEAR).unwrap();
        assert_eq!(state.economy.group(&source).unwrap().money, Money(0));
        assert_eq!(state.economy.treasury.money, Money(5));
        assert_eq!(state.economy.treasury.goods[&grain], Quantity(3));
        let report = finalize_economy_year(&config, &mut state, YEAR).unwrap();
        assert_eq!(group_ledger(&report, &source).transfers_out, Money(9));
        assert_eq!(group_ledger(&report, &destination).transfers_in, Money(4));
        assert_eq!(report.treasury_goods[&grain].transfers_in, Quantity(3));
        assert_eq!(report.money.closing_group_money, Money(4));
        assert_eq!(report.money.closing_treasury_money, Money(5));
    }

    #[test]
    fn checked_stock_arithmetic_rejects_production_overflow() {
        let mut state = state_with_people(&[1], &[(1, 1, Some("farmer"))]);
        let bread = GoodId::from("bread");
        let config = EconomicConfig {
            goods: BTreeMap::from([(bread.clone(), definition(2, true, 0))]),
            profession_policies: BTreeMap::from([(
                ProfessionId::from("farmer"),
                ProfessionEconomicPolicy {
                    recipe: Some(ProductionRecipe {
                        batches_per_worker: 1,
                        maximum_batches_per_year: None,
                        inputs_per_batch: BTreeMap::new(),
                        outputs_per_batch: BTreeMap::from([(bread.clone(), Quantity(1))]),
                    }),
                },
            )]),
            initial_group_endowments: vec![endowment(
                group(1, Some("farmer")),
                0,
                &[("bread", i64::MAX)],
            )],
            ..EconomicConfig::default()
        };
        initialize(&config, &mut state);
        begin_economy_year(&config, &mut state, None, YEAR, &mut Vec::new()).unwrap();
        reconcile_economic_groups(&config, &mut state, YEAR).unwrap();
        assert!(matches!(
            produce_economy(&config, &mut state, YEAR, &EconomyYearModifiers::default()),
            Err(EconomyError::Arithmetic("production output"))
        ));
    }
}
