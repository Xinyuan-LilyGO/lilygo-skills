#!/usr/bin/env bash
set -euo pipefail

# Proves the injection chain end-to-end in a sandbox HOME: install wires
# ~/.claude/skills + settings.json hook + ~/.codex/AGENTS.md idempotently, and
# the installed binary emits the Claude UserPromptSubmit envelope when invoked
# from outside any source tree (the production condition).

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/lilygo-install-injection.XXXXXX")"
trap 'rm -rf "$SANDBOX"' EXIT

cargo build -q -p lilygo-skills-cli
BIN_SRC="$ROOT/target/debug/lilygo-skills"

run_install() {
  HOME="$SANDBOX" node "$ROOT/install.js" --all --bin "$BIN_SRC" \
    >"$SANDBOX/install-$1.json" 2>&1
}

run_install first
LEGACY_SOURCE_INTAKE=".claude/lilygo-skills/$(printf '%s/%s/%s' doc references source-intake)"
mkdir -p "$SANDBOX/.codex/lilygo-skills/data/stale-dir"
mkdir -p "$SANDBOX/.claude/lilygo-skills/data/references/source-intake/stale-dir"
mkdir -p "$SANDBOX/$LEGACY_SOURCE_INTAKE/legacy-stale-dir"
printf 'stale\n' >"$SANDBOX/.codex/lilygo-skills/data/stale-dir/old.json"
printf 'stale\n' >"$SANDBOX/.claude/lilygo-skills/data/references/source-intake/stale-dir/old.md"
printf 'stale\n' >"$SANDBOX/$LEGACY_SOURCE_INTAKE/legacy-stale-dir/old.md"
run_install second

UNKNOWN_HOME="$SANDBOX/unknown-flag-home"
mkdir -p "$UNKNOWN_HOME"
if HOME="$UNKNOWN_HOME" node "$ROOT/install.js" --codex --not-a-real-flag --bin "$BIN_SRC" \
  >"$SANDBOX/install-unknown-flag.json" 2>&1; then
  echo "FAIL unknown install flag must fail" >&2
  exit 1
fi
test ! -e "$UNKNOWN_HOME/.codex/lilygo-skills" || {
  echo "FAIL unknown install flag wrote runtime files" >&2
  exit 1
}

WINDOWS_HOME="$SANDBOX/windows-home"
mkdir -p "$WINDOWS_HOME"
LILYGO_INSTALL_PLATFORM=win32 HOME="$WINDOWS_HOME" \
  node "$ROOT/install.js" --codex --dry-run --bin "$SANDBOX/lilygo-skills.exe" \
  --home "$WINDOWS_HOME" >"$SANDBOX/install-windows-dry-run.json"

# P2 regression: co-located user hook must survive; stale lilygo command is
# replaced, not duplicated.
node - "$SANDBOX" <<'PRESEED'
const fs = require("fs");
const path = require("path");
const sandbox = process.argv[2];
const settingsPath = path.join(sandbox, ".claude", "settings.json");
const settings = JSON.parse(fs.readFileSync(settingsPath, "utf8"));
settings.hooks.UserPromptSubmit = [
  {
    hooks: [
      { type: "command", command: "/usr/local/bin/user-precious-hook" },
      { type: "command", command: "/old/path/lilygo-skills hook claude" },
    ],
  },
];
fs.writeFileSync(settingsPath, JSON.stringify(settings, null, 2) + "\n");
PRESEED
run_install shared
node - "$SANDBOX" <<'SHAREDCHECK'
const fs = require("fs");
const path = require("path");
const sandbox = process.argv[2];
const settings = JSON.parse(
  fs.readFileSync(path.join(sandbox, ".claude", "settings.json"), "utf8")
);
const entries = settings.hooks.UserPromptSubmit;
const commands = entries.flatMap((e) => e.hooks.map((h) => h.command));
const survived =
  commands.includes("/usr/local/bin/user-precious-hook") &&
  !commands.includes("/old/path/lilygo-skills hook claude") &&
  commands.filter((c) => c.includes("lilygo-skills") && c.includes("hook claude")).length === 1;
fs.writeFileSync(
  path.join(sandbox, ".claude", "settings-shared-check.json"),
  JSON.stringify({ survived, commands }) + "\n"
);
SHAREDCHECK

