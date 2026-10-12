/** Versioned wire contracts. IDs in scenario JSON stay within JS safe integers. */
export type Year = number;
export type DomainId = number;
/** Canonical wire form for stable simulation identity; Rust accepts legacy safe JSON integers on input. */
export type LogicalId = string;
export type TraitKey = `${string}::${string}`;
export type TraitValues = Partial<Record<TraitKey, number>>;
/** i64 accounting quantities are serialized as canonical decimal strings. */
export type EconomicAmount = string;

export interface ScenarioBase {
  world_id: DomainId;
  schema_version: 1 | 2 | 3;
  rules_version: 1 | 2 | 3;
  seed: DomainId;
  start_year: Year;
  settlements: Settlement[];
  initial_people: Person[];
  initial_households: Household[];
  initial_unions: Union[];
  initial_family_links: FamilyLink[];
  trait_catalog: TraitDefinition[];
  initial_population_profiles: InitialPopulationProfile[];
  policies: DemographicPolicies;
  genetics: GeneticCatalog;
}

export interface ScenarioV1 extends ScenarioBase {
  schema_version: 1;
  rules_version: 1;
}

export interface ScenarioV2 extends ScenarioBase {
  schema_version: 2;
  rules_version: 2;
  territory: TerritoryConfig;
  culture: CultureConfig;
}

export interface ScenarioV3 extends ScenarioBase {
  schema_version: 3;
  rules_version: 3;
  territory: TerritoryConfig;
  culture: CultureConfig;
  economy: EconomicConfig;
  scheduled_events: ScheduledEvent[];
}

export type Scenario = ScenarioV1 | ScenarioV2 | ScenarioV3;

export interface Settlement {
  id: DomainId;
  logical_id: LogicalId;
  name: string;
  culture_values: TraitValues;
}

export type Gamete = "ovum" | "sperm" | "other";

export interface Person {
  id: DomainId;
  logical_id: LogicalId;
  birth_year: Year;
  death_year: Year | null;
  settlement_id: DomainId;
  household_id: DomainId | null;
  reproductive: ReproductiveProfile;
  traits: TraitValues;
  genome: Genome;
  last_widowed_year: Year | null;
  profession?: ProfessionId;
}

export type ProfessionId = string;
export type MigrationUnit = "person" | "household";

export interface DirectedRoute {
  logical_id: LogicalId;
  from: DomainId;
  to: DomainId;
  cost: number;
  capacity_per_year: number | null;
  open_intervals?: YearInterval[];
}

/** Historical API projection omits future opening-calendar details. */
export interface OpenRouteAtClose {
  logical_id: LogicalId;
  from: DomainId;
  to: DomainId;
  cost: number;
  capacity_per_year: number | null;
}

export interface MigrationPolicy {
  enabled: boolean;
  probability_ppm: number;
  unit: MigrationUnit;
  minimum_age: number;
  maximum_age: number | null;
  candidate_limit: number;
  max_moving_units_per_year: number;
  maximum_travel_cost: number | null;
  minimum_net_opportunity_gain: number;
}

export interface ForcedDisplacementCommand {
  id: LogicalId;
  person_ids: DomainId[];
  destination: DomainId;
}

export interface ScheduledDisplacement {
  year: Year;
  command: ForcedDisplacementCommand;
}

export interface ProfessionDefinition { id: ProfessionId; name: string }

export interface TerritoryConfig {
  route_revision: number;
  routes: DirectedRoute[];
  settlement_capacities?: Record<string, number>;
  opportunity_scores?: Record<string, number>;
  migration: MigrationPolicy;
  scheduled_displacements?: ScheduledDisplacement[];
  professions?: ProfessionDefinition[];
  profession_options_by_settlement?: Record<string, ProfessionId[]>;
}

export type CultureEligibility =
  | { kind: "all_living" }
  | { kind: "minimum_age"; minimum_age: number }
  | { kind: "age_range"; minimum_age: number; maximum_age: number };

export interface CultureDimensionPolicy {
  eligibility: CultureEligibility;
  member_weight: number;
  parent_weight: number;
  local_weight: number;
  variation_max: number;
  minimum: number;
  maximum: number;
  founder_value: number | null;
}

