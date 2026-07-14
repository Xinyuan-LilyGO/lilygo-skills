// Ambient data-structure types for the LilyGO JS context kernel.
//
// This file has no top-level import/export, so every type below is a GLOBAL
// (ambient) type usable from JSDoc annotations in bin/*.mjs without importing.
// The shapes mirror data/facts/board-fact-packs.json and data/boards.json so a
// mistyped field name fails `tsc --checkJs --strict` (the CI type gate).

/** A source citation triple attached to every fact (official URL + optional
 *  line range + sha256), mirroring the Rust `SourceFactSource`. */
interface SourceRef {
  kind: string;
  path_or_url: string;
  /** Present only for line-anchored, single-file-verifiable facts. */
  line_range?: string;
  hash: string;
}

/** One source-backed board fact (Rust `SourceFact`). */
interface Fact {
  schema_version: number;
  board_id: string;
  topic: string;
  key: string;
  value: string;
  claim: string;
  source: SourceRef;
  authority_rank: number;
  evidence_level: string;
  stale: boolean;
  confidence: string;
}

/** A per-board fact pack (Rust `BoardFactPack`). */
interface FactPack {
  schema_version: number;
  board_id: string;
  mcu_family: string;
  supported: boolean;
  pin_matrix: Fact[];
  bus_matrix: Fact[];
  expander_matrix: Fact[];
  connector_matrix: Fact[];
  peripheral_table: Fact[];
  source_refs: SourceRef[];
  conflicts: Fact[];
}

/** The committed fact-pack index (data/facts/board-fact-packs.json). */
interface FactPackIndex {
  schema_version: number;
  packs: FactPack[];
}

/** A board's official source URL entry (Rust `SourceUrl`). */
interface SourceUrl {
  kind: string;
  url: string;
  status?: string;
}

/** A board peripheral row used for completeness readiness. */
interface PeripheralRecord {
  category: string;
  name: string;
  chip: string;
  bus: string;
  driver: string;
  source_url: string;
  source_status?: string;
  evidence_level?: string;
}

/** An official demo/example reference. */
interface DemoRef {
  framework: string;
  target: string;
  source_url: string;
  path: string;
  stale?: boolean;
  source_status?: string;
  evidence_level?: string;
}

/** A board registry record (data/boards.json `boards[]`). */
interface BoardRecord {
  id: string;
  family_id?: string | null;
  product?: boolean;
  display_name?: string;
  aliases: string[];
  mcu: string;
  supported: boolean;
  frameworks: string[];
  peripherals?: string[];
  repo_url?: string;
  wiki_url?: string;
  source_status?: string;
  source_urls: SourceUrl[];
  source_hashes?: Record<string, string>;
  stale?: boolean;
  peripheral_matrix: PeripheralRecord[];
  demo_refs: DemoRef[];
  warnings?: string[];
}

/** data/boards.json top-level. */
interface BoardIndex {
  schema_version: number;
  boards: BoardRecord[];
}

/** Topic readiness signal surfaced inside a source-query report. */
interface CompletenessSignal {
  board_id: string;
  topic: string;
  completeness: string;
  evidence_level: string;
  source_query_command: string;
  update_command?: string;
  /** Omitted (like Rust's skip-empty) when nothing is missing. */
  required_missing?: string[];
}

/** A discovery pointer telling the model which command to run next. */
interface DiscoveryHint {
  when: string;
  action: string;
  command?: string;
  reference_id?: string;
  reason: string;
}

/** The `source query --json` report (Rust `FactQueryReport`). */
interface FactQueryReport {
  status: string;
  board_id: string;
  topic: string;
  supported: boolean;
  fact_pack: FactPack;
  facts: Fact[];
  unknowns: Fact[];
  conflicts: Fact[];
  source_refs: SourceRef[];
  completeness?: CompletenessSignal;
  discovery_hints: DiscoveryHint[];
  warnings: string[];
  on_demand?: {
    cache_status: string;
    repo_url: string;
    source_path: string;
    gates: Record<string, string | number>;
  };
}

/** Pinless response returned when dynamic verification cannot prove a source. */
interface HonestDegradeReport {
  status: "NO_VERIFIABLE_PINOUT";
  board_id: string;
  resolved_board_id: string;
  topic: string;
  supported: false;
  repo_url: string | null;
  reason: string;
  message: string;
  facts: Fact[];
  pin_matrix: Fact[];
  source_refs: SourceRef[];
  conflicts: Fact[];
  warnings: string[];
}

/** One re-fetch verdict row (Rust `SourceVerifyFact`). */
interface VerifyFact {
  key: string;
  topic: string;
  fetch_url: string;
  line_range?: string;
  stored_hash: string;
  live_hash?: string;
  verdict: string;
  detail?: string;
}

/** `verify sources --json` report (Rust `SourceVerifyReport`). */
interface VerifyReport {
  status: string;
  board_id: string;
  topic?: string;
  counts: { total: number; ok: number; drift: number; unreachable: number };
  facts: VerifyFact[];
}

/** Data-driven board sniff rule set (data/sniff-rules.json). */
interface SniffRules {
  schema_version: number;
  min_alias_len: number;
  max_file_bytes: number;
  max_source_files: number;
  /** One matcher entry per registry board. */
  boards: SniffBoard[];
}

/** A single board's normalized alias set for project/keyword matching. */
interface SniffBoard {
  board_id: string;
  aliases: string[];
}

/** The `context --json` thin pointer capsule. */
interface ContextReport {
  board: string | null;
  board_source: string | null;
  context: string;
  decision: string;
  skills: string[];
  verification_level: string;
}
