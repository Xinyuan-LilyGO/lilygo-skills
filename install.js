#!/usr/bin/env node
// Installer for the LilyGO JS context kernel. It materializes a self-contained
// runtime under ~/.claude/lilygo-skills (or ~/.codex/lilygo-skills): the Node
// dispatcher (bin/*.mjs), the data model (data/**), and a `lilygo-skills` shim
// that execs `node <root>/bin/lilygo-skills.mjs`. The data travels WITH the
// dispatcher (bin/lib.mjs anchors data/** at bin/'s parent), so a runtime update
// can never leave stale data behind. No compiler or prebuilt binary is involved:
// the host already has Node (Claude Code / Codex both run on it).
const fs = require("fs");
const os = require("os");
const path = require("path");
const cp = require("child_process");

function parseArgs(argv) {
  const valueFlags = new Set(["--home"]);
  // --build is accepted but does nothing: the JS dispatcher needs no build step.
  // It is kept so a previously documented `install.js --all --dry-run --build`
  // invocation still runs (with a one-line notice), rather than erroring.
  const booleanFlags = new Set([
    "--codex",
    "--claude",
    "--all",
    "--dry-run",
    "--build",
    "--no-self-test",
  ]);
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (!arg.startsWith("--")) {
      throw new Error(`unexpected positional argument: ${arg}`);
    }
    if (valueFlags.has(arg)) {
      const value = argv[i + 1];
      if (value === undefined || value.startsWith("--")) {
        throw new Error(`${arg} requires a value`);
      }
      i += 1;
      continue;
    }
    if (!booleanFlags.has(arg)) {
      throw new Error(`unknown option: ${arg}`);
    }
  }
  const args = new Set(argv);
  const hosts = [];
  if (args.has("--codex") || args.has("--all")) hosts.push("codex");
  if (args.has("--claude") || args.has("--all")) hosts.push("claude");
  if (hosts.length === 0) hosts.push("codex");
  const optionValue = (flag) => {
    const index = argv.indexOf(flag);
    if (index < 0) return null;
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("--")) {
      throw new Error(`${flag} requires a value`);
    }
    return value;
  };
  return {
    dryRun: args.has("--dry-run"),
    build: args.has("--build"),
    hosts,
    home: optionValue("--home") ?? os.homedir(),
    selfTest: !args.has("--no-self-test"),
  };
}

const AGENTS_SECTION_START = "<!-- lilygo-skills:start -->";
const AGENTS_SECTION_END = "<!-- lilygo-skills:end -->";

function installPlatform() {
  return process.env.LILYGO_INSTALL_PLATFORM || process.platform;
}

// Human-facing shim name. `.cmd` on Windows, bare on POSIX. Hooks and the
// install self-test call `node <dispatcher>` directly, so the shim is only a
// convenience for a person typing `lilygo-skills` on their PATH.
function shimName() {
  return installPlatform() === "win32" ? "lilygo-skills.cmd" : "lilygo-skills";
}

function dispatcherPath(root) {
  return path.join(root, "bin", "lilygo-skills.mjs");
}

// Conventional per-user PATH dir. The runtime shim lives under the runtime root
// (NOT on PATH), so a person typing `lilygo-skills` — or the model self-running
// `lilygo-skills source query ...` from any cwd — can only resolve it if we also
// place it on PATH. POSIX: ~/.local/bin; Windows: %USERPROFILE%\bin.
function pathBinDir(home) {
  if (installPlatform() === "win32") return path.join(home, "bin");
  return path.join(home, ".local", "bin");
}

function pathShimPath(home) {
  return path.join(pathBinDir(home), shimName());
}

// True if `dir` is already an entry on the current PATH (resolved, so symlinked
// or trailing-slash forms still match). Used only to decide whether to warn.
function pathIncludes(dir) {
  const resolvedDir = path.resolve(dir);
  return String(process.env.PATH || "")
    .split(path.delimiter)
    .filter(Boolean)
    .some((entry) => {
      try {
        return path.resolve(entry) === resolvedDir;
      } catch {
        return false;
      }
    });
}

// Marker guarding the auto-added PATH line so re-installs never duplicate it.
const PATH_RC_MARKER = "# lilygo-skills PATH (added by install.js)";

