#!/usr/bin/env bash
set -euo pipefail

# Full install -> hook-injection chain for the JS dispatcher install.
# Exercises install.js against a sandbox HOME: idempotent settings/AGENTS
# wiring, mirror cleanup of stale runtime files, co-located user-hook survival,
# stale Rust-era hook replacement, unknown-flag refusal, Windows dry-run plan,
# and the installed hook envelopes (inject / no-op / bad-input) run through
# `node <root>/bin/hook.mjs` from outside any source tree.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-install-injection.XXXXXX")"
trap 'rm -rf "$SANDBOX"' EXIT

fail() { echo "FAIL install-injection: $1" >&2; exit 1; }

# --- first install (claude + codex) ----------------------------------------
HOME="$SANDBOX" node "$ROOT/install.js" --all --home "$SANDBOX" \
  >"$SANDBOX/install-1.json" 2>&1 || fail "first install exited non-zero"
node -e 'if(require(process.argv[1]).status!=="PASS"){process.exit(1)}' \
  "$SANDBOX/install-1.json" || fail "first install not PASS"

CLAUDE_ROOT="$SANDBOX/.claude/lilygo-skills"
[ -x "$CLAUDE_ROOT/bin/lilygo-skills" ] || fail "shim not installed/executable"
[ -f "$CLAUDE_ROOT/bin/lilygo-skills.mjs" ] || fail "dispatcher not installed"
[ -f "$CLAUDE_ROOT/data/boards.json" ] || fail "data model not installed"

# --- mirror cleanup: stale runtime files must not survive a re-install ------
mkdir -p "$CLAUDE_ROOT/data/stale-dir" "$CLAUDE_ROOT/bin/stale-dir"
printf 'stale\n' >"$CLAUDE_ROOT/data/stale-dir/old.json"
printf 'stale\n' >"$CLAUDE_ROOT/bin/stale-dir/old.mjs"
HOME="$SANDBOX" node "$ROOT/install.js" --all --home "$SANDBOX" \
  >"$SANDBOX/install-2.json" 2>&1 || fail "re-install exited non-zero"
node -e 'if(require(process.argv[1]).status!=="PASS"){process.exit(1)}' \
  "$SANDBOX/install-2.json" || fail "re-install not PASS"
[ -e "$CLAUDE_ROOT/data/stale-dir/old.json" ] && fail "data mirror kept stale file"
[ -e "$CLAUDE_ROOT/bin/stale-dir/old.mjs" ] && fail "bin mirror kept stale file"

# --- unknown flag refusal (no files written) --------------------------------
UNKNOWN_HOME="$SANDBOX/unknown-flag-home"
if HOME="$UNKNOWN_HOME" node "$ROOT/install.js" --codex --not-a-real-flag \
  --home "$UNKNOWN_HOME" >"$SANDBOX/install-unknown.json" 2>&1; then
  fail "unknown flag must fail"
fi
node -e 'const j=require(process.argv[1]); if(j.status!=="FAIL"||!(j.errors?.[0]||"").includes("unknown option")) process.exit(1)' \
  "$SANDBOX/install-unknown.json" || fail "unknown flag report shape"
[ -e "$UNKNOWN_HOME/.codex/lilygo-skills" ] && fail "unknown flag wrote runtime files"

# --- windows dry-run plan carries the .cmd shim -----------------------------
WINDOWS_HOME="$SANDBOX/windows-home"
LILYGO_INSTALL_PLATFORM=win32 node "$ROOT/install.js" --claude --dry-run \
  --home "$WINDOWS_HOME" >"$SANDBOX/install-windows.json" 2>&1 \
  || fail "windows dry-run exited non-zero"
node -e '
const j=require(process.argv[1]);
if(j.status!=="PASS"||j.mode!=="dry-run") process.exit(1);
if(!(j.planned_writes||[]).some(w=>w.includes("lilygo-skills.cmd"))) process.exit(2);
' "$SANDBOX/install-windows.json" || fail "windows dry-run plan missing .cmd shim"

