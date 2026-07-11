#!/usr/bin/env node
// Committed provenance gate — makes "verifiable" continuously, offline provable.
//
// Loads data/facts/board-fact-packs.json and, for every pin/bus fact that
// claims evidence_level "V3-source-reference", asserts it carries the
// provenance needed to re-prove it live:
//   - source.path_or_url : a resolvable github URL (github.com / raw.github…),
//   - source.hash        : a sha256:<64-hex> digest.
// These two are exactly what `lilygo-skills verify sources` re-fetches and
// re-hashes (the stored hash is of the whole file, so the file URL + hash is
// the re-verifiable unit). A V3 fact missing either is provenance-less — it
// cannot be re-proven and must not carry the V3 label — so the gate exits
// non-zero.
//
// It additionally reports the *line-anchor* split honestly: a fully
// line-anchored V3 fact also carries source.line_range (the strongest,
// #define-verified tier). Facts without a line_range are repo/reference-tier;
// they are counted and printed, never silently rounded up to line-anchored and
// never relabeled here (data stays untouched — see honesty-evidence.md ⑤).
//
// Deterministic, offline, repeatable: reads only the committed JSON.
//
// Usage:
//   node eval/verify-provenance.js            # gate: non-zero if any V3 fact lacks url+hash
//   node eval/verify-provenance.js --json     # machine-readable summary

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const FACT_PACKS = path.join(ROOT, "data/facts/board-fact-packs.json");
const args = process.argv.slice(2);
const jsonOut = args.includes("--json");

const V3 = "V3-source-reference";
const SHA256_RE = /^sha256:[0-9a-f]{64}$/;

function isResolvableGithubUrl(url) {
  return (
    typeof url === "string" &&
    (url.startsWith("https://github.com/") ||
      url.startsWith("https://raw.githubusercontent.com/"))
  );
}

const index = JSON.parse(fs.readFileSync(FACT_PACKS, "utf8"));

let total = 0;
let lineAnchored = 0;
const incomplete = []; // V3 facts missing url or hash — hard failures.
const referenceTier = []; // V3 facts with url+hash but no line_range — reported.

for (const pack of index.packs || []) {
  for (const matrix of ["pin_matrix", "bus_matrix"]) {
    for (const fact of pack[matrix] || []) {
      if (fact.evidence_level !== V3) continue;
      total += 1;
      const s = fact.source || {};
      const hasUrl = isResolvableGithubUrl(s.path_or_url);
      const hasHash = SHA256_RE.test(s.hash || "");
      const hasLine = typeof s.line_range === "string" && s.line_range.length > 0;
      if (!hasUrl || !hasHash) {
        incomplete.push({
          board_id: pack.board_id,
          matrix,
          key: fact.key,
          missing: [!hasUrl && "path_or_url", !hasHash && "hash"].filter(Boolean),
          path_or_url: s.path_or_url || null,
          hash: s.hash || null,
        });
        continue;
      }
      if (hasLine) {
        lineAnchored += 1;
      } else {
        referenceTier.push({
          board_id: pack.board_id,
          matrix,
          key: fact.key,
          path_or_url: s.path_or_url,
        });
      }
    }
  }
}

const allHaveProvenance = incomplete.length === 0;
const summary = {
  status: allHaveProvenance ? "PASS" : "FAIL",
  fact_packs: path.relative(ROOT, FACT_PACKS),
  v3_pin_bus_facts: total,
  all_have_provenance: allHaveProvenance, // url + sha256 on every V3 fact
  line_anchored: lineAnchored, // + line_range (strongest, live-re-provable)
  reference_tier: referenceTier.length, // url+hash only, no line_range
  incomplete_count: incomplete.length,
  incomplete, // provenance-less V3 facts — must be empty to pass
};

if (jsonOut) {
  console.log(JSON.stringify(summary, null, 2));
} else {
  console.log(
    `provenance gate: ${total} V3 pin/bus facts; all-have-provenance (url+sha256)=${
      allHaveProvenance ? "yes" : "no"
    }; line-anchored=${lineAnchored}/${total}; reference-tier=${referenceTier.length}`
  );
  if (!allHaveProvenance) {
    console.error(`FAIL: ${incomplete.length} V3 fact(s) lack a resolvable url+sha256:`);
    for (const f of incomplete) {
      console.error(`  ${f.board_id} ${f.matrix} ${f.key} — missing ${f.missing.join("+")}`);
    }
  }
}

process.exit(allHaveProvenance ? 0 : 1);