// When the shim's bin dir is not yet on PATH, append an idempotent export to the
// user's shell rc so `lilygo-skills` — and the model's `source query` pull —
// resolves after the next shell start. POSIX only; picks the rc for the active
// shell (zsh/bash), else ~/.profile. Returns { rc, added } or null (win32 / no-op).
function ensurePathInShellRc(home, binDir) {
  if (installPlatform() === "win32") return null;
  const shell = String(process.env.SHELL || "");
  const candidates = [];
  if (shell.includes("zsh")) candidates.push(path.join(home, ".zshrc"));
  else if (shell.includes("bash")) candidates.push(path.join(home, ".bashrc"));
  candidates.push(path.join(home, ".zshrc"), path.join(home, ".bashrc"));
  const rc = candidates.find((p) => fs.existsSync(p)) || path.join(home, ".profile");
  let current = "";
  try {
    current = fs.readFileSync(rc, "utf8");
  } catch {
    current = "";
  }
  if (current.includes(PATH_RC_MARKER)) return { rc, added: false };
  const exportValue =
    path.resolve(binDir) === path.resolve(path.join(home, ".local", "bin"))
      ? '"$HOME/.local/bin:$PATH"'
      : `"${binDir}:$PATH"`;
  const prefix = current && !current.endsWith("\n") ? "\n" : "";
  fs.appendFileSync(rc, `${prefix}${PATH_RC_MARKER}\nexport PATH=${exportValue}\n`);
  return { rc, added: true };
}

// Link/refresh the PATH shim so `lilygo-skills` resolves runtime-wide. Idempotent
// by construction: any existing entry (symlink to an old root, stale file) is
// removed and re-pointed at THIS install's runtime shim, so a re-install or a
// root move always leaves the PATH entry current. POSIX uses a symlink (follows
// dispatcher updates in place); Windows writes a thin forwarding .cmd because
// symlinks there need elevation. When the bin dir is not yet on PATH, the shell
// rc is updated so a brand-new user's pull path resolves. Returns where it
// landed, whether it's on PATH, and any rc it updated.
function installPathShim(home, root) {
  const binDir = pathBinDir(home);
  const target = pathShimPath(home);
  const runtimeShim = path.join(root, "bin", shimName());
  fs.mkdirSync(binDir, { recursive: true });
  fs.rmSync(target, { force: true });
  if (installPlatform() === "win32") {
    fs.writeFileSync(target, shimContents(root));
    fs.chmodSync(target, 0o755);
  } else {
    fs.symlinkSync(runtimeShim, target);
  }
  const onPath = pathIncludes(binDir);
  const rcUpdated = onPath ? null : ensurePathInShellRc(home, binDir);
  return { target, binDir, onPath, rcUpdated };
}

function claudeSkillPath(home) {
  return path.join(home, ".claude", "skills", "lilygo-skills", "SKILL.md");
}

function claudeSettingsPath(home) {
  return path.join(home, ".claude", "settings.json");
}

function codexAgentsPath(home) {
  return path.join(home, ".codex", "AGENTS.md");
}

// `node "<hook.mjs>" claude` runs cross-platform (Node is on PATH on every
// supported host). $HOME-relative for the default home so the entry survives a
// home move; a non-default --home cannot rely on $HOME at hook runtime, so it
// gets the absolute path instead.
function claudeHookScript(home) {
  if (home && path.resolve(home) !== path.resolve(os.homedir())) {
    return path.join(home, ".claude", "lilygo-skills", "bin", "hook.mjs");
  }
  return "$HOME/.claude/lilygo-skills/bin/hook.mjs";
}

// Hooks run in a NON-login shell, so a version-manager node (fnm/nvm/volta —
// only wired up in the login shell's rc) is not on that PATH. Probe a bare
// system PATH; when `node` does not resolve there, pin the hook to the absolute
// node running this installer so the hook cannot silently die on such hosts.
function hookNodeBinary() {
  if (installPlatform() === "win32") return "node";
  try {
    const probe = cp.execFileSync(
      "/bin/sh",
      ["-c", "command -v node"],
      { env: { PATH: "/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin" }, encoding: "utf8" }
    ).trim();
    if (probe) return "node";
  } catch {
    // fall through to the absolute path
  }
  return process.execPath;
}

function claudeHookCommand(home) {
  return `${hookNodeBinary()} "${claudeHookScript(home)}" claude`;
}

function manualClaudeWiring(home) {
  return (
    `add {"hooks":{"UserPromptSubmit":[{"hooks":[{"type":"command",` +
    `"command":"${claudeHookCommand(home).replaceAll('"', '\\"')}"}]}]}} ` +
    `to ~/.claude/settings.json`
  );
}

