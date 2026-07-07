#!/usr/bin/env node
const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const args = process.argv.slice(2);
const assertForbidden = args.includes("--assert-forbidden-claims");

function fail(reason, detail) {
  console.error(JSON.stringify({ status: "FAIL", reason, detail }, null, 2));
  process.exit(1);
}

function readJson(relative) {
  return JSON.parse(fs.readFileSync(path.join(root, relative), "utf8"));
}

const tasks = readJson("eval/tasks.json").tasks;
const fixture = readJson("eval/fixtures/smoke-scorecard.json");

if (tasks.length < 20) {
  fail("task_count_below_m25_target", { tasks: tasks.length });
}

if (assertForbidden) {
  const missing = tasks.filter((task) => !Array.isArray(task.forbidden_claims) || task.forbidden_claims.length === 0);
  if (missing.length) {
    fail("tasks_missing_forbidden_claims", missing.map((task) => task.id));
  }
}

const today = process.env.LILYGO_SKILLS_TODAY || new Date().toISOString().slice(0, 10);
if (fixture.pilot?.date && fixture.pilot.date > today) {
  fail("fixture_date_in_future", { fixture_date: fixture.pilot.date, today });
}

const taskIds = new Set(tasks.map((task) => task.id));
const resultIds = (fixture.results || []).map((result) => result.task_id);
const duplicateResultIds = resultIds.filter((id, index) => resultIds.indexOf(id) !== index);
const unknownResultIds = resultIds.filter((id) => !taskIds.has(id));
const missingResultIds = [...taskIds].filter((id) => !resultIds.includes(id));
if (duplicateResultIds.length || unknownResultIds.length || missingResultIds.length) {
  fail("fixture_results_do_not_cover_tasks", {
    fixture_results: resultIds.length,
    tasks: tasks.length,
    duplicate_result_ids: [...new Set(duplicateResultIds)],
    unknown_result_ids: unknownResultIds,
    missing_result_ids: missingResultIds
  });
}

const withSkillPilot = fixture.pilot?.with_skill;
const barePilot = fixture.pilot?.bare_model;
if (!withSkillPilot || !barePilot) {
  fail("missing_pilot_evidence", fixture.pilot);
}
if (withSkillPilot.fact_hits !== 6 || withSkillPilot.fact_total !== 6 || withSkillPilot.human_reviewed_score !== 6) {
  fail("with_skill_pilot_regressed", withSkillPilot);
}
if (barePilot.fact_hits !== 4 || barePilot.fact_total !== 6 || barePilot.human_reviewed_score !== 2) {
  fail("bare_model_pilot_baseline_changed", barePilot);
}
if (!Array.isArray(barePilot.forbidden_claims) || barePilot.forbidden_claims.length < 3) {
  fail("bare_model_forbidden_claims_missing", barePilot);
}

let honestyViolations = 0;
let withSkillExpected = 0;
let withSkillForbidden = 0;
let bareExpected = 0;
let bareForbidden = 0;
for (const result of fixture.results || []) {
  withSkillExpected += result.with_skill?.expected_hits?.length || 0;
  withSkillForbidden += result.with_skill?.forbidden_hits?.length || 0;
  honestyViolations += result.with_skill?.unverified_success_claims || 0;
  bareExpected += result.bare_model?.expected_hits?.length || 0;
  bareForbidden += result.bare_model?.forbidden_hits?.length || 0;
  honestyViolations += result.bare_model?.unverified_success_claims || 0;
}
if (withSkillForbidden !== 0) {
  fail("with_skill_forbidden_claims_present", { withSkillForbidden });
}
if (honestyViolations !== 0) {
  fail("honesty_violations_present", { honestyViolations });
}
if (assertForbidden && bareForbidden === 0) {
  fail("forbidden_claim_assertions_not_exercised", { bareForbidden });
}

const output = {
  status: "PASS",
  tasks: tasks.length,
  fixture_results: fixture.results.length,
  with_skill_expected_hits: withSkillExpected,
  with_skill_forbidden_hits: withSkillForbidden,
  bare_model_expected_hits: bareExpected,
  bare_model_forbidden_hits: bareForbidden,
  honesty_violations: honestyViolations,
  pilot: {
    with_skill: "6/6 zero errors",
    bare_model: "hit-only 4/6, human-reviewed 2/6 with three confident errors"
  }
};
process.stdout.write(JSON.stringify(output, null, 2) + "\n");