export type InstitutionKind = "institution" | "religion";
export type InstitutionScope = "world" | "settlement";

export interface AffiliationPolicy {
  enabled: boolean;
  minimum_age: number;
  maximum_age: number | null;
  minimum_affinity_ppm: number;
  opportunity_ppm: number;
  candidate_limit: number;
  maximum_affiliations_per_person: number | null;
  continue_after_migration: boolean;
}

export interface InstitutionDefinition {
  id: string;
  name: string;
  kind: InstitutionKind;
  scope: InstitutionScope;
  allowed_settlements?: DomainId[] | null;
  values: TraitValues;
  affiliation: AffiliationPolicy;
}

export interface OrganizationPolicy {
  enabled: boolean;
  cadence_years: number;
  candidate_limit_per_settlement: number;
  maximum_organizations_per_settlement: number;
  minimum_founders: number;
  maximum_members: number;
  dissolve_below_members: number;
  minimum_age: number;
  maximum_age: number | null;
  minimum_affinity_ppm: number;
  formation_opportunity_ppm: number;
  continue_after_migration: boolean;
}

export interface CultureConfig {
  dimensions: Partial<Record<TraitKey, CultureDimensionPolicy>>;
  institutions?: InstitutionDefinition[];
  organizations: OrganizationPolicy;
}

export interface ReproductiveProfile {
  gametes: Gamete[];
  can_gestate: boolean;
  fertility_enabled: boolean;
}

export type HouseholdKind = "family" | "collective" | "single_person" | "institution" | { other: string };

export interface Household {
  id: DomainId;
  logical_id: LogicalId;
  kind: HouseholdKind;
  settlement_id: DomainId;
  members: DomainId[];
  closed_year: Year | null;
}

export type UnionKind = "partnership" | "marriage" | { other: string };

export interface Union {
  id: DomainId;
  logical_id: LogicalId;
  participants: DomainId[];
  kind: UnionKind;
  validity: YearInterval;
}

export type FamilyLinkKind = "biological_parent" | "adoptive_parent" | "caregiver";

export interface FamilyLink {
  id: DomainId;
  logical_id: LogicalId;
  parent: DomainId;
  child: DomainId;
  kind: FamilyLinkKind;
  validity: YearInterval;
}

/** Half-open period: start is included and end_exclusive is excluded. */
export interface YearInterval { start: Year; end_exclusive: Year | null }

export interface TraitDefinition {
  key: TraitKey;
  minimum: number;
  maximum: number;
}

export interface InitialPopulationProfile {
  settlement_id: DomainId;
  population_size: number;
  household_size_minimum: number;
  household_size_maximum: number;
  household_kind: HouseholdKind;
  age_bands: Array<{ minimum_age: number; maximum_age: number; weight: number }>;
  reproductive_profiles: Array<{ profile: ReproductiveProfile; weight: number }>;
  trait_distributions: Partial<Record<TraitKey, Array<{ value: number; weight: number }>>>;
  profession_distribution?: Array<{ profession_id: ProfessionId | null; weight: number }>;
}

export interface EconomicGroupId {
  settlement_id: DomainId;
  profession_id: ProfessionId | null;
}

export interface GoodDefinition {
  name: string;
  unit: string;
  transport_units_per_unit: number;
  storable: boolean;
  annual_storage_loss_ppm: number;
  initial_price: EconomicAmount;
  minimum_price: EconomicAmount;
  maximum_price: EconomicAmount;
}

export interface ProductionRecipe {
  batches_per_worker: number;
  maximum_batches_per_year?: number | null;
  inputs_per_batch?: Record<string, EconomicAmount>;
  outputs_per_batch?: Record<string, EconomicAmount>;
}

export interface EconomicGroupEndowment {
  money: EconomicAmount;
  goods?: Record<string, EconomicAmount>;
}

