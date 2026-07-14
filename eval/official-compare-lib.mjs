import { createHash } from "node:crypto";

/** @typedef {{ product: string; title: string; category?: string; tags?: string[]; shopLink?: string }} OfficialProduct */
/** @typedef {{ aliases: string[]; primary: string; value: string; raw_label: string; raw_value: string; path: string; variant?: string }} OfficialSignal */
/** @typedef {{ aliases: string[]; primary: string; value: string; key: string; raw_value: string; citation: { url: string; line_range: string | null; sha256: string } }} OursSignal */

const GPIO_RE = /\b(?:GPIO|IO)\s*0*([0-9]{1,3})\b/i;
const SHA256_RE = /^sha256:[0-9a-f]{64}$/;

/**
 * @param {string} value
 * @returns {string}
 */
export function canonicalName(value) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "");
}

/**
 * Map shipped board IDs to official products using normalized exact names.
 * Ambiguous prefix matches are deliberately left unmapped.
 * @param {string[]} boardIds
 * @param {OfficialProduct[]} products
 */
export function mapBoardsToProducts(boardIds, products) {
  return boardIds.map((boardId) => {
    const boardName = boardId.replace(/^board-/, "");
    const target = canonicalName(boardName);
    const matches = products.filter((product) =>
      canonicalName(product.product) === target || canonicalName(product.title) === target
    );
    if (matches.length !== 1) {
      return {
        board_id: boardId,
        status: "no-official-coverage",
        official_product: null,
        official_title: null,
        match_method: matches.length > 1 ? "ambiguous-normalized-name" : "no-normalized-name-match",
      };
    }
    const match = /** @type {OfficialProduct} */ (matches[0]);
    return {
      board_id: boardId,
      status: "mapped",
      official_product: match.product,
      official_title: match.title,
      match_method: canonicalName(match.product) === target ? "normalized-product-id" : "normalized-title",
    };
  });
}

/**
 * @param {Record<string, unknown>} result
 * @param {string} toolName
 * @returns {unknown}
 */
export function parseToolJson(result, toolName) {
  if (!Array.isArray(result.content)) throw new Error(`${toolName} returned no content array`);
  const texts = result.content
    .filter((item) => isRecord(item) && item.type === "text" && typeof item.text === "string")
    .map((item) => /** @type {string} */ (item.text));
  if (texts.length === 0) throw new Error(`${toolName} returned no text content`);
  const raw = texts.join("\n");
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`${toolName} returned unparseable JSON: ${error instanceof Error ? error.message : String(error)}; raw=${raw}`);
  }
}

/**
 * @param {unknown} value
 * @returns {value is Record<string, unknown>}
 */
function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * @param {string} value
 * @returns {string | undefined}
 */
function normalizeGpio(value) {
  const match = value.match(GPIO_RE);
  if (match?.[1]) return `GPIO${Number(match[1])}`;
  if (/^\s*0*[0-9]{1,3}\s*$/.test(value)) return `GPIO${Number(value.trim())}`;
  return undefined;
}

/**
 * @param {string} text
 * @returns {Set<string>}
 */
function categoriesFor(text) {
  const normalized = ` ${text.toLowerCase().replace(/i[²2]c/g, "i2c").replace(/[_./-]+/g, " ")} `;
  /** @type {Set<string>} */
  const categories = new Set();
  /** @param {string} category @param {RegExp} pattern */
  const add = (category, pattern) => { if (pattern.test(normalized)) categories.add(category); };
  add("display", /\b(display|lcd|tft|amoled|st77\w+|gc9\w+|rm67\w+|sh86\w+|icma\w+|axs15\w+|co5300)\b/);
  add("touch", /\b(touch|cst\w+|ft\d+|chsc\w+)\b/);
  add("lora", /\b(lora|radio|sx12\w+|lr1121)\b/);
  add("gps", /\b(gps|gnss|mia-m10\w*|ublox|u-blox)\b/);
  add("sd", /\b(sd|sdcard|sd card|tf card|microsd|storage)\b/);
  add("i2c", /\bi2c\d*\b/);
  add("spi", /\bspi\b/);
  add("qspi", /\bqspi\b/);
  add("i2s", /\bi2s\b/);
  add("pdm", /\bpdm\b/);
  add("uart", /\buart\b/);
  add("button", /\b(button|boot|user|key[12])\b/);
  add("battery", /\b(battery|bat voltage|adc)\b/);
  add("audio", /\b(speaker|max98357\w*|es7210|es8311|audio|dac)\b/);
  add("mic", /\b(mic|microphone|mp34\w+|msm261\w+|pdm)\b/);
  add("encoder", /\bencoder\b/);
  add("encoder", /\brotary\b/);
  add("keyboard", /\b(keyboard|tca8418)\b/);
  add("motor", /\bmotor\b/);
  add("ir", /\b(ir|infrared)\b/);
  add("buzzer", /\bbuzzer\b/);
  add("nfc", /\b(nfc|st25r\w+)\b/);
  add("sensor", /\b(sensor|bhi260\w*|bma423|imu)\b/);
  add("power", /\b(power|pmu|axp\w+|sy6970)\b/);
  add("rtc", /\b(rtc|pcf85\w+)\b/);
  add("camera", /\b(camera|ov2640|ov5640)\b/);
  add("led", /\b(led|apa102|ws2812\w*)\b/);
  add("ethernet", /\b(ethernet|w5500)\b/);
  add("rs485", /\brs485\b/);
  add("rs232", /\brs232\b/);
  add("can", /\b(can|twai)\b/);
  add("halow", /\bhalow\b/);
  add("halow", /\btx[-_ ]?ah\b/);
  add("sdmmc", /\bsdmmc\b/);
  return categories;
}