# P2 regression: unbalanced AGENTS.md markers must refuse loudly instead of
# eating user content on the next run.
ORPHAN_HOME="$SANDBOX/orphan-home"
mkdir -p "$ORPHAN_HOME/.codex"
printf '%s\nMY PRECIOUS USER NOTES\n' "<!-- lilygo-skills:start -->" \
  >"$ORPHAN_HOME/.codex/AGENTS.md"
if HOME="$ORPHAN_HOME" node "$ROOT/install.js" --codex --bin "$BIN_SRC" \
  >"$SANDBOX/install-orphan.json" 2>&1; then
  echo "FAIL orphan-marker install must fail" >&2
  exit 1
fi
grep -q "MY PRECIOUS USER NOTES" "$ORPHAN_HOME/.codex/AGENTS.md" || {
  echo "FAIL orphan-marker user content was modified" >&2
  exit 1
}

MOUNT_SRC="$SANDBOX/mount-src"
MOUNT_HOME="$SANDBOX/mount-home"
mkdir -p "$MOUNT_SRC" "$MOUNT_HOME"
tar --exclude .git --exclude target --exclude .tmp --exclude .artifact-lens \
  -cf - . | tar -C "$MOUNT_SRC" -xf -
HOME="$MOUNT_HOME" node "$MOUNT_SRC/install.js" --codex \
  >"$SANDBOX/install-mount-only.json" 2>&1
MOUNT_BIN="$MOUNT_HOME/.codex/lilygo-skills/bin/lilygo-skills"
"$MOUNT_BIN" setup plan --framework platformio --json \
  >"$SANDBOX/mount-setup-platformio.json"
printf '{"prompt":"I am using LilyGO T-Display-S3 with PlatformIO LVGL"}\n' \
  | "$MOUNT_BIN" hook codex >"$SANDBOX/mount-hook-inject.json"
printf '{"prompt":"tomato pruning"}\n' \
  | "$MOUNT_BIN" hook codex >"$SANDBOX/mount-hook-noop.json"

INSTALLED_BIN="$SANDBOX/.claude/lilygo-skills/bin/lilygo-skills"

cd "$SANDBOX"
printf '{"prompt":"T-Display-S3 LVGL touch not working"}\n' \
  | HOME="$SANDBOX" "$INSTALLED_BIN" hook claude >"$SANDBOX/hook-inject.json"
printf '{"prompt":"how do I prune tomato plants"}\n' \
  | HOME="$SANDBOX" "$INSTALLED_BIN" hook claude >"$SANDBOX/hook-noop.json"
printf 'not json at all\n' \
  | HOME="$SANDBOX" "$INSTALLED_BIN" hook claude >"$SANDBOX/hook-badinput.json"
cd "$ROOT"

SANDBOX="$SANDBOX" node <<'NODE'
const fs = require("fs");
const path = require("path");
const sandbox = process.env.SANDBOX;
const read = (p) => fs.readFileSync(path.join(sandbox, p), "utf8");
const readJson = (p) => JSON.parse(read(p));
const fail = (name, detail) => {
  console.error(`FAIL ${name}`);
  if (detail !== undefined) console.error(detail);
  process.exit(1);
};

