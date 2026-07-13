#!/usr/bin/env node
// Command dispatcher for the LilyGO JS context kernel. The command surface
// mirrors the Rust CLI (target/release/lilygo-skills) exactly so installed
// surfaces can point here without changing any documented invocation:
//   lilygo-skills context [--project <dir>] [--json] [prompt]
//   lilygo-skills source query --board <id> --topic <topic> --json
//   lilygo-skills verify sources --board <id> [--topic <topic>] --json
//   lilygo-skills doctor --json
import { runContext } from "./find.mjs";
import { runSourceQuery } from "./query.mjs";
import { runVerify } from "./verify.mjs";
import { runDoctor } from "./doctor.mjs";
import { runHookCommand } from "./hook.mjs";

const USAGE =
  "Usage: lilygo-skills <command>\n\n" +
  "  hook <claude|codex>                              push the thick board capsule (stdin: {\"prompt\":..})\n" +
  "  context [--project <dir>] [--json] [prompt]      resolve board + thin capsule\n" +
  "  source query --board <id> --topic <t> --json     source-cited facts for a topic\n" +
  "  verify sources --board <id> [--topic <t>] --json live re-proof (OK/DRIFT/UNREACHABLE)\n" +
  "  doctor --json                                    data-integrity self-check\n";

/**
 * @param {string[]} argv full argv tail (command first)
 * @returns {Promise<number>} exit code
 */
export async function dispatch(argv) {
  const command = argv[0];
  switch (command) {
    case "hook":
      return runHookCommand(argv.slice(1));
    case "context":
      return runContext(argv);
    case "source":
      if (argv[1] === "query") return runSourceQuery(argv);
      process.stderr.write("unknown source subcommand; expected: source query\n");
      return 2;
    case "verify":
      if (argv[1] === "sources") return runVerify(argv);
      process.stderr.write("unknown verify subcommand; expected: verify sources\n");
      return 2;
    case "doctor":
      return runDoctor(argv);
    case "--help":
    case "-h":
    case undefined:
      process.stdout.write(USAGE);
      return 0;
    default:
      process.stderr.write(`unknown command: ${command}\n\n${USAGE}`);
      return 2;
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  dispatch(process.argv.slice(2)).then((code) => process.exit(code));
}