export interface EconomicConfig {
  goods?: Record<string, GoodDefinition>;
  profession_policies?: Record<ProfessionId, { recipe?: ProductionRecipe | null }>;
  needs?: Record<string, { quantity_per_person_per_year: EconomicAmount; priority: number }>;
  route_goods_capacities?: Record<LogicalId, EconomicAmount>;
  transport_money_per_route_cost_unit?: EconomicAmount;
  transport_cost_account?: "treasury" | "external_sink";
  initial_group_endowments?: Array<{ group_id: EconomicGroupId; endowment: EconomicGroupEndowment }>;
  initial_treasury?: EconomicGroupEndowment;
  price_policy?: { adjustment_step_ppm: number };
  buyer_priority?: "seeded_logical_rank" | "group_key_order";
  asset_continuity?: "retain_group_assets" | "proportional_share_transfer";
  empty_group_disposition?: "retain_dormant" | "transfer_to_consumer_group" | "transfer_to_treasury";
  labor_policy?: { enabled: boolean; minimum_working_age: number; max_changes_per_settlement_per_year: number };
  responses?: { mortality_increase_at_full_scarcity_ppm: number; birth_reduction_at_full_scarcity_ppm: number; migration_opportunity_at_full_scarcity: number };
  production_response?: { enabled: boolean; step_ppm: number; minimum_capacity_ppm: number; maximum_capacity_ppm: number };
}

export type EventSelection =
  | { mode: "exact"; data: { count: number; overflow: "error" | "clamp_with_warning" } }
  | { mode: "probabilistic"; data: { probability_ppm: number } };

export interface ScheduledEvent {
  id: LogicalId;
  name: string;
  start_year: Year;
  duration_years?: number | null;
  priority: number;
  population_filter?: {
    settlement_ids?: DomainId[] | null;
    minimum_age?: number | null;
    maximum_age?: number | null;
    profession_ids?: ProfessionId[] | null;
  };
  effects: EventEffect[];
}

export type EventEffect =
  | { type: "deaths"; data: { selection: EventSelection } }
  | { type: "work_incapacity"; data: { selection: EventSelection } }
  | { type: "production_loss"; data: { loss_ppm: number } }
  | { type: "stock_destruction"; data: { loss_ppm: number } }
  | { type: "route_closure"; data: { route_ids: LogicalId[] } }
  | { type: "displacement"; data: { selection: EventSelection; destination: DomainId } };

export interface EconomicGroupLedger {
  group_id: EconomicGroupId;
  member_count_before: number;
  worker_count_before: number;
  member_count_after: number;
  worker_count_after: number;
  opening_money: EconomicAmount;
  sales_income: EconomicAmount;
  purchases_spent: EconomicAmount;
  transfers_in: EconomicAmount;
  transfers_out: EconomicAmount;
  closing_money: EconomicAmount;
  goods: Record<string, EconomicGoodLedger>;
}

export interface EconomicGoodLedger {
  opening_stock: EconomicAmount;
  produced: EconomicAmount;
  input_consumed: EconomicAmount;
  input_replenishment_target: EconomicAmount;
  unmet_input_demand: EconomicAmount;
  transfers_in: EconomicAmount;
  transfers_out: EconomicAmount;
  bought: EconomicAmount;
  bought_for_human_need: EconomicAmount;
  bought_for_inputs: EconomicAmount;
  sold: EconomicAmount;
  consumer_consumed: EconomicAmount;
  storage_loss: EconomicAmount;
  event_destroyed: EconomicAmount;
  closing_stock: EconomicAmount;
  physical_need: EconomicAmount;
  unmet_physical_need: EconomicAmount;
  solvent_demand: EconomicAmount;
  unmet_solvent_demand: EconomicAmount;
  solvent_input_demand: EconomicAmount;
  unmet_solvent_input_demand: EconomicAmount;
}

export interface EconomicTrade {
  source_group: EconomicGroupId;
  destination_group: EconomicGroupId;
  good_id: string;
  quantity: EconomicAmount;
  seller_revenue: EconomicAmount;
  buyer_payment: EconomicAmount;
  transport_cost: EconomicAmount;
  route_id: LogicalId | null;
  purpose: "human_need" | "production_input";
}

