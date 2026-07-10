// Shared source I/O for the ingest/verify pipeline. Fetch an official header
// over the ambient http(s) proxy (curl), and slice an inclusive 1-based line
// range. Deduped from ingest-from-manifest / verify-auto-mapping /
// verify-source-authority, which previously each carried byte-identical copies.
const { execFileSync } = require("child_process");

function fetchText(url) {
  return execFileSync("curl", ["-sfL", "--max-time", "30", url], {
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  });
}

function sliceRange(text, range) {
  const [a, b] = range.split("-").map((n) => parseInt(n, 10));
  return text.split("\n").slice(a - 1, b).join("\n");
}

module.exports = { fetchText, sliceRange };