for (const report of ["install-first.json", "install-second.json"]) {
  const parsed = readJson(report);
  if (parsed.status !== "PASS") fail(`${report} status`, read(report));
}
if (fs.existsSync(path.join(sandbox, ".codex/lilygo-skills/data/stale-dir/old.json"))) {
  fail("codex data mirror stale file survived");
}
if (
  fs.existsSync(
    path.join(
      sandbox,
      ".claude/lilygo-skills/data/references/source-intake/stale-dir/old.md"
    )
  )
) {
  fail("claude source-intake mirror stale file survived");
}
if (
  fs.existsSync(
    path.join(
      sandbox,
      ".claude/lilygo-skills",
      "doc",
      "references",
      "source-intake",
      "legacy-stale-dir",
      "old.md"
    )
  )
) {
  fail("claude legacy source-intake mirror survived");
}
const unknown = readJson("install-unknown-flag.json");
if (unknown.status !== "FAIL" || !unknown.errors?.[0]?.includes("unknown option")) {
  fail("unknown flag report", read("install-unknown-flag.json"));
}
const windows = readJson("install-windows-dry-run.json");
if (
  windows.status !== "PASS" ||
  !windows.planned_writes?.some((target) =>
    target.includes(".codex/lilygo-skills/bin/lilygo-skills.exe")
  )
) {
  fail("windows binary plan", read("install-windows-dry-run.json"));
}
const mountInstall = readJson("install-mount-only.json");
if (mountInstall.status !== "PASS" || mountInstall.runtime_mode !== "mount-only") {
  fail("mount-only install", read("install-mount-only.json"));
}
const mountSetup = readJson("mount-setup-platformio.json");
if (
  mountSetup.status !== "planned" ||
  mountSetup.runtime_mode !== "mount-only" ||
  !mountSetup.toolchains?.some((tool) => tool.id === "platformio-core") ||
  mountSetup.writes?.length !== 0
) {
  fail("mount-only setup plan", read("mount-setup-platformio.json"));
}
const mountHook = readJson("mount-hook-inject.json");
if (
  mountHook.decision !== "needs_runtime_setup" ||
  !mountHook.missing?.includes("runtime-binary") ||
  !mountHook.context?.includes("setup-only mode")
) {
  fail("mount-only hook inject", read("mount-hook-inject.json"));
}
const mountNoop = readJson("mount-hook-noop.json");
if (mountNoop.decision !== "no-op" || mountNoop.context !== "") {
  fail("mount-only hook no-op", read("mount-hook-noop.json"));
}

const skill = read(".claude/skills/lilygo-skills/SKILL.md");
if (!skill.startsWith("---")) fail("installed skill lacks frontmatter");
if (!/^name: lilygo-skills$/m.test(skill)) fail("frontmatter lacks name");
if (!/^description: /m.test(skill)) fail("frontmatter lacks description");

const settings = readJson(".claude/settings.json");
const entries = settings?.hooks?.UserPromptSubmit ?? [];
const ours = entries.filter(
  (entry) =>
    Array.isArray(entry?.hooks) &&
    entry.hooks.some(
      (h) =>
        typeof h?.command === "string" &&
        h.command.includes("lilygo-skills") &&
        h.command.includes("hook claude")
    )
);
if (ours.length !== 1) fail("settings hook entries", JSON.stringify(entries));

const agents = read(".codex/AGENTS.md");
const marks = agents.split("<!-- lilygo-skills:start -->").length - 1;
if (marks !== 1) fail("AGENTS.md marked sections", `count=${marks}`);

const inject = readJson("hook-inject.json");
const injectOut = inject.hookSpecificOutput;
if (injectOut?.hookEventName !== "UserPromptSubmit")
  fail("inject hookEventName", read("hook-inject.json"));
if (
  typeof injectOut.additionalContext !== "string" ||
  !injectOut.additionalContext.includes("board-t-display-s3")
)
  fail("inject additionalContext", read("hook-inject.json"));
if ("decision" in inject) fail("inject leaks top-level decision key");

const noop = readJson("hook-noop.json");
if (noop.hookSpecificOutput?.hookEventName !== "UserPromptSubmit")
  fail("noop envelope", read("hook-noop.json"));
if ("additionalContext" in (noop.hookSpecificOutput ?? {}))
  fail("noop must not inject context", read("hook-noop.json"));

const bad = readJson("hook-badinput.json");
if (bad.hookSpecificOutput?.hookEventName !== "UserPromptSubmit")
  fail("bad-input fail-open envelope", read("hook-badinput.json"));

// A user hook sharing an entry with an older lilygo command must survive the
// merge; only our command is replaced.
const shared = readJson(".claude/settings-shared-check.json");
if (shared.survived !== true) fail("co-located user hook survived", JSON.stringify(shared));

console.log(
  JSON.stringify({
    status: "PASS",
    checks: [
      "install twice PASS",
      "mirror removed stale runtime files",
      "unknown flag refused before writes",
      "windows exe binary plan",
      "mount-only install",
      "mount-only setup plan",
      "mount-only hook routing",
      "claude skill frontmatter",
      "settings hook single entry",
      "agents md single section",
      "inject envelope",
      "noop envelope",
      "fail-open envelope",
    ],
  })
);
NODE