export type EventWarning =
  | { kind: "exact_count_clamped"; data: { requested_count: number; eligible_count: number; realized_count: number } }
  | { kind: "displacement_skipped_already_moved"; data: { skipped_count: number } };

export interface EventMove {
  person_ids: DomainId[];
  from: DomainId;
  to: DomainId;
  /** Directed route IDs used by this realized movement. */
  route_ids: LogicalId[];
}

export type EventRealization =
  | { kind: "people"; data: { selected_person_ids: DomainId[]; requested_count: number; realized_count: number; warning: EventWarning | null } }
  | { kind: "production_loss"; data: { settlement_ids: DomainId[]; profession_ids: ProfessionId[] | null; loss_ppm: number } }
  | { kind: "stock_destruction"; data: { losses: Array<{ event_id: LogicalId; group_id: EconomicGroupId; good_id: string; requested_loss_ppm: number; destroyed: EconomicAmount }> } }
  | { kind: "route_closure"; data: { route_ids: LogicalId[] } }
  | { kind: "displacement"; data: { requested_person_ids: DomainId[]; moved_person_ids: DomainId[]; moves: EventMove[]; requested_count: number; realized_count: number; warning: EventWarning | null } };

export type HistoricalEventKind =
  | { type: "scheduled_event_occurred"; data: { event_id: LogicalId; name: string; priority: number } }
  | { type: "event_effect_realized"; data: { event_id: LogicalId; event_name: string; effect_index: number; effect: EventEffect; realization: EventRealization } }
  | { type: "economic_year_closed"; data: { summary: EconomicYearClosedSummary } }
  | { type: string; data: Record<string, unknown> };

export interface EconomicYearClosedSummary {
  year: Year;
  settlement_ids: DomainId[];
  group_ids: EconomicGroupId[];
  money: EconomicMoneyLedger;
  physical_shortage_totals: SettlementPhysicalShortageSummary[];
}

export interface SettlementPhysicalShortageSummary {
  settlement_id: DomainId;
  goods: Record<string, EconomicAmount>;
}

export interface HistoricalEvent {
  id: LogicalId;
  year: Year;
  order: number;
  affected_entities: Array<Record<string, unknown>>;
  kind: HistoricalEventKind;
}

export interface MarketsPage {
  world_id: string;
  branch_id: string;
  year: Year;
  enabled: boolean;
  total: number;
  limit: number;
  offset: number;
  money: EconomicMoneyLedger | null;
  treasury_goods: Record<string, EconomicGoodLedger>;
  groups: EconomicGroupLedger[];
}

export interface EconomicMoneyLedger {
  opening_group_money: EconomicAmount;
  opening_treasury_money: EconomicAmount;
  closing_group_money: EconomicAmount;
  closing_treasury_money: EconomicAmount;
  external_transport_outflow: EconomicAmount;
  explicit_emission: EconomicAmount;
  explicit_retirement: EconomicAmount;
}

export interface MarketTradesPage {
  world_id: string;
  branch_id: string;
  year: Year;
  enabled: boolean;
  total: number;
  limit: number;
  offset: number;
  trades: EconomicTrade[];
}

export interface SettlementGoodMarketLedger {
  good_id: string;
  opening_price: EconomicAmount;
  next_price: EconomicAmount;
  physical_need: EconomicAmount;
  unmet_physical_need: EconomicAmount;
  solvent_demand: EconomicAmount;
  unmet_solvent_demand: EconomicAmount;
  input_replenishment_target: EconomicAmount;
  unmet_input_demand: EconomicAmount;
  solvent_input_demand: EconomicAmount;
  unmet_solvent_input_demand: EconomicAmount;
  local_traded: EconomicAmount;
  imports: EconomicAmount;
  exports: EconomicAmount;
  unsold_offer: EconomicAmount;
  ending_stock: EconomicAmount;
}

export interface SettlementEconomicLedger {
  settlement_id: DomainId;
  goods: SettlementGoodMarketLedger[];
}

export interface SettlementMarketsPage {
  world_id: string;
  branch_id: string;
  year: Year;
  enabled: boolean;
  total: number;
  limit: number;
  offset: number;
  settlements: SettlementEconomicLedger[];
}

