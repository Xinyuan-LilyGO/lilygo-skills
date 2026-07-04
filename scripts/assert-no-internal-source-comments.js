#!/usr/bin/env node
const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
const scanRoots = [
  path.join(repoRoot, "crates", "lilygo-skills-cli", "src"),
  path.join(repoRoot, "install.js"),
  path.join(repoRoot, "scripts"),
];
const blocked = /\b[FM][0-9]+(?:\.[0-9]+)?\b|req-change|task-spec|milestone|\baudit\b/i;

function walk(target) {
  if (!fs.existsSync(target)) return [];
  const stat = fs.statSync(target);
  if (stat.isFile()) return [target];
  return fs.readdirSync(target, { withFileTypes: true }).flatMap((entry) => {
    const child = path.join(target, entry.name);
    if (entry.isDirectory()) return walk(child);
    if (entry.isFile()) return [child];
    return [];
  });
}

function commentText(line) {
  const trimmed = line.trimStart();
  if (trimmed.startsWith("//")) return trimmed;
  if (trimmed.startsWith("/*")) return trimmed;
  if (trimmed.startsWith("*")) return trimmed;
  if (trimmed.startsWith("#") && !trimmed.startsWith("#!")) return trimmed;
  const inline = line.indexOf("//");
  if (inline >= 0) return line.slice(inline);
  return "";
}

const findings = [];
for (const file of scanRoots.flatMap(walk)) {
  const rel = path.relative(repoRoot, file);
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/);
  lines.forEach((line, index) => {
    const comment = commentText(line);
    if (comment && blocked.test(comment)) {
      findings.push({ file: rel, line: index + 1, text: comment.trim() });
    }
  });
}

const report = {
  status: findings.length === 0 ? "PASS" : "FAIL",
  scanned_roots: scanRoots.map((root) => path.relative(repoRoot, root)),
  finding_count: findings.length,
  findings,
};

process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
process.exit(findings.length === 0 ? 0 : 1);