// The router skill source is the single origin for every installed meta
// SKILL.md; Claude Code discovers personal skills only under
// ~/.claude/skills/<name>/SKILL.md with YAML frontmatter.
function installClaudeSkill(repoRoot, home) {
  const source = path.join(repoRoot, "skills", "lilygo-router", "SKILL.md");
  const target = claudeSkillPath(home);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.copyFileSync(source, target);
}

// Match our own hook entry across both the current node form and any stale
// Rust-era `... lilygo-skills ... hook claude` command, so a re-install cleanly
// replaces the old entry instead of stacking a duplicate.
function isLilygoHookCommand(hook) {
  return (
    typeof hook?.command === "string" &&
    hook.command.includes("lilygo-skills") &&
    (hook.command.includes("hook.mjs") || hook.command.includes("hook claude"))
  );
}

function isLilygoHookEntry(entry) {
  return entry && Array.isArray(entry.hooks) && entry.hooks.some(isLilygoHookCommand);
}

function mergeClaudeSettings(home) {
  const settingsPath = claudeSettingsPath(home);
  let settings = {};
  if (fs.existsSync(settingsPath)) {
    const raw = fs.readFileSync(settingsPath, "utf8");
    try {
      settings = JSON.parse(raw);
    } catch (error) {
      throw new Error(
        `~/.claude/settings.json is not valid JSON (${error.message}); ` +
          `fix it or ${manualClaudeWiring(home)}`
      );
    }
    if (!settings || typeof settings !== "object" || Array.isArray(settings)) {
      throw new Error(
        `~/.claude/settings.json must contain a JSON object; ${manualClaudeWiring(home)}`
      );
    }
  }
  if (settings.hooks === undefined || settings.hooks === null) {
    settings.hooks = {};
  }
  if (typeof settings.hooks !== "object" || Array.isArray(settings.hooks)) {
    throw new Error(
      `~/.claude/settings.json "hooks" must be an object; ${manualClaudeWiring(home)}`
    );
  }
  const existingValue = settings.hooks.UserPromptSubmit;
  if (existingValue !== undefined && !Array.isArray(existingValue)) {
    throw new Error(
      `~/.claude/settings.json "hooks.UserPromptSubmit" must be an array; ` +
        `refusing to overwrite it; ${manualClaudeWiring(home)}`
    );
  }
  const existing = existingValue ?? [];
  // Remove only our own commands; a user hook sharing an entry with ours must
  // survive the merge.
  const kept = [];
  for (const entry of existing) {
    if (!isLilygoHookEntry(entry)) {
      kept.push(entry);
      continue;
    }
    const remaining = entry.hooks.filter((hook) => !isLilygoHookCommand(hook));
    if (remaining.length > 0) {
      kept.push({ ...entry, hooks: remaining });
    }
  }
  kept.push({ hooks: [{ type: "command", command: claudeHookCommand(home) }] });
  settings.hooks.UserPromptSubmit = kept;
  fs.mkdirSync(path.dirname(settingsPath), { recursive: true });
  fs.writeFileSync(settingsPath, JSON.stringify(settings, null, 2) + "\n");
}

function codexAgentsSection(root) {
  const dispatcher = dispatcherPath(root);
  return [
    AGENTS_SECTION_START,
    "## LilyGO Skills",
    "",
    `LilyGO board context runtime lives at \`${root}\`.`,
    "For any prompt about LilyGO boards (T-Display, T-Watch, T-Beam, T-Deck and",
    "other LilyGO products), firmware, flashing, LVGL, OTA, LoRa/GNSS, sensors,",
    "battery, or pinouts:",
    "",
    `1. Run \`node "${dispatcher}" context --json "<prompt>"\` to get the injected board capsule (facts, pins, source refs).`,
    `2. Run \`node "${dispatcher}" source query --board <board-id> --topic <topic> --json\` for exact pins/buses before claiming them.`,
    `3. Read the operating patterns in the meta skill \`${path.join(root, "public-skill", "SKILL.md")}\`.`,
    "",
    "Skip this section for prompts unrelated to LilyGO hardware.",
    AGENTS_SECTION_END,
  ].join("\n");
}