export interface HistoricalEventsPage {
  world_id: string;
  branch_id: string;
  year: Year;
  total: number;
  limit: number;
  offset: number;
  events: HistoricalEvent[];
}

export interface DemographicPolicies {
  mortality: MortalityPolicy;
  unions: UnionPolicy;
  births: BirthPolicy;
  adoption: AdoptionPolicy;
  traits: Record<TraitKey, TraitInheritancePolicy>;
}

export interface MortalityPolicy {
  default_probability_ppm: number;
  age_bands: Array<{ minimum_age: number; maximum_age: number | null; probability_ppm: number }>;
}

export interface UnionPolicy {
  enabled: boolean;
  minimum_age: number;
  maximum_age: number | null;
  formation_probability_ppm: number;
  separation_probability_ppm: number;
  candidate_limit: number;
  formation_attempts_per_person: number;
  max_active_unions_per_person: number | null;
  prohibited_kinship_generations: number;
  allow_widowed_remarriage: boolean;
  kind: UnionKind;
}

export interface BirthPolicy {
  enabled: boolean;
  probability_ppm: number;
  minimum_gestational_age: number;
  maximum_gestational_age: number;
  require_active_union: boolean;
  required_contributor_gamete: Gamete | null;
  minimum_contributor_age: number;
  allow_unknown_contributor: boolean;
  maximum_births_per_person_per_year: number;
  minimum_years_between_births: number;
  infant_mortality_probability_ppm: number;
  child_reproductive_profiles: Array<{ profile: ReproductiveProfile; weight: number }>;
}

export interface AdoptionPolicy {
  enabled: boolean;
  adopt_orphans: boolean;
  adoption_probability_ppm: number;
  candidate_limit: number;
  minor_age: number;
  minimum_caregiver_age: number;
  maximum_adoptions_per_year: number;
  maximum_children_per_household: number | null;
  maximum_caregivers_per_child: number;
  move_to_adoptive_household: boolean;
  add_household_care_links: boolean;
}

export interface TraitInheritancePolicy {
  parent_weight: number;
  local_weight: number;
  variation_max: number;
  minimum: number;
  maximum: number;
}

export interface GeneticCatalog {
  loci: GeneticLocus[];
}

export interface GeneticLocus {
  id: string;
  variants: string[];
  founder_distribution: Array<{ variant: string; weight: number }>;
  expression: GeneticExpression;
}

export type GeneticExpression =
  | { mode: "dominant"; dominant_variants: string[]; dominant_value: number; recessive_value: number }
  | { mode: "additive"; values: Record<string, number> };

export interface Genome {
  loci: Record<string, [Allele, Allele]>;
}

export interface Allele {
  variant: string;
  lineage: LogicalId | null;
  inherited_from: LogicalId | null;
}

export interface ValidationIssue {
  path: string;
  code: string;
  message: string;
}

export interface ApiError {
  code: string;
  message: string;
  details?: unknown;
}

export interface HealthResponse {
  api_version: 1;
  version: string;
  offline: true;
}

export interface ValidateScenarioResponse {
  valid: boolean;
  issues: ValidationIssue[];
}

export interface WorldSummary {
  world_id: string;
  created_at: string;
  root_branch_id: string;
  branch_count: number;
  latest_year: Year | null;
}

export interface BranchSummary {
  world_id: string;
  branch_id: string;
  parent_branch_id: string | null;
  fork_year: Year | null;
  head_year: Year | null;
  revision_id: number;
}

export interface CreateBranchRequest {
  parent_branch_id: string;
  branch_id: string;
  /** null forks the scenario before its first annual close. */
  cut_year: Year | null;
  scenario: Scenario;
}

export interface CreateBranchResponse {
  world_id: string;
  parent_branch_id: string;
  branch_id: string;
  cut_year: Year | null;
  head_year: Year | null;
}

export type RunStatus = "queued" | "running" | "paused" | "completed" | "cancelled" | "failed";

