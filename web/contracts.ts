/** Versioned wire contracts. IDs in scenario JSON stay within JS safe integers. */
export type Year = number;
export type DomainId = number;
/** Canonical wire form for stable simulation identity; Rust accepts legacy safe JSON integers on input. */
export type LogicalId = string;
export type TraitKey = `${string}::${string}`;
export type TraitValues = Partial<Record<TraitKey, number>>;

export interface Scenario {
  world_id: DomainId;
  schema_version: number;
  rules_version: number;
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