/**
 * @param {string} text
 * @returns {string | undefined}
 */
function signalFor(text) {
  const normalized = text.toLowerCase()
    .replace(/data[-_ ]?out/g, "dout")
    .replace(/data[-_ ]?in/g, "din")
    .replace(/sdio([0-7])/g, "d$1")
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
  /** @param {RegExp} pattern */
  const has = (pattern) => pattern.test(normalized);
  const indexed = normalized.match(/\bd([0-7])\b/);
  if (indexed?.[1]) return `d${indexed[1]}`;
  if (has(/\b(backlight|bl)\b/)) return "backlight";
  if (has(/\b(power enable|power en|poweron|power on|vci en)\b/)) return "power";
  if (has(/\b(reset|rst|res)\b/)) return "reset";
  if (has(/\bpwdn\b/)) return "pwdn";
  if (has(/\b(mosi|cmd)\b/)) return has(/\bcmd\b/) ? "cmd" : "mosi";
  if (has(/\b(miso|sdo)\b/)) return "miso";
  if (has(/\b(xclk)\b/)) return "xclk";
  if (has(/\b(pclk)\b/)) return "pclk";
  if (has(/\b(sclk|sck|clock|clk)\b/)) return "sck";
  if (has(/\b(bclk|bck)\b/)) return "bclk";
  if (has(/\b(wclk|lrclk|lrck|ws)\b/)) return "ws";
  if (has(/\bmclk\b/)) return "mclk";
  if (has(/\bdout\b/)) return "dout";
  if (has(/\bdin\b/)) return "din";
  if (has(/\b(data|dat)\b/)) return "data";
  if (has(/\b(sda|siod)\b/)) return has(/\bsiod\b/) ? "siod" : "sda";
  if (has(/\b(scl|sioc)\b/)) return has(/\bsioc\b/) ? "sioc" : "scl";
  if (has(/\b(cs|nss)\b/)) return "cs";
  if (has(/\bdc\b/)) return "dc";
  if (has(/\bwr\b/)) return "wr";
  if (has(/\brd\b/)) return "rd";
  if (has(/\bte\b/)) return "te";
  if (has(/\bbusy\b/)) return "busy";
  if (has(/\bdio0\b/)) return "dio0";
  if (has(/\b(dio1|irq|interrupt|int)\b/)) return "int";
  if (has(/\b(txd|tx)\b/)) return "tx";
  if (has(/\b(rxd|rx)\b/)) return "rx";
  if (has(/\bpps\b/)) return "pps";
  if (has(/\b(adc|voltage)\b/)) return "voltage";
  if (has(/\b(vsync)\b/)) return "vsync";
  if (has(/\bhref\b/)) return "href";
  if (has(/\b(boot|button 1|key1)\b/)) return "1";
  if (has(/\b(user|button 2|key2)\b/)) return "2";
  if (has(/\b(button pwr|button power|pwr button)\b/)) return "pwr";
  if (has(/\bwakeup\b/)) return "wakeup";
  if (has(/\b(center)\b/)) return "center";
  if (has(/\b(encoder|rotary) a\b/)) return "a";
  if (has(/\b(encoder|rotary) b\b/)) return "b";
  if (has(/\brotary c\b/)) return "c";
  if (has(/\bencoder key\b/)) return "key";
  if (has(/\b(enable| en)\b/)) return "en";
  if (has(/\b(g0[1-4]|g[1-4])\b/)) return normalized.match(/\b(g0?[1-4])\b/)?.[1];
  if (has(/\b(motor|ir emitter|buzzer|led|gpio|pin)\b/)) return "pin";
  return undefined;
}