export interface RunJob {
  job_id: string;
  world_id: string;
  branch_id: string;
  status: RunStatus;
  start_year: Year;
  through_year: Year;
  current_year: Year | null;
  completed_years: number;
  total_years: number;
  message?: string;
}

export interface CensusRow {
  settlement_id: DomainId;
  settlement_name: string;
  population: number;
}

export interface CensusResponse {
  world_id: string;
  branch_id: string;
  year: Year;
  total_population: number;
  settlements: CensusRow[];
  limit: number;
  offset: number;
}

export interface HistoricalPage<T> {
  world_id: string;
  branch_id: string;
  year: Year;
  total: number;
  limit: number;
  offset: number;
  items: T[];
}

export interface SettlementView {
  settlement_id: DomainId;
  settlement_name: string;
  population: number;
  capacity: number | null;
  opportunity_score: number | null;
  balance: SettlementYearBalance | null;
}

export interface SettlementYearBalance {
  settlement_id: DomainId;
  before: number;
  births: number;
  deaths: number;
  incoming: number;
  outgoing: number;
  after: number;
}

export interface RoutesPage extends HistoricalPage<OpenRouteAtClose> {
  route_revision: number | null;
}

export interface CultureTraitAtClose {
  key: TraitKey;
  weighted_sum: number;
  total_weight: number;
  member_count: number;
  current_value: number | null;
  minimum: number | null;
  maximum: number | null;
}

export interface SettlementCultureAtClose {
  settlement_id: DomainId;
  settlement_name: string;
  traits: CultureTraitAtClose[];
}

export interface InstitutionMembership {
  institution_id: string;
  person_id: DomainId;
  settlement_id: DomainId;
  validity: YearInterval;
}

export interface InstitutionAtClose {
  definition: InstitutionDefinition;
  active_affiliation_count: number;
}

export interface InstitutionMembersPage {
  world_id: string;
  branch_id: string;
  year: Year;
  institution_id: string;
  total: number;
  limit: number;
  offset: number;
  affiliations: InstitutionMembership[];
}

export interface OrganizationMembership {
  person_id: DomainId;
  validity: YearInterval;
  founder: boolean;
}

export interface OrganizationAtClose {
  organization: {
    id: DomainId;
    logical_id: LogicalId;
    name: string;
    settlement_id: DomainId;
    founded_year: Year;
    dissolved_year: Year | null;
    founders: DomainId[];
    culture_values: CultureTraitAtClose[];
  };
  active_at_close: boolean;
  active_member_count: number;
}

export interface OrganizationMembersPage {
  world_id: string;
  branch_id: string;
  year: Year;
  organization_id: DomainId;
  total: number;
  limit: number;
  offset: number;
  member_ids: DomainId[];
}

export interface PersonResidenceAtClose {
  world_id: string;
  branch_id: string;
  year: Year;
  person: PersonRecord;
  residences: Array<{ settlement_id: DomainId; validity: YearInterval }>;
  professions: ProfessionAtClose[];
}

export interface PersonAffiliationsAtClose {
  world_id: string;
  branch_id: string;
  year: Year;
  person_id: DomainId;
  institutions: Array<{ institution_id: string; settlement_id: DomainId; validity: YearInterval }>;
  organizations: Array<{ organization_id: DomainId; organization_name: string; settlement_id: DomainId; founder: boolean; validity: YearInterval }>;
}

export interface PersonRecord {
  person: Person;
  age_at_close: number | null;
  alive_at_close: boolean;
}

export interface GenealogyResponse {
  world_id: string;
  branch_id: string;
  year: Year;
  subject: PersonRecord;
  ancestors: GenealogyRelation[];
  descendants: GenealogyRelation[];
  truncated: boolean;
}

export interface GenealogyRelation {
  relative: PersonRecord;
  link: FamilyLink;
  distance: number;
}

/** New paginated E4 row; the legacy genealogy response remains unchanged. */
export interface GenealogyPageRelation extends GenealogyRelation {
  presentation: PersonPresentation;
}

export interface ExportResponse {
  export_id: string;
  download_url: string;
  manifest: {
    format_version: number;
    schema_version: number;
    world_ids: string[];
    database_file: string;
    byte_length: number;
    checksum_fnv1a64: string;
  };
}