function appendCodexAgents(home, root) {
  const target = codexAgentsPath(home);
  const section = codexAgentsSection(root);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  let content = fs.existsSync(target) ? fs.readFileSync(target, "utf8") : "";
  const starts = content.split(AGENTS_SECTION_START).length - 1;
  const ends = content.split(AGENTS_SECTION_END).length - 1;
  const start = content.indexOf(AGENTS_SECTION_START);
  const end = content.indexOf(AGENTS_SECTION_END);
  if (starts === 1 && ends === 1 && end > start) {
    content =
      content.slice(0, start) + section + content.slice(end + AGENTS_SECTION_END.length);
  } else if (starts === 0 && ends === 0) {
    const separator = content.length === 0 ? "" : content.endsWith("\n") ? "\n" : "\n\n";
    content = content + separator + section + "\n";
  } else {
    // Unbalanced or duplicated markers: replacing between them could delete
    // user content, so refuse and ask for a manual cleanup.
    throw new Error(
      `~/.codex/AGENTS.md has unbalanced lilygo-skills section markers ` +
        `(${starts} start / ${ends} end); clean them up manually, then re-run install`
    );
  }
  fs.writeFileSync(target, content);
}

function hostRoot(host, home) {
  if (host === "codex") return path.join(home, ".codex", "lilygo-skills");
  if (host === "claude") return path.join(home, ".claude", "lilygo-skills");
  throw new Error(`unsupported host ${host}`);
}

function copyDir(src, dst, options = {}) {
  if (options.mirror && fs.existsSync(dst)) {
    fs.rmSync(dst, { recursive: true, force: true });
  }
  fs.mkdirSync(dst, { recursive: true });
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const from = path.join(src, entry.name);
    const to = path.join(dst, entry.name);
    if (entry.isDirectory()) {
      copyDir(from, to);
    } else if (entry.isFile()) {
      fs.copyFileSync(from, to);
    }
  }
}

function planHost(host, home) {
  const root = hostRoot(host, home);
  const integration_writes =
    host === "claude"
      ? [claudeSkillPath(home), claudeSettingsPath(home)]
      : [codexAgentsPath(home)];
  return {
    host,
    home,
    runtime_root: root,
    // The Node dispatcher (bin/**), its official MCP transport, and the data
    // model (data/**) are copied as a self-contained unit; board/peripheral
    // context is built from data/** at query time.
    materialize_plan: {
      copies: [
        "bin/**",
        "eval/official-mcp.mjs",
        "pipeline/auto-map-pins.js",
        "pipeline/extract-defines.js",
        "pipeline/pin-naming-conventions.json",
        "data/**",
        "skills/lilygo-router/SKILL.md",
      ],
      source: "bin/** dispatcher + official MCP transport + pin extractor + data/** source model",
    },
    planned_writes: [
      path.join(root, "bin", shimName()),
      pathShimPath(home),
      dispatcherPath(root),
      path.join(root, "bin", "hook.mjs"),
      path.join(root, "eval", "official-mcp.mjs"),
      path.join(root, "pipeline", "auto-map-pins.js"),
      path.join(root, "pipeline", "extract-defines.js"),
      path.join(root, "pipeline", "pin-naming-conventions.json"),
      path.join(root, "data", "boards.json"),
      path.join(root, "data", "facts", "board-fact-packs.json"),
      path.join(root, "data", "references", "source-intake", "manifest.md"),
      path.join(root, "public-skill", "SKILL.md"),
      path.join(root, "public-skill", "references"),
      ...integration_writes,
    ],
  };
}

function displayPath(target, home, repoRoot) {
  const resolved = path.resolve(target);
  const resolvedHome = path.resolve(home);
  const resolvedRepo = path.resolve(repoRoot);
  if (resolved === resolvedRepo) return ".";
  if (resolved.startsWith(resolvedRepo + path.sep)) {
    return path.relative(resolvedRepo, resolved);
  }
  if (resolved === resolvedHome) return "~";
  if (resolved.startsWith(resolvedHome + path.sep)) {
    return `~${path.sep}${path.relative(resolvedHome, resolved)}`;
  }
  if (path.isAbsolute(resolved)) {
    return `<redacted-path>${path.sep}${path.basename(resolved)}`;
  }
  return target;
}

function displayMessage(message, home, repoRoot) {
  return String(message)
    .replaceAll(path.resolve(home), "~")
    .replaceAll(path.resolve(repoRoot), ".");
}

function publicPlan(plan, home, repoRoot) {
  const runtimeRoot = displayPath(plan.runtime_root, home, repoRoot);
  return {
    copies: plan.materialize_plan.copies,
    output: path.join(runtimeRoot, "bin", "lilygo-skills.mjs"),
    source: plan.materialize_plan.source,
  };
}