/**
 * @param {string} label
 * @param {string} [context]
 * @returns {{ aliases: string[]; primary: string } | undefined}
 */
function aliasesFor(label, context = "") {
  const combined = `${context} ${label}`.trim();
  let signal = signalFor(combined);
  const categories = categoriesFor(combined);
  if (signal === "pin" && categories.has("button")) signal = "1";
  if ((categories.has("sd") || categories.has("sdmmc")) && signal === "sck" && /\b(?:sclk|clk)\b/i.test(combined)) {
    signal = "clk";
  }
  if (!signal) return undefined;
  if (categories.size === 0) {
    if (["sda", "scl"].includes(signal)) categories.add("i2c");
    else if (["mosi", "miso", "sck"].includes(signal)) categories.add("spi");
    else if (["tx", "rx"].includes(signal)) categories.add("uart");
    else categories.add("gpio");
  }
  const aliases = new Set([...categories].map((category) => `${category}.${signal}`));
  for (const category of categories) {
    if (["display", "lora", "sd", "ethernet"].includes(category) && ["mosi", "miso", "sck"].includes(signal)) {
      aliases.add(`spi.${signal}`);
    }
    if (category === "qspi" && (["d0", "d1", "d2", "d3", "sck", "cs"].includes(signal))) {
      aliases.add(`display.${signal}`);
    }
    if (["touch", "sensor", "power", "rtc"].includes(category) && ["sda", "scl"].includes(signal)) {
      aliases.add(`i2c.${signal}`);
    }
    if (["gps", "rs485", "rs232"].includes(category) && ["tx", "rx"].includes(signal)) {
      aliases.add(`uart.${signal}`);
    }
    if (category === "audio" && ["bclk", "ws", "dout", "din", "mclk"].includes(signal)) {
      aliases.add(`i2s.${signal}`);
    }
    if (category === "mic" && ["sck", "ws", "data"].includes(signal)) {
      aliases.add(`pdm.${signal}`);
    }
  }
  if (categories.has("sdmmc")) aliases.add(`sd.${signal}`);
  if (categories.has("lora") && signal === "int") aliases.add("lora.dio1");
  if (categories.has("camera") && signal === "siod") aliases.add("camera.sda");
  if (categories.has("camera") && signal === "sioc") aliases.add("camera.scl");
  if (categories.has("camera") && /\bpwdn\s*[\/_-]\s*rst\b/i.test(combined)) {
    aliases.add("camera.pwdn");
    aliases.add("camera.reset");
  }
  if (categories.has("display") && signal === "en") aliases.add("display.power");
  if (categories.has("display") && signal === "power") aliases.add("display.en");
  if (categories.has("button") && signal === "1") aliases.add("button.boot");
  if (categories.has("button") && signal === "2") aliases.add("button.user");
  if (categories.has("battery") && signal === "voltage") aliases.add("battery.adc");
  const ordered = [...aliases].sort();
  return { aliases: ordered, primary: ordered[0] ?? `gpio.${signal}` };
}

/**
 * The fact key defines the signal; the source identifier can add a missing
 * peripheral category without overriding that signal (for example TX_AH_*).
 * @param {string} key
 * @param {string} rawLabel
 */
function aliasesForFact(key, rawLabel) {
  const base = aliasesFor(key);
  if (!base) return aliasesFor(`${key} ${rawLabel}`);
  const signal = base.primary.split(".").at(-1);
  if (!signal) return base;
  const aliases = new Set(base.aliases);
  for (const category of categoriesFor(rawLabel)) {
    const extra = aliasesFor(`${category} ${signal}`);
    for (const alias of extra?.aliases ?? []) aliases.add(alias);
  }
  return { aliases: [...aliases].sort(), primary: base.primary };
}

/**
 * @param {unknown[]} table
 * @returns {string}
 */
