#!/usr/bin/env node
const fs = require("fs");
const os = require("os");
const path = require("path");
const cp = require("child_process");

function parseArgs(argv) {
  const valueFlags = new Set(["--home", "--profile", "--bin"]);
  const booleanFlags = new Set([
    "--codex",
    "--claude",
    "--all",
    "--dry-run",
    "--build",
    "--no-self-test",
    "--prebuilt-only",
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
  const profile = optionValue("--profile") ?? "auto";
  if (!["auto", "release", "debug"].includes(profile)) {
    throw new Error("--profile must be auto, release, or debug");
  }
  if (args.has("--prebuilt-only") && (args.has("--build") || args.has("--bin"))) {
    throw new Error("--prebuilt-only cannot be combined with --build or --bin");
  }
  return {
    dryRun: args.has("--dry-run"),
    build: args.has("--build"),
    hosts,
    home: optionValue("--home") ?? os.homedir(),
    profile,
    bin: optionValue("--bin"),
    selfTest: !args.has("--no-self-test"),
    prebuiltOnly: args.has("--prebuilt-only"),
  };
}

const AGENTS_SECTION_START = "<!-- lilygo-skills:start -->";
const AGENTS_SECTION_END = "<!-- lilygo-skills:end -->";

function installPlatform() {
  return process.env.LILYGO_INSTALL_PLATFORM || process.platform;
}

function runtimeBinaryName() {
  return installPlatform() === "win32" ? "lilygo-skills.exe" : "lilygo-skills";
}

function prebuiltPlatformId() {
  const platform = installPlatform();
  const arch = process.env.LILYGO_INSTALL_ARCH || process.arch;
  if (platform === "darwin" && arch === "arm64") return "macos-arm64";
  if (platform === "darwin" && arch === "x64") return "macos-x64";
  if (platform === "linux" && arch === "arm64") return "linux-arm64";
  if (platform === "linux" && arch === "x64") return "linux-x64";
  return `${platform}-${arch}`;
}

function prebuiltBinaryPath(repoRoot) {
  return path.join(repoRoot, "dist", "bin", prebuiltPlatformId(), runtimeBinaryName());
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

// Quoted for space-safe shells, $HOME-relative so the entry survives home
// moves; hook commands run through a shell that expands $HOME inside quotes.
// A non-default --home cannot rely on $HOME at hook runtime, so it gets the
// absolute path instead.
function claudeHookCommand(home) {
  if (home && path.resolve(home) !== path.resolve(os.homedir())) {
    return `"${path.join(home, ".claude", "lilygo-skills", "bin", runtimeBinaryName())}" hook claude`;
  }
  return `"$HOME/.claude/lilygo-skills/bin/${runtimeBinaryName()}" hook claude`;
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

function isLilygoHookCommand(hook) {
  return (
    typeof hook?.command === "string" &&
    hook.command.includes("lilygo-skills") &&
    hook.command.includes("hook claude")
  );
}

function isLilygoHookEntry(entry) {
  return entry && Array.isArray(entry.hooks) && entry.hooks.some(isLilygoHookCommand);
}

function mergeClaudeSettings(home, root) {
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
  const bin = path.join(root, "bin", runtimeBinaryName());
  return [
    AGENTS_SECTION_START,
    "## LilyGO Skills",
    "",
    `LilyGO board context runtime lives at \`${root}\`.`,
    "For any prompt about LilyGO boards (T-Display, T-Watch, T-Beam, T-Deck and",
    "other LilyGO products), firmware, flashing, LVGL, OTA, LoRa/GNSS, sensors,",
    "battery, or pinouts:",
    "",
    `1. Run \`"${bin}" route --json "<prompt>"\` to resolve skill ids.`,
    `2. Read the matched files under \`${path.join(root, "skills")}/<skill-id>/SKILL.md\`.`,
    `3. For debug/implementation goals use \`"${bin}" goal complete --dry-run --json "<prompt>"\`.`,
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
    // Generated skills are produced from the source model, not copied from a
    // committed snapshot (meta-only release boundary).
    generate_plan: {
      command: `lilygo-skills generate skills --out ${root} --json`,
      output: path.join(root, "skills"),
      source: "data/** source model",
    },
    planned_writes: [
      path.join(root, "bin", runtimeBinaryName()),
      path.join(root, "index", "routes.json"),
      path.join(root, "skills"),
      path.join(root, "skills", "references"),
      path.join(root, "templates", "skills"),
      path.join(root, "data", "boards.json"),
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
    command: `lilygo-skills generate skills --out ${runtimeRoot} --json`,
    output: path.join(runtimeRoot, "skills"),
    source: plan.generate_plan.source,
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

function fallbackRuntimeScript(repoRoot, runtimeRoot) {
  return `#!/usr/bin/env node
const fs = require("fs");
const path = require("path");
const cp = require("child_process");

const SOURCE_ROOT = ${JSON.stringify(path.resolve(repoRoot))};
const RUNTIME_ROOT = ${JSON.stringify(path.resolve(runtimeRoot))};

function runtimeBinaryName() {
  return process.platform === "win32" ? "lilygo-skills.exe" : "lilygo-skills";
}

function realRuntime() {
  const candidates = [
    process.env.LILYGO_SKILLS_REAL_BIN,
    path.join(SOURCE_ROOT, "target", "release", runtimeBinaryName()),
    path.join(SOURCE_ROOT, "target", "debug", runtimeBinaryName()),
    path.join(RUNTIME_ROOT, "bin", "lilygo-skills-real"),
  ].filter(Boolean);
  return candidates.find((candidate) => fs.existsSync(candidate));
}

function tryRealRuntime(args) {
  const real = realRuntime();
  if (!real) return false;
  const result = cp.spawnSync(real, args, { stdio: "inherit" });
  process.exit(result.status === null ? 1 : result.status);
}

function readInput() {
  try {
    return fs.readFileSync(0, "utf8");
  } catch (_) {
    return "";
  }
}

function promptFromInput(input) {
  try {
    const parsed = JSON.parse(input);
    if (typeof parsed?.prompt === "string") return parsed.prompt;
  } catch (_) {}
  return input;
}

function promptFromRouteArgs(args) {
  const jsonIndex = args.indexOf("--json");
  if (jsonIndex >= 0 && args[jsonIndex + 1]) return args[jsonIndex + 1];
  return args.slice(1).join(" ");
}

function isLilygoPrompt(prompt) {
  return /LilyGO|T-Display|T-Watch|T-Beam|T-Deck|T-Echo|T-SIM|ESP32|ESP-IDF|PlatformIO|Arduino|LVGL|LoRa|GNSS|IMU|OTA|烧录|显示|固件|串口|传感器/i.test(prompt);
}

function setupPlan(framework) {
  const common = [
    ["rustup", ["cli-runtime"], "rustup --version", "Install from https://rustup.rs/."],
    ["cargo", ["cli-runtime"], "cargo --version", "Installed by rustup; required to build the full lilygo-skills runtime."],
    ["node", ["installer"], "node --version", "Install Node.js LTS for install.js."],
    ["git", ["source"], "git --version", "Install Git for LilyGO, Espressif, and reference source checkouts."],
  ];
  const frameworkTools = {
    arduino: [
      ["arduino-cli", ["arduino"], "arduino-cli version", "Install Arduino CLI from https://docs.arduino.cc/arduino-cli/."],
      ["arduino-esp32-core", ["arduino"], "arduino-cli core list | grep esp32:esp32", "Use arduino-cli core update-index and core install esp32:esp32."],
      ["lilygo-libraries", ["arduino"], "arduino-cli lib list | grep -i LilyGo", "Install LilyGo libraries following the official repository guidance."],
      ["serial-mcp-server", ["serial-debug"], "serial-mcp-server --help", "Optional serial observation loop: https://github.com/Adancurusul/serial-mcp-server."],
    ],
    platformio: [
      ["python3", ["platformio"], "python3 --version", "Install Python 3 before PlatformIO Core."],
      ["platformio-core", ["platformio"], "pio --version", "Install PlatformIO Core from https://docs.platformio.org/."],
      ["platformio-esp32-platform", ["platformio"], "pio pkg list --global | grep espressif32", "PlatformIO resolves espressif32 from platformio.ini or pio pkg install."],
      ["serial-mcp-server", ["serial-debug"], "serial-mcp-server --help", "Optional serial observation loop for pio device monitor output."],
    ],
    "esp-idf": [
      ["python3", ["esp-idf"], "python3 --version", "Install Python 3 for ESP-IDF tooling."],
      ["esp-idf", ["esp-idf"], "idf.py --version", "Install ESP-IDF from Espressif get-started docs for ESP32-S3."],
      ["idf-tools", ["esp-idf"], "python3 $IDF_PATH/tools/idf_tools.py list", "Use the official install script to provision compiler, OpenOCD, and Python environment."],
      ["serial-mcp-server", ["serial-debug"], "serial-mcp-server --help", "Optional serial observation loop for idf.py monitor output."],
    ],
    rust: [
      ["espup", ["rust", "esp-rs"], "espup --version", "Install with cargo install espup and run espup install."],
      ["espflash", ["rust", "flash", "serial"], "espflash --version", "Install with cargo install espflash."],
      ["cargo-espflash", ["rust", "flash"], "cargo espflash --version", "Install with cargo install cargo-espflash when using cargo espflash."],
      ["serial-mcp-server", ["serial-debug"], "serial-mcp-server --help", "Optional serial observation loop for espflash monitor output."],
    ],
  };
  const selected = frameworkTools[framework];
  if (!selected) {
    return { status: "FAIL", errors: ["framework must be arduino, platformio, esp-idf, or rust"] };
  }
  const toTool = ([id, required_for, check, install_hint]) => ({
    id,
    required_for,
    check,
    install_hint,
    mutates: false,
  });
  return {
    schema_version: 1,
    framework,
    status: "planned",
    runtime_mode: "mount-only",
    dry_run: true,
    no_mutation: true,
    host_requirements: ["rustup", "cargo", "node", "git"],
    toolchains: common.concat(selected).map(toTool),
    next_commands: [
      "node install.js --all --dry-run --build",
      "node install.js --all --build",
      framework === "platformio" ? "pio --version" : null,
      framework === "arduino" ? "arduino-cli version" : null,
      framework === "esp-idf" ? "idf.py --version" : null,
      framework === "rust" ? "espup --version" : null,
    ].filter(Boolean),
    private_inputs_needed: [
      "USB serial port is needed only for later flash/monitor commands",
      "Wi-Fi credentials or OTA target must stay in private local config if needed later",
    ],
    writes: [],
  };
}

function runtimeMissingContext() {
  return "LilyGO Skill is mounted in setup-only mode. The full Rust runtime binary is not installed yet, so dynamic board facts and generated skills are not available. For setup, run 'lilygo-skills setup plan --framework <arduino|platformio|esp-idf|rust> --json'. To enable full context injection, build or provide the runtime with 'node install.js --all --build' or 'node install.js --all --bin /path/to/lilygo-skills'.";
}

function emitHook(host, prompt) {
  if (!isLilygoPrompt(prompt)) {
    if (host === "claude") {
      console.log(JSON.stringify({ hookSpecificOutput: { hookEventName: "UserPromptSubmit" } }, null, 2));
    } else {
      console.log(JSON.stringify({ decision: "no-op", context: "", fail_open: true, host }, null, 2));
    }
    return;
  }
  const context = runtimeMissingContext();
  if (host === "claude") {
    console.log(JSON.stringify({ hookSpecificOutput: { hookEventName: "UserPromptSubmit", additionalContext: context } }, null, 2));
  } else {
    console.log(JSON.stringify({
      decision: "needs_runtime_setup",
      context,
      fail_open: true,
      host,
      missing: ["runtime-binary"],
      questions: [],
      skills: ["lilygo-router"]
    }, null, 2));
  }
}

const args = process.argv.slice(2);
tryRealRuntime(args);

if (args[0] === "setup" && args[1] === "--help") {
  console.log("setup plan --framework <arduino|platformio|esp-idf|rust> --json");
  process.exit(0);
}
if (args[0] === "setup" && args[1] === "plan") {
  const framework = args[args.indexOf("--framework") + 1];
  const plan = setupPlan(framework);
  console.log(JSON.stringify(plan, null, 2));
  process.exit(plan.status === "FAIL" ? 2 : 0);
}
if (args[0] === "hook") {
  emitHook(args[1] || "codex", promptFromInput(readInput()));
  process.exit(0);
}
if (args[0] === "route") {
  const prompt = promptFromRouteArgs(args);
  const matched = isLilygoPrompt(prompt);
  console.log(JSON.stringify(matched ? {
    decision: "needs_runtime_setup",
    skills: ["lilygo-router"],
    missing: ["runtime-binary"],
    questions: [],
    paths: { "lilygo-router": "skills/lilygo-router/SKILL.md" },
    notes: [runtimeMissingContext()]
  } : { decision: "no-op", skills: [], missing: [], questions: [] }, null, 2));
  process.exit(0);
}
if (args[0] === "verify" || args[0] === "doctor") {
  console.log(JSON.stringify({
    status: "PASS",
    runtime_mode: "mount-only",
    full_runtime_available: false,
    checks: [
      { id: "mount-only", status: "WARN", summary: runtimeMissingContext() }
    ],
    sample_injection: {
      status: "WARN",
      prompt: "T-Display-S3 PlatformIO Arduino TFT_eSPI first screen",
      matched_skills: ["lilygo-router"],
      no_op_status: "not_checked"
    },
    warnings: [runtimeMissingContext()]
  }, null, 2));
  process.exit(0);
}
if (args.length === 0 || args[0] === "--help" || args[0] === "help") {
  console.log("lilygo-skills mount-only launcher: setup|hook|route|verify|doctor. Build the Rust runtime for full dynamic context.");
  process.exit(0);
}
console.log(JSON.stringify({ status: "FAIL", runtime_mode: "mount-only", errors: [runtimeMissingContext()] }, null, 2));
process.exit(2);
`;
}

function installFallbackRuntime(plan, repoRoot) {
  const binPath = path.join(plan.runtime_root, "bin", runtimeBinaryName());
  fs.mkdirSync(path.join(plan.runtime_root, "bin"), { recursive: true });
  fs.writeFileSync(binPath, fallbackRuntimeScript(repoRoot, plan.runtime_root));
  fs.chmodSync(binPath, 0o755);
  copyDir(path.join(repoRoot, "data"), path.join(plan.runtime_root, "data"), {
    mirror: true,
  });
  copyDir(path.join(repoRoot, "index"), path.join(plan.runtime_root, "index"), {
    mirror: true,
  });
  fs.rmSync(path.join(plan.runtime_root, "skills"), { recursive: true, force: true });
  fs.mkdirSync(path.join(plan.runtime_root, "skills", "lilygo-router"), {
    recursive: true,
  });
  fs.copyFileSync(
    path.join(repoRoot, "skills", "lilygo-router", "SKILL.md"),
    path.join(plan.runtime_root, "skills", "lilygo-router", "SKILL.md")
  );
  copyDir(
    path.join(repoRoot, "skills", "references"),
    path.join(plan.runtime_root, "skills", "references"),
    { mirror: true }
  );
  copyDir(
    path.join(repoRoot, "templates", "skills"),
    path.join(plan.runtime_root, "templates", "skills"),
    { mirror: true }
  );
  copyRouterSkill(repoRoot, plan.runtime_root);
  if (plan.host === "claude") {
    installClaudeSkill(repoRoot, plan.home);
    mergeClaudeSettings(plan.home, plan.runtime_root);
  } else if (plan.host === "codex") {
    appendCodexAgents(plan.home, plan.runtime_root);
  }
}

function applyHost(plan, repoRoot, binaryPath) {
  if (!binaryPath) {
    installFallbackRuntime(plan, repoRoot);
    return;
  }
  const binPath = path.join(plan.runtime_root, "bin", runtimeBinaryName());
  fs.mkdirSync(path.join(plan.runtime_root, "bin"), { recursive: true });
  fs.copyFileSync(binaryPath, binPath);
  fs.chmodSync(binPath, 0o755);
  // Meta-only release boundary: the source tree ships no generated SKILL.md.
  // Install materializes every runtime skill from the source model into the
  // install root instead of copying committed snapshots.
  copyDir(path.join(repoRoot, "data"), path.join(plan.runtime_root, "data"), {
    mirror: true,
  });
  fs.rmSync(path.join(plan.runtime_root, "doc", "references", "source-intake"), {
    recursive: true,
    force: true,
  });
  const generated = cp.spawnSync(
    binPath,
    ["generate", "skills", "--out", plan.runtime_root, "--json"],
    { cwd: repoRoot, encoding: "utf8" }
  );
  if (generated.status !== 0) {
    throw new Error(
      `generate skills failed: ${(generated.stderr || generated.stdout || "").trim()}`
    );
  }
  // Single source: the public/meta skill is the committed router skill.
  copyRouterSkill(repoRoot, plan.runtime_root);
  if (plan.host === "claude") {
    installClaudeSkill(repoRoot, plan.home);
    mergeClaudeSettings(plan.home, plan.runtime_root);
  } else if (plan.host === "codex") {
    appendCodexAgents(plan.home, plan.runtime_root);
  }
}

function validateHost(plan) {
  const errors = [];
  for (const target of plan.planned_writes) {
    if (!fs.existsSync(target)) {
      errors.push(`missing installed path ${target}`);
    }
  }
  const bin = path.join(plan.runtime_root, "bin", runtimeBinaryName());
  if (fs.existsSync(bin) && (fs.statSync(bin).mode & 0o111) === 0) {
    errors.push(`installed binary is not executable ${bin}`);
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
  const bin = path.join(plan.runtime_root, "bin", runtimeBinaryName());
  if (!fs.existsSync(bin)) {
    return {
      host: plan.host,
      status: "FAIL",
      command: "lilygo-skills doctor --json --home <home>",
      summary: "installed runtime binary is missing",
    };
  }
  const result = cp.spawnSync(bin, ["doctor", "--json", "--home", plan.home], {
    cwd: plan.runtime_root,
    encoding: "utf8",
  });
  let parsed = null;
  try {
    parsed = JSON.parse(result.stdout || "{}");
  } catch (_) {}
  const passed = result.status === 0 && parsed?.status === "PASS";
  return {
    host: plan.host,
    status: passed ? "PASS" : "FAIL",
    command: "lilygo-skills doctor --json --home <home>",
    summary:
      parsed?.sample_injection?.status === "PASS"
        ? "injection chain self-test passed"
        : "doctor did not report a passing sample injection",
    runtime_mode: parsed?.runtime_mode || "unknown",
  };
}

function resolveBinary(repoRoot, options) {
  if (options.prebuiltOnly) {
    return {
      path: prebuiltBinaryPath(repoRoot),
      profile: "prebuilt",
      build_hint: `install a release bundle containing dist/bin/${prebuiltPlatformId()}/${runtimeBinaryName()}`,
    };
  }
  if (options.bin) {
    return {
      path: path.resolve(repoRoot, options.bin),
      profile: "custom",
      build_hint: "provide an existing executable with --bin <path>",
    };
  }
  const candidates = {
    release: path.join(repoRoot, "target", "release", runtimeBinaryName()),
    debug: path.join(repoRoot, "target", "debug", runtimeBinaryName()),
  };
  if (options.build && options.profile === "auto") {
    return {
      path: candidates.release,
      profile: "release",
      build_hint: "run cargo build --release -p lilygo-skills-cli",
    };
  }
  if (options.profile === "release") {
    return {
      path: candidates.release,
      profile: "release",
      build_hint: "run cargo build --release -p lilygo-skills-cli",
    };
  }
  if (options.profile === "debug") {
    return {
      path: candidates.debug,
      profile: "debug",
      build_hint: "run cargo build -p lilygo-skills-cli",
    };
  }
  const existing = Object.entries(candidates)
    .filter(([, candidate]) => fs.existsSync(candidate))
    .map(([profile, candidate]) => ({
      profile,
      path: candidate,
      mtimeMs: fs.statSync(candidate).mtimeMs,
    }))
    .sort((left, right) => right.mtimeMs - left.mtimeMs);
  if (existing.length > 0) {
    const selected = existing[0];
    return {
      path: selected.path,
      profile: selected.profile,
      build_hint:
        selected.profile === "release"
          ? "run cargo build --release -p lilygo-skills-cli"
          : "run cargo build -p lilygo-skills-cli",
    };
  }
  return {
    path: candidates.release,
    profile: "release",
    build_hint: "run cargo build --release -p lilygo-skills-cli",
  };
}

function buildPlan(repoRoot, options, binary) {
  if (options.prebuiltOnly || !options.build || options.bin) {
    return {
      enabled: false,
      profile: binary.profile,
      command: null,
      binary: binary.path,
    };
  }
  const release = options.profile !== "debug";
  const command = release
    ? ["cargo", "build", "--release", "-p", "lilygo-skills-cli"]
    : ["cargo", "build", "-p", "lilygo-skills-cli"];
  return {
    enabled: true,
    profile: release ? "release" : "debug",
    command,
    binary: binary.path,
  };
}

function runBuild(plan, repoRoot) {
  if (!plan.enabled) return null;
  const result = cp.spawnSync(plan.command[0], plan.command.slice(1), {
    cwd: repoRoot,
    encoding: "utf8",
  });
  return {
    status: result.status === 0 ? "PASS" : "FAIL",
    exit_code: result.status,
    stderr: (result.stderr || "").trim().slice(-4000),
    stdout: (result.stdout || "").trim().slice(-4000),
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
  const binary = resolveBinary(repoRoot, options);
  const build = buildPlan(repoRoot, options, binary);
  const binaryExists = fs.existsSync(binary.path);
  const mountOnly = !binaryExists && !build.enabled && !options.bin && !options.prebuiltOnly;
  const runtimeMode =
    mountOnly ? "mount-only" : options.prebuiltOnly && !binaryExists ? "prebuilt-missing" : "full";
  if (!binaryExists && !build.enabled) {
    warnings.push(
      mountOnly
        ? `runtime binary missing at ${binary.path}; installing setup-only mount; ${binary.build_hint} for full dynamic context`
        : `runtime binary missing at ${binary.path}; ${binary.build_hint}`
    );
  }
  const writes = [];
  const errors = [];
  const verified_writes = [];
  const self_tests = [];
  let build_result = null;
  if (!options.dryRun) {
    if (options.prebuiltOnly && !binaryExists) {
      errors.push(
        `prebuilt runtime missing at ${binary.path}; ${binary.build_hint}`
      );
    }
    build_result = runBuild(build, repoRoot);
    if (build_result && build_result.status !== "PASS") {
      errors.push(
        `build failed: ${build.command.join(" ")} exited ${build_result.exit_code}; ${
          build_result.stderr || build_result.stdout
        }`
      );
    }
    if (!build_result || build_result.status === "PASS") {
      for (const plan of plans) {
        let applied = false;
        try {
          if (!mountOnly && !fs.existsSync(binary.path)) {
            throw new Error(`runtime binary missing at ${binary.path}; ${binary.build_hint}`);
          }
          applyHost(plan, repoRoot, mountOnly ? null : binary.path);
          applied = true;
        } catch (error) {
          errors.push(`${plan.host}: ${error.message}`);
        }
        if (applied) {
          writes.push(...plan.planned_writes);
        }
        errors.push(
          ...validateHost(plan).map((error) => `${plan.host}: ${error}`)
        );
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
  }
  const warningsAllowed = options.dryRun || mountOnly;
  const status =
    (warnings.length === 0 || warningsAllowed) && errors.length === 0
      ? "PASS"
      : "FAIL";
  process.stdout.write(
    JSON.stringify(
      {
        status,
        hosts: plans.map((plan) => plan.host),
        mode: options.dryRun ? "dry-run" : "apply",
        runtime_mode: runtimeMode,
        prebuilt_only: options.prebuiltOnly,
        prebuilt_platform: prebuiltPlatformId(),
        prebuilt_available: options.prebuiltOnly ? binaryExists : null,
        binary_profile: binary.profile,
        binary_source: displayPath(binary.path, options.home, repoRoot),
        build_plan: {
          enabled: build.enabled,
          profile: build.profile,
          command: build.command ? build.command.join(" ") : null,
          binary: displayPath(build.binary, options.home, repoRoot),
        },
        build_result: build_result
          ? {
              status: build_result.status,
              exit_code: build_result.exit_code,
            }
          : null,
        self_tests,
        generate_plans: plans.map((plan) => publicPlan(plan, options.home, repoRoot)),
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