function copyRouterSkill(repoRoot, runtimeRoot) {
  fs.mkdirSync(path.join(runtimeRoot, "public-skill"), { recursive: true });
  fs.copyFileSync(
    path.join(repoRoot, "skills", "lilygo-router", "SKILL.md"),
    path.join(runtimeRoot, "public-skill", "SKILL.md")
  );
  copyDir(
    path.join(repoRoot, "skills", "references"),
    path.join(runtimeRoot, "public-skill", "references"),
    { mirror: true }
  );
}

// Cross-platform `lilygo-skills` launcher: exec the dispatcher with Node so a
// person can invoke the documented `lilygo-skills <command>` name directly.
function shimContents(root) {
  const dispatcher = dispatcherPath(root);
  if (installPlatform() === "win32") {
    return `@echo off\r\nnode "${dispatcher}" %*\r\n`;
  }
  return `#!/bin/sh\nexec node "${dispatcher}" "$@"\n`;
}

function installDispatcher(plan, repoRoot) {
  const root = plan.runtime_root;
  // bin/** (dispatcher + hook + data-reading modules) and data/** move together
  // as one unit; mirroring guarantees no stale file survives a re-install.
  copyDir(path.join(repoRoot, "bin"), path.join(root, "bin"), { mirror: true });
  fs.mkdirSync(path.join(root, "eval"), { recursive: true });
  fs.copyFileSync(
    path.join(repoRoot, "eval", "official-mcp.mjs"),
    path.join(root, "eval", "official-mcp.mjs")
  );
  fs.rmSync(path.join(root, "pipeline"), { recursive: true, force: true });
  fs.mkdirSync(path.join(root, "pipeline"), { recursive: true });
  for (const name of ["auto-map-pins.js", "extract-defines.js", "pin-naming-conventions.json"]) {
    fs.copyFileSync(path.join(repoRoot, "pipeline", name), path.join(root, "pipeline", name));
  }
  copyDir(path.join(repoRoot, "data"), path.join(root, "data"), { mirror: true });
  const shimPath = path.join(root, "bin", shimName());
  fs.writeFileSync(shimPath, shimContents(root));
  fs.chmodSync(shimPath, 0o755);
  copyRouterSkill(repoRoot, root);
  if (plan.host === "claude") {
    installClaudeSkill(repoRoot, plan.home);
    mergeClaudeSettings(plan.home);
  } else if (plan.host === "codex") {
    appendCodexAgents(plan.home, root);
  }
  // Link the shim onto PATH so `lilygo-skills` (and the model's `source query`
  // self-run) resolves from any cwd. With --all both hosts install identical
  // bytes, so whichever runs last owns the PATH entry — either points at a
  // complete runtime.
  return installPathShim(plan.home, root);
}

function validateHost(plan) {
  const errors = [];
  for (const target of plan.planned_writes) {
    if (!fs.existsSync(target)) {
      errors.push(`missing installed path ${target}`);
    }
  }
  const shim = path.join(plan.runtime_root, "bin", shimName());
  if (fs.existsSync(shim) && (fs.statSync(shim).mode & 0o111) === 0) {
    errors.push(`installed launcher is not executable ${shim}`);
  }
  // The PATH shim must resolve to a real dispatcher: fs.existsSync follows
  // symlinks, so a dangling link (root moved/removed) is caught here.
  const pathShim = pathShimPath(plan.home);
  if (!fs.existsSync(pathShim)) {
    errors.push(`PATH shim missing or dangling ${pathShim}`);
  }
  if (plan.host === "claude") {
    const skill = claudeSkillPath(plan.home);
    if (fs.existsSync(skill) && !fs.readFileSync(skill, "utf8").startsWith("---")) {
      errors.push(`installed skill lacks frontmatter ${skill}`);
    }
    const settings = claudeSettingsPath(plan.home);
    if (fs.existsSync(settings)) {
      try {
        const parsed = JSON.parse(fs.readFileSync(settings, "utf8"));
        const entries = parsed?.hooks?.UserPromptSubmit;
        if (!Array.isArray(entries) || !entries.some(isLilygoHookEntry)) {
          errors.push(`settings.json has no lilygo-skills UserPromptSubmit hook`);
        }
      } catch {
        errors.push(`settings.json is not valid JSON after install`);
      }
    }
  }
  if (plan.host === "codex") {
    const agents = codexAgentsPath(plan.home);
    if (
      fs.existsSync(agents) &&
      !fs.readFileSync(agents, "utf8").includes(AGENTS_SECTION_START)
    ) {
      errors.push(`AGENTS.md lacks the lilygo-skills section`);
    }
  }
  return errors;
}