export interface PersonHistoryResponse {
  world_id: string;
  branch_id: string;
  year: Year;
  person: PersonRecord;
  events: HistoricalEvent[];
}

export interface RestoreResponse {
  restored_id: string;
  database_file: string;
  worlds: string[];
  restart_required: boolean;
  api_base: string;
}

/** E4 temporal metadata. A close's revision may be inherited from its parent branch. */
export interface ScenarioRevisionResponse {
  world_id: string;
  branch_id: string;
  year: Year | null;
  revision_id: number;
  scenario: Scenario;
}

export interface ScenarioSchemaResponse {
  version: 1 | 2 | 3;
  schema: Record<string, unknown>;
}

export interface RestoreCatalogEntry {
  restored_id: string;
  api_base: string;
  worlds: string[];
  available: boolean;
  error?: ApiError;
}

export interface RestoreCatalogResponse {
  restores: RestoreCatalogEntry[];
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
}

export type RestoreResponseE4 = RestoreResponse;

export type PersonVisibility =
  | { status: "alive" }
  | { status: "deceased"; death_year: Year }
  | { status: "not_known_at_year" };

export interface PersonPresentation {
  name: string;
  descriptive_traits: string[];
}

export interface PersonPresentationRow {
  person_id: DomainId;
  presentation: PersonPresentation;
  status: PersonVisibility;
  age_at_close: number | null;
  age_at_death: number | null;
  settlement_id: DomainId;
  household_id: DomainId | null;
}

export interface PersonTraitFact { key: TraitKey; value: number; description: string | null }
export interface ProfessionAtClose {
  profession_id: ProfessionId;
  profession_name: string | null;
  validity: YearInterval;
}
export interface ResidencePresentation {
  settlement_id: DomainId;
  settlement_name: string | null;
  validity: YearInterval;
}
export interface InstitutionAffiliationPresentation {
  institution_id: string;
  institution_name: string | null;
  settlement_id: DomainId;
  settlement_name: string | null;
  validity: YearInterval;
  active_at_close: boolean;
}
export interface OrganizationAffiliationPresentation {
  organization_id: DomainId;
  organization_name: string;
  settlement_id: DomainId;
  settlement_name: string | null;
  founder: boolean;
  validity: YearInterval;
  active_at_close: boolean;
}
export interface PersonGenomeLocus {
  locus_id: string;
  alleles: [Allele, Allele];
  expressed_value: number | null;
}
export interface FamilyLinkPresentation {
  direction: "parent" | "child";
  relative: PersonPresentationRow;
  link: FamilyLink;
}
export interface ChronologyEntry { year: Year; order: number; event_id: LogicalId; text_es: string }

export interface PersonDossier {
  world_id: string;
  branch_id: string;
  year: Year;
  template_version: number;
  fact_fingerprint: string | null;
  person_id: DomainId;
  status: PersonVisibility;
  record: PersonRecord | null;
  presentation: PersonPresentation | null;
  birth_year: Year | null;
  death_year: Year | null;
  age_at_close: number | null;
  age_at_death: number | null;
  settlement_id: DomainId | null;
  settlement_name: string | null;
  household_id: DomainId | null;
  profession: ProfessionId | null;
  professions: ProfessionAtClose[];
  traits: PersonTraitFact[];
  genome: PersonGenomeLocus[];
  residences: ResidencePresentation[];
  institution_affiliations: InstitutionAffiliationPresentation[];
  organization_affiliations: OrganizationAffiliationPresentation[];
  family_links: FamilyLinkPresentation[];
  family_links_total: number;
  family_links_truncated: boolean;
  unions: PersonUnionAtClose[];
  unions_total: number;
  unions_truncated: boolean;
  timeline: ChronologyEntry[];
  timeline_total: number;
  timeline_truncated: boolean;
  revision_id: number;
}

export interface PersonUnionAtClose {
  union_id: DomainId;
  logical_id: LogicalId;
  kind: UnionKind;
  validity: YearInterval;
  active_at_close: boolean;
  participants: PersonPresentationRow[];
  participants_total: number;
  participants_truncated: boolean;
}