function inferSignalTableContext(table) {
  const labels = table.flatMap((row) => {
    if (!isRecord(row)) return [];
    const key = ["Signal", "Name", "Function"].find((candidate) => typeof row[candidate] === "string");
    return key ? [String(row[key]).toLowerCase()] : [];
  });
  const joined = labels.join(" ");
  if (labels.length <= 8 && /\bmosi\b/.test(joined) && /\b(?:sclk|sck)\b/.test(joined) && /\b(?:dc|bl)\b/.test(joined)) return "display";
  if (labels.length <= 4 && labels.some((label) => /\bsda\b/.test(label)) && labels.some((label) => /\bscl\b/.test(label))) return "i2c";
  if (labels.length <= 4 && labels.some((label) => /\btx\b/.test(label)) && labels.some((label) => /\brx\b/.test(label))) return "gps";
  return "";
}

/**
 * @param {string} text
 * @param {string} context
 * @param {string} path
 * @param {string} [variant]
 * @returns {OfficialSignal[]}
 */
function parseIoParenthetical(text, context, path, variant) {
  /** @type {OfficialSignal[]} */
  const facts = [];
  const slashPattern = /IO\s*(\d+)\s*\/\s*IO\s*(\d+)\s*\(([^/()]+)\/([^()]+)\)/gi;
  for (const match of text.matchAll(slashPattern)) {
    const pairs = [[match[1], match[3]], [match[2], match[4]]];
    for (const [pin, label] of pairs) {
      if (!pin || !label) continue;
      const semantic = aliasesFor(label, context);
      if (semantic) facts.push({ ...semantic, value: `GPIO${Number(pin)}`, raw_label: label.trim(), raw_value: `IO${pin}`, path, ...(variant ? { variant } : {}) });
    }
  }
  const rangePattern = /IO\s*(\d+)\s*[–-]\s*(?:IO\s*)?(\d+)\s*\(D(\d+)\s*[–-]\s*D(\d+)\)/gi;
  for (const match of text.matchAll(rangePattern)) {
    if (!match[1] || !match[2] || !match[3] || !match[4]) continue;
    const pinStart = Number(match[1]);
    const pinEnd = Number(match[2]);
    const dataStart = Number(match[3]);
    const dataEnd = Number(match[4]);
    const pinStep = pinStart <= pinEnd ? 1 : -1;
    const dataStep = dataStart <= dataEnd ? 1 : -1;
    const pins = Array.from({ length: Math.abs(pinStart - pinEnd) + 1 }, (_, index) => pinStart + index * pinStep);
    const data = Array.from({ length: Math.abs(dataStart - dataEnd) + 1 }, (_, index) => dataStart + index * dataStep);
    if (pins.length !== data.length) continue;
    pins.forEach((pin, index) => {
      const dataIndex = data[index];
      if (dataIndex === undefined) return;
      const label = `D${dataIndex}`;
      const semantic = aliasesFor(label, context);
      if (semantic) facts.push({ ...semantic, value: `GPIO${pin}`, raw_label: label, raw_value: `IO${pin}`, path, ...(variant ? { variant } : {}) });
    });
  }
  const singlePattern = /IO\s*(\d+)\s*\(([^)]+)\)/gi;
  for (const match of text.matchAll(singlePattern)) {
    const pin = match[1];
    const label = match[2];
    if (!pin || !label || label.includes("/") || /[–-]/.test(label)) continue;
    const semantic = aliasesFor(label, context);
    if (semantic) facts.push({ ...semantic, value: `GPIO${Number(pin)}`, raw_label: label.trim(), raw_value: `IO${pin}`, path, ...(variant ? { variant } : {}) });
  }
  if (facts.length === 0) {
    const bare = normalizeGpio(text);
    const semantic = aliasesFor(context);
    if (bare && semantic) facts.push({ ...semantic, value: bare, raw_label: context, raw_value: text, path, ...(variant ? { variant } : {}) });
  }
  return dedupeSignals(facts);
}

/**
 * @param {unknown} parsed
 * @returns {OfficialSignal[]}
 */
