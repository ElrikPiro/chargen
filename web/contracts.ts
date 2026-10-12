/** Versioned wire contracts. IDs in scenario JSON stay within JS safe integers. */
export type Year = number;
export type DomainId = number;
/** Canonical wire form for stable simulation identity; Rust accepts legacy safe JSON integers on input. */
export type LogicalId = string;
export type TraitKey = `${string}::${string}`;
export type TraitValues = Partial<Record<TraitKey, number>>;

export interface ScenarioBase {
  world_id: DomainId;
  schema_version: 1 | 2;
  rules_version: 1 | 2;
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

export type Scenario = ScenarioV1 | ScenarioV2;

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
  professions: Array<{ profession_id: ProfessionId; validity: YearInterval }>;
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

export interface ExportResponse {
  export_id: string;
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

export interface HistoricalEvent {
  id: DomainId;
  year: Year;
  order: number;
  affected_entities: Array<{ entity: string; id: DomainId }>;
  kind: { type: string; data?: unknown };
}

export interface RestoreResponse {
  restored_id: string;
  database_file: string;
  worlds: string[];
  restart_required: boolean;
}