export interface PersonUnionsPage {
  world_id: string;
  branch_id: string;
  year: Year;
  person_id: DomainId;
  offset: number;
  total: number;
  truncated: boolean;
  unions: PersonUnionAtClose[];
  revision_id: number;
}

export interface UnionParticipantsPage {
  world_id: string;
  branch_id: string;
  year: Year;
  union_id: DomainId;
  offset: number;
  total: number;
  truncated: boolean;
  participants: PersonPresentationRow[];
  revision_id: number;
}

export interface HouseholdRow {
  household_id: DomainId;
  kind: HouseholdKind;
  settlement_id: DomainId;
  active_at_close: boolean;
  member_count: number;
}
export interface HouseholdsPage {
  world_id: string;
  branch_id: string;
  year: Year;
  total: number;
  limit: number;
  offset: number;
  households: HouseholdRow[];
  revision_id: number;
}
export interface HouseholdDossier {
  world_id: string;
  branch_id: string;
  year: Year;
  household_id: DomainId;
  kind: HouseholdKind;
  settlement_id: DomainId;
  active_at_close: boolean;
  closed_year: Year | null;
  total: number;
  offset: number;
  members: PersonPresentationRow[];
  revision_id: number;
}
export interface SettlementView {
  settlement_id: DomainId;
  settlement_name: string;
  population: number;
  capacity: number | null;
  opportunity_score: number | null;
  balance: SettlementYearBalance | null;
}
export interface SettlementYearBalance {
  settlement_id: DomainId;
  before: number;
  births: number;
  deaths: number;
  incoming: number;
  outgoing: number;
  after: number;
}
export interface SettlementCultureAtClose {
  settlement_id: DomainId;
  settlement_name: string;
  traits: Array<{
    key: TraitKey;
    weighted_sum: number;
    total_weight: number;
    member_count: number;
    current_value: number | null;
    minimum: number | null;
    maximum: number | null;
  }>;
}
export interface SettlementDossier {
  world_id: string;
  branch_id: string;
  year: Year;
  settlement: SettlementView;
  culture: SettlementCultureAtClose | null;
  household_count: number;
  total: number;
  offset: number;
  people: PersonPresentationRow[];
  revision_id: number;
}
export interface GenealogyPage {
  world_id: string;
  branch_id: string;
  year: Year;
  subject: PersonRecord;
  direction: "ancestors" | "descendants";
  relation_kinds: FamilyLinkKind[];
  maximum_depth: number;
  offset: number;
  total: number;
  truncated: boolean;
  relations: GenealogyPageRelation[];
  revision_id: number;
}
export interface PersonEventsPage {
  world_id: string;
  branch_id: string;
  year: Year;
  person_id: DomainId;
  total: number;
  offset: number;
  events: HistoricalEvent[];
  revision_id: number;
}

export interface ComparisonContext {
  branch_id: string;
  year: Year;
  revision_id: number;
  economy_enabled: boolean;
  money: EconomicMoneyLedger | null;
  treasury_goods: Record<string, EconomicGoodLedger>;
}
export interface ComparisonSettlementSide {
  settlement: SettlementView;
  balance: SettlementYearBalance | null;
  economy: SettlementEconomicLedger | null;
  culture: SettlementCultureAtClose | null;
}
export interface BranchComparisonResponse {
  world_id: string;
  left: ComparisonContext;
  right: ComparisonContext;
  total: number;
  limit: number;
  offset: number;
  items: Array<{
    settlement_id: DomainId;
    left: ComparisonSettlementSide | null;
    right: ComparisonSettlementSide | null;
  }>;
}

/** GET /runs/{job_id}/progress emits `event: progress` with this JSON payload. */
export interface RunSnapshot {
  job_id: string;
  world_id: string;
  branch_id: string;
  status: "queued" | "running" | "paused" | "completed" | "cancelled" | "failed";
  start_year: Year;
  through_year: Year;
  current_year: Year | null;
  completed_years: number;
  total_years: number;
  message: string | null;
}