# --- settings.json merge: co-located user hook survives, stale ours replaced -
node - "$SANDBOX" <<'PRESEED'
const fs=require("fs"), path=require("path");
const p=path.join(process.argv[2],".claude","settings.json");
const settings=JSON.parse(fs.readFileSync(p,"utf8"));
settings.hooks.UserPromptSubmit=[
  { hooks:[
    { type:"command", command:"/usr/local/bin/user-precious-hook" },
    { type:"command", command:"/old/path/lilygo-skills hook claude" },
  ]},
];
fs.writeFileSync(p, JSON.stringify(settings,null,2)+"\n");
PRESEED
HOME="$SANDBOX" node "$ROOT/install.js" --claude --home "$SANDBOX" \
  >"$SANDBOX/install-merge.json" 2>&1 || fail "merge re-install exited non-zero"
node - "$SANDBOX" <<'SHAREDCHECK'
const fs=require("fs"), path=require("path");
const s=JSON.parse(fs.readFileSync(path.join(process.argv[2],".claude","settings.json"),"utf8"));
const cmds=s.hooks.UserPromptSubmit.flatMap(e=>e.hooks.map(h=>h.command));
const ok = cmds.includes("/usr/local/bin/user-precious-hook")
  && !cmds.includes("/old/path/lilygo-skills hook claude")
  && cmds.filter(c=>c.includes("lilygo-skills")&&(c.includes("hook.mjs")||c.includes("hook claude"))).length===1;
if(!ok){ console.error("settings merge wrong:", JSON.stringify(cmds)); process.exit(1); }
SHAREDCHECK
[ $? -eq 0 ] || fail "settings.json merge preserve/replace"

# --- AGENTS.md single section + orphan-marker refusal -----------------------
[ "$(grep -c 'lilygo-skills:start' "$SANDBOX/.codex/AGENTS.md")" = "1" ] \
  || fail "AGENTS.md not single-section"
ORPHAN_HOME="$SANDBOX/orphan-home"
mkdir -p "$ORPHAN_HOME/.codex"
printf '<!-- lilygo-skills:start -->\nMY PRECIOUS USER NOTES\n' >"$ORPHAN_HOME/.codex/AGENTS.md"
if HOME="$ORPHAN_HOME" node "$ROOT/install.js" --codex --home "$ORPHAN_HOME" \
  >"$SANDBOX/install-orphan.json" 2>&1; then
  fail "orphan AGENTS marker must fail install"
fi
grep -q "MY PRECIOUS USER NOTES" "$ORPHAN_HOME/.codex/AGENTS.md" \
  || fail "orphan-marker user content was modified"

# --- installed hook envelopes: inject / no-op / bad-input -------------------
printf '{"prompt":"T-Display-S3 LVGL touch not working"}' \
  | node "$CLAUDE_ROOT/bin/hook.mjs" claude >"$SANDBOX/hook-inject.json"
printf '{"prompt":"how do I prune tomato plants"}' \
  | node "$CLAUDE_ROOT/bin/hook.mjs" claude >"$SANDBOX/hook-noop.json"
printf 'not json at all' \
  | node "$CLAUDE_ROOT/bin/hook.mjs" claude >"$SANDBOX/hook-bad.json"
node - "$SANDBOX" <<'HOOKCHECK'
const fs=require("fs"), path=require("path");
const read=(f)=>JSON.parse(fs.readFileSync(path.join(process.argv[2],f),"utf8"));
const inj=read("hook-inject.json");
if(inj.hookSpecificOutput?.hookEventName!=="UserPromptSubmit") process.exit(1);
if(!String(inj.hookSpecificOutput?.additionalContext||"").includes("board-t-display-s3")) process.exit(2);
if("decision" in inj) process.exit(3); // claude envelope must not leak a top-level decision
const noop=read("hook-noop.json");
if(noop.hookSpecificOutput?.hookEventName!=="UserPromptSubmit") process.exit(4);
if("additionalContext" in (noop.hookSpecificOutput||{})) process.exit(5);
const bad=read("hook-bad.json"); // fail-open: valid envelope, never a crash
if(bad.hookSpecificOutput?.hookEventName!=="UserPromptSubmit") process.exit(6);
HOOKCHECK
[ $? -eq 0 ] || fail "installed hook envelopes wrong"

echo '{"status":"PASS","smoke":"install-injection","host":"js-dispatcher"}'