export function extractOfficialSignals(parsed) {
  if (!isRecord(parsed) || !Array.isArray(parsed.pinTables)) throw new Error("get_product_specs result lacks pinTables array");
  /** @type {OfficialSignal[]} */
  const facts = [];
  parsed.pinTables.forEach((table, tableIndex) => {
    if (!Array.isArray(table)) return;
    const tableContext = inferSignalTableContext(table);
    table.forEach((row, rowIndex) => {
      if (!isRecord(row)) return;
      const path = `pinTables[${tableIndex}][${rowIndex}]`;
      const labelKey = ["Signal", "Name", "Function"].find((key) => typeof row[key] === "string");
      if (labelKey && typeof row.GPIO === "string") {
        const value = normalizeGpio(row.GPIO);
        const semantic = aliasesFor(/** @type {string} */ (row[labelKey]), tableContext);
        if (value && semantic) facts.push({ ...semantic, value, raw_label: /** @type {string} */ (row[labelKey]), raw_value: row.GPIO, path });
        return;
      }
      /** @type {[string, string][]} */
      const entries = Object.entries(row).flatMap(([key, value]) => typeof value === "string" ? [[key, value]] : []);
      const pairedCells = entries[1];
      if (entries.length === 2 && pairedCells && (/\bIO\s*\d+.*\(/i.test(pairedCells[0]) || /\bIO\s*\d+.*\(/i.test(pairedCells[1]))) {
        const leftContext = entries[0]?.[0] ?? "";
        const rightContext = entries[0]?.[1] ?? "";
        facts.push(...parseIoParenthetical(pairedCells[0], leftContext, path));
        facts.push(...parseIoParenthetical(pairedCells[1], rightContext, path));
        return;
      }
      const [contextEntry, ...fields] = entries;
      const context = contextEntry?.[0] ?? "";
      const variant = contextEntry?.[1] ?? "";
      for (const [label, rawValue] of fields) {
        const value = normalizeGpio(rawValue);
        const semantic = aliasesFor(label, context);
        if (value && semantic) facts.push({ ...semantic, value, raw_label: `${context}.${label}`, raw_value: rawValue, path, ...(variant ? { variant } : {}) });
      }
    });
  });
  return dedupeSignals(facts);
}

/**
 * @param {Fact} fact
 * @returns {OursSignal[]}
 */
function extractFactSignals(fact) {
  if (fact.confidence === "unknown_with_sources" || fact.value === "unknown_with_sources") return [];
  const citation = {
    url: fact.source.path_or_url,
    line_range: fact.source.line_range ?? null,
    sha256: fact.source.hash,
  };
  /** @type {OursSignal[]} */
  const signals = [];
  const pattern = /(?:^|[,;:]\s*)([A-Za-z][A-Za-z0-9_./ -]{0,40}?)\s*=\s*((?:GPIO|IO)\s*\d+)/gi;
  for (const match of fact.value.matchAll(pattern)) {
    const rawLabel = match[1]?.trim();
    const rawValue = match[2];
    if (!rawLabel || !rawValue) continue;
    const semantic = aliasesForFact(fact.key, rawLabel);
    const value = normalizeGpio(rawValue);
    if (semantic && value) signals.push({ ...semantic, value, key: fact.key, raw_value: fact.value, citation });
  }
  if (signals.length === 0) {
    const value = normalizeGpio(fact.value);
    const semantic = aliasesFor(fact.key, fact.value.split("=")[0]);
    if (value && semantic) signals.push({ ...semantic, value, key: fact.key, raw_value: fact.value, citation });
  }
  return signals;
}

/**
 * @param {Fact[]} facts
 * @returns {OursSignal[]}
 */
export function extractOursSignals(facts) {
  /** @type {OursSignal[]} */
  const signals = [];
  for (const fact of facts) {
    for (const signal of extractFactSignals(fact)) {
      const duplicate = signals.some((current) => current.value === signal.value && aliasesOverlap(current.aliases, signal.aliases));
      if (!duplicate) signals.push(signal);
    }
  }
  return signals;
}

/**
 * @template {OfficialSignal | OursSignal} T
 * @param {T[]} signals
 * @returns {T[]}
 */
function dedupeSignals(signals) {
  const seen = new Set();
  return signals.filter((signal) => {
    const variant = "variant" in signal ? signal.variant ?? "" : "";
    const fingerprint = `${signal.aliases.join("|")}=${signal.value}@${variant}`;
    if (seen.has(fingerprint)) return false;
    seen.add(fingerprint);
    return true;
  });
}

/**
 * @param {string[]} left
 * @param {string[]} right
 */
function aliasesOverlap(left, right) {
  const rightSet = new Set(right);
  return left.some((alias) => rightSet.has(alias));
}

/**
 * @param {OursSignal[]} ours
 * @param {OfficialSignal[]} official
 */
export function compareSignals(ours, official) {
  const comparisons = ours.map((oursSignal) => {
    const candidates = official.filter((officialSignal) => aliasesOverlap(oursSignal.aliases, officialSignal.aliases));
    if (candidates.length === 0) {
      return { status: "official-missing", ours: oursSignal, official_candidates: [] };
    }
    const agreeing = candidates.filter((candidate) => candidate.value === oursSignal.value);
    return {
      status: agreeing.length > 0 ? "agree" : "disagree",
      ours: oursSignal,
      official_candidates: candidates.map(({ primary, value, raw_label, raw_value, path, variant }) => ({
        signal: primary, value, raw_label, raw_value, path, ...(variant ? { variant } : {}),
      })),
    };
  });
  /** @type {Map<string, OfficialSignal[]>} */
  const officialGroups = new Map();
  for (const signal of official) {
    const key = signal.primary;
    const current = officialGroups.get(key) ?? [];
    current.push(signal);
    officialGroups.set(key, current);
  }
  const oursMissing = [...officialGroups.entries()]
    .filter(([, signals]) => !ours.some((oursSignal) => signals.some((officialSignal) => aliasesOverlap(oursSignal.aliases, officialSignal.aliases))))
    .map(([signal, signals]) => ({
      signal,
      values: [...new Set(signals.map((item) => item.value))].sort(),
      examples: signals.slice(0, 3).map(({ raw_label, raw_value, path, variant }) => ({ raw_label, raw_value, path, ...(variant ? { variant } : {}) })),
    }));
  const counts = {
    agree: comparisons.filter((row) => row.status === "agree").length,
    disagree: comparisons.filter((row) => row.status === "disagree").length,
    official_missing: comparisons.filter((row) => row.status === "official-missing").length,
    ours_missing: oursMissing.length,
  };
  return { comparisons, ours_missing: oursMissing, counts };
}

/**
 * @param {Fact[]} facts
 */
export function measureOursProvenance(facts) {
  const rows = facts.map((fact) => ({
    key: fact.key,
    has_url: /^https?:\/\//.test(fact.source.path_or_url),
    has_line_range: typeof fact.source.line_range === "string" && fact.source.line_range.length > 0,
    has_sha256: SHA256_RE.test(fact.source.hash),
  }));
  const count = rows.length;
  const url = rows.filter((row) => row.has_url).length;
  const lineRange = rows.filter((row) => row.has_line_range).length;
  const sha256 = rows.filter((row) => row.has_sha256).length;
  const complete = rows.filter((row) => row.has_url && row.has_line_range && row.has_sha256).length;
  return {
    facts: count,
    url,
    line_range: lineRange,
    sha256,
    url_line_range_sha256: complete,
    all_have_url: url === count,
    all_have_line_range: lineRange === count,
    all_have_sha256: sha256 === count,
    all_have_url_line_range_sha256: complete === count,
  };
}

/**
 * @param {unknown} parsed
 */
export function measureOfficialProvenance(parsed) {
  if (!isRecord(parsed)) return { has_provenance: false, has_per_fact_provenance: false, observed_metadata_links: [] };
  const metadataLinks = [];
  if (typeof parsed.shopLink === "string" && parsed.shopLink) {
    metadataLinks.push({ field: "shopLink", value: parsed.shopLink, kind: "product-shop-link-not-fact-citation" });
  }
  const factRows = [
    ...(Array.isArray(parsed.parameters) ? parsed.parameters.flatMap((row) => isRecord(row) ? [row] : []) : []),
    ...(Array.isArray(parsed.pinTables) ? parsed.pinTables.flatMap((table) => Array.isArray(table) ? table.filter(isRecord) : []) : []),
  ];
  const provenanceKey = /^(source|sources|citation|citations|line_range|sha256|source_url)$/i;
  const hasPerFact = factRows.some((row) => Object.keys(row).some((key) => provenanceKey.test(key)));
  return {
    has_provenance: hasPerFact,
    has_per_fact_provenance: hasPerFact,
    observed_metadata_links: metadataLinks,
  };
}

/**
 * @param {string} text
 */
export function sha256(text) {
  return `sha256:${createHash("sha256").update(text, "utf8").digest("hex")}`;
}

/**
 * @param {unknown} parsed
 */
export function officialStructuredFactCount(parsed) {
  if (!isRecord(parsed)) return 0;
  const parameters = Array.isArray(parsed.parameters) ? parsed.parameters.filter(isRecord).length : 0;
  const features = Array.isArray(parsed.keyFeatures) ? parsed.keyFeatures.filter((value) => typeof value === "string").length : 0;
  const pins = extractOfficialSignals(parsed).length;
  return parameters + features + pins;
}