function runSelfTest(plan) {
  const dispatcher = dispatcherPath(plan.runtime_root);
  if (!fs.existsSync(dispatcher)) {
    return {
      host: plan.host,
      status: "FAIL",
      command: "node <root>/bin/lilygo-skills.mjs doctor --json",
      summary: "installed dispatcher is missing",
    };
  }
  const result = cp.spawnSync(process.execPath, [dispatcher, "doctor", "--json"], {
    cwd: plan.runtime_root,
    encoding: "utf8",
  });
  let parsed = null;
  try {
    parsed = JSON.parse(result.stdout || "{}");
  } catch (_) {}
  const injected = parsed?.sample_injection?.decision === "inject";
  const passed = result.status === 0 && parsed?.status === "PASS" && injected;
  return {
    host: plan.host,
    status: passed ? "PASS" : "FAIL",
    command: "node <root>/bin/lilygo-skills.mjs doctor --json",
    summary: injected
      ? "injection chain self-test passed"
      : "doctor did not report a passing sample injection",
    runtime_mode: parsed?.runtime_mode || "unknown",
  };
}

function main() {
  let options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    process.stdout.write(
      JSON.stringify(
        {
          status: "FAIL",
          errors: [error.message],
        },
        null,
        2
      ) + "\n"
    );
    process.exit(2);
  }
  const repoRoot = __dirname;
  const plans = options.hosts.map((host) => planHost(host, options.home));
  const warnings = [];
  if (options.build) {
    warnings.push("--build is a no-op for the JS dispatcher; there is nothing to compile");
  }
  const writes = [];
  const errors = [];
  const verified_writes = [];
  const self_tests = [];
  if (!options.dryRun) {
    for (const plan of plans) {
      let applied = false;
      try {
        const shim = installDispatcher(plan, repoRoot);
        applied = true;
        if (shim && !shim.onPath) {
          if (shim.rcUpdated && shim.rcUpdated.added) {
            warnings.push(
              `added ${shim.binDir} to PATH via ${shim.rcUpdated.rc}; ` +
                `restart your shell or run: source ${shim.rcUpdated.rc} ` +
                `(so \`lilygo-skills\` and the model's source-query pull resolve)`
            );
          } else {
            warnings.push(
              `${shim.binDir} is not on your PATH; add it (e.g. export ` +
                `PATH="${shim.binDir}:$PATH") so \`lilygo-skills\` resolves`
            );
          }
        }
      } catch (error) {
        errors.push(`${plan.host}: ${error.message}`);
      }
      if (applied) {
        writes.push(...plan.planned_writes);
      }
      errors.push(...validateHost(plan).map((error) => `${plan.host}: ${error}`));
      verified_writes.push(
        ...plan.planned_writes.filter((target) => fs.existsSync(target))
      );
    }
    if (options.selfTest && errors.length === 0) {
      for (const plan of plans) {
        const selfTest = runSelfTest(plan);
        self_tests.push(selfTest);
        if (selfTest.status !== "PASS") {
          errors.push(`${plan.host}: install self-test failed: ${selfTest.summary}`);
        }
      }
    }
  }
  const status = errors.length === 0 ? "PASS" : "FAIL";
  process.stdout.write(
    JSON.stringify(
      {
        status,
        hosts: plans.map((plan) => plan.host),
        mode: options.dryRun ? "dry-run" : "apply",
        runtime_mode: "js-dispatcher",
        self_tests,
        materialize_plans: plans.map((plan) => publicPlan(plan, options.home, repoRoot)),
        planned_writes: plans.flatMap((plan) =>
          plan.planned_writes.map((target) => displayPath(target, options.home, repoRoot))
        ),
        writes: writes.map((target) => displayPath(target, options.home, repoRoot)),
        verified_writes: verified_writes.map((target) =>
          displayPath(target, options.home, repoRoot)
        ),
        errors: errors.map((error) => displayMessage(error, options.home, repoRoot)),
        warnings: warnings.map((warning) => displayMessage(warning, options.home, repoRoot)),
      },
      null,
      2
    ) + "\n"
  );
  process.exit(status === "PASS" ? 0 : 2);
}

main();
