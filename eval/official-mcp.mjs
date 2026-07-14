#!/usr/bin/env node
// Minimal MCP-over-SSE client for the official LilyGO documentation server.
// Uses only Node's built-in fetch/Web Streams implementation.

import { pathToFileURL } from "node:url";

const DEFAULT_ENDPOINT = "https://lilygo-doc-mcp-production.up.railway.app";
const DEFAULT_PROTOCOL_VERSION = "2024-11-05";
const DEFAULT_TIMEOUT_MS = 30_000;
const SUPPORTED_TOOLS = new Set(["list_products", "get_product", "get_product_specs"]);

/** @typedef {{ event: string; data: string }} SseEvent */

/**
 * @param {string} baseUrl
 * @returns {string}
 */
function normalizeBaseUrl(baseUrl) {
  const url = new URL(baseUrl);
  url.pathname = url.pathname.replace(/\/$/, "");
  url.search = "";
  url.hash = "";
  return url.href.replace(/\/$/, "");
}

/**
 * @param {unknown} value
 * @returns {value is Record<string, unknown>}
 */
function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/**
 * @param {unknown} raw
 * @returns {Record<string, unknown>}
 */
function requireObject(raw) {
  if (!isRecord(raw)) throw new Error("tool arguments must be a JSON object");
  return raw;
}

export class McpSseClient {
  /** @type {string} */
  #baseUrl;
  /** @type {number} */
  #timeoutMs;
  /** @type {AbortController | undefined} */
  #controller;
  /** @type {ReadableStreamDefaultReader<Uint8Array> | undefined} */
  #reader;
  #decoder = new TextDecoder();
  /** @type {string} */
  #buffer = "";
  /** @type {string | undefined} */
  #messageUrl;
  /** @type {number} */
  #nextId = 1;
  /** @type {Map<number, Record<string, unknown>>} */
  #pending = new Map();
  /** @type {boolean} */
  #initialized = false;

  /**
   * @param {{ baseUrl?: string; timeoutMs?: number }} [options]
   */
  constructor(options = {}) {
    this.#baseUrl = normalizeBaseUrl(options.baseUrl ?? DEFAULT_ENDPOINT);
    this.#timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    if (!Number.isFinite(this.#timeoutMs) || this.#timeoutMs <= 0) {
      throw new Error(`timeoutMs must be positive, received ${this.#timeoutMs}`);
    }
  }

  /** @returns {Promise<void>} */
  async connect() {
    if (this.#initialized) return;
    this.#controller = new AbortController();
    const response = await this.#fetchWithContext(`${this.#baseUrl}/sse`, {
      headers: { accept: "text/event-stream" },
      signal: this.#controller.signal,
    }, "open SSE stream");
    if (!response.ok || !response.body) {
      throw new Error(`open SSE stream failed: HTTP ${response.status}`);
    }
    const contentType = response.headers.get("content-type") ?? "";
    if (!contentType.toLowerCase().includes("text/event-stream")) {
      throw new Error(`open SSE stream failed: expected text/event-stream, received ${contentType || "unknown"}`);
    }
    this.#reader = response.body.getReader();

    while (!this.#messageUrl) {
      const event = await this.#readEvent();
      if (event.event !== "endpoint") continue;
      if (!event.data) throw new Error("MCP endpoint event did not contain a message URL");
      this.#messageUrl = new URL(event.data, this.#baseUrl).href;
    }

    const initialize = await this.#request("initialize", {
      protocolVersion: DEFAULT_PROTOCOL_VERSION,
      capabilities: {},
      clientInfo: { name: "lilygo-skills-official-compare", version: "1.0.0" },
    });
    if (!isRecord(initialize.result)) {
      throw new Error("initialize returned no result object");
    }
    await this.#post({ jsonrpc: "2.0", method: "notifications/initialized" });
    this.#initialized = true;
  }

  /** @returns {Promise<void>} */
  async close() {
    this.#initialized = false;
    this.#controller?.abort();
    try {
      await this.#reader?.cancel();
    } catch {
      // Aborting an active fetch commonly makes cancel reject; connection is closed either way.
    }
    this.#reader = undefined;
    this.#controller = undefined;
    this.#messageUrl = undefined;
    this.#buffer = "";
    this.#pending.clear();
  }

  /**
   * @returns {Promise<Record<string, unknown>[]>}
   */
  async listTools() {
    await this.connect();
    const response = await this.#request("tools/list", {});
    const result = requireObject(response.result);
    if (!Array.isArray(result.tools) || !result.tools.every(isRecord)) {
      throw new Error("tools/list returned an invalid tools array");
    }
    return result.tools;
  }

  /**
   * @param {"list_products" | "get_product" | "get_product_specs"} name
   * @param {Record<string, unknown>} [args]
   * @returns {Promise<Record<string, unknown>>}
   */
  async callTool(name, args = {}) {
    if (!SUPPORTED_TOOLS.has(name)) {
      throw new Error(`unsupported official tool: ${name}`);
    }
    await this.connect();
    const response = await this.#request("tools/call", { name, arguments: requireObject(args) });
    const result = requireObject(response.result);
    if (result.isError === true) {
      throw new Error(`official tool ${name} returned isError=true: ${JSON.stringify(result.content ?? null)}`);
    }
    return result;
  }

  /**
   * @param {Record<string, unknown>} body
   * @returns {Promise<void>}
   */
  async #post(body) {
    if (!this.#messageUrl || !this.#controller) throw new Error("MCP client is not connected");
    const response = await this.#fetchWithContext(this.#messageUrl, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      signal: this.#controller.signal,
    }, `POST ${String(body.method ?? "MCP message")}`);
    if (!response.ok) {
      const detail = await response.text().catch(() => "");
      throw new Error(`POST ${String(body.method ?? "MCP message")} failed: HTTP ${response.status}${detail ? `: ${detail}` : ""}`);
    }
  }

  /**
   * @param {string} method
   * @param {Record<string, unknown>} params
   * @returns {Promise<Record<string, unknown>>}
   */
  async #request(method, params) {
    const id = this.#nextId++;
    await this.#post({ jsonrpc: "2.0", id, method, params });
    const cached = this.#pending.get(id);
    if (cached) {
      this.#pending.delete(id);
      return this.#unwrapResponse(method, cached);
    }
    while (true) {
      const event = await this.#readEvent();
      if (event.event !== "message" || !event.data) continue;
      /** @type {unknown} */
      let parsed;
      try {
        parsed = JSON.parse(event.data);
      } catch (error) {
        throw new Error(`parse MCP message failed: ${error instanceof Error ? error.message : String(error)}; raw=${event.data}`);
      }
      if (!isRecord(parsed) || typeof parsed.id !== "number") continue;
      if (parsed.id === id) return this.#unwrapResponse(method, parsed);
      this.#pending.set(parsed.id, parsed);
    }
  }

  /**
   * @param {string} method
   * @param {Record<string, unknown>} response
   * @returns {Record<string, unknown>}
   */
  #unwrapResponse(method, response) {
    if (response.error !== undefined) {
      throw new Error(`${method} returned JSON-RPC error: ${JSON.stringify(response.error)}`);
    }
    if (!("result" in response)) throw new Error(`${method} returned neither result nor error`);
    return response;
  }

  /** @returns {Promise<SseEvent>} */
  async #readEvent() {
    if (!this.#reader) throw new Error("MCP SSE reader is not open");
    while (true) {
      const boundary = this.#buffer.indexOf("\n\n");
      if (boundary >= 0) {
        const block = this.#buffer.slice(0, boundary);
        this.#buffer = this.#buffer.slice(boundary + 2);
        const event = parseSseBlock(block);
        if (event) return event;
        continue;
      }
      const timeout = setTimeout(() => this.#controller?.abort(), this.#timeoutMs);
      try {
        const chunk = await this.#reader.read();
        if (chunk.done) throw new Error("MCP SSE stream ended before a response arrived");
        this.#buffer += this.#decoder.decode(chunk.value, { stream: true }).replace(/\r\n?/g, "\n");
      } catch (error) {
        if (this.#controller?.signal.aborted) {
          throw new Error(`MCP SSE response timed out after ${this.#timeoutMs}ms`);
        }
        throw error;
      } finally {
        clearTimeout(timeout);
      }
    }
  }

  /**
   * @param {string} url
   * @param {RequestInit} init
   * @param {string} context
   * @returns {Promise<Response>}
   */
  async #fetchWithContext(url, init, context) {
    try {
      return await fetch(url, init);
    } catch (error) {
      throw new Error(`${context} failed: ${error instanceof Error ? error.message : String(error)}`);
    }
  }
}

/**
 * @param {string} block
 * @returns {SseEvent | undefined}
 */
export function parseSseBlock(block) {
  if (!block || block.startsWith(":")) return undefined;
  const lines = block.split("\n");
  const eventLine = lines.find((line) => line.startsWith("event:"));
  const data = lines
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice(5).replace(/^ /, ""))
    .join("\n");
  return { event: eventLine ? eventLine.slice(6).trim() : "message", data };
}

/**
 * @param {Record<string, unknown>} result
 * @returns {string[]}
 */
export function toolText(result) {
  if (!Array.isArray(result.content)) return [];
  return result.content
    .filter((item) => isRecord(item) && item.type === "text" && typeof item.text === "string")
    .map((item) => /** @type {string} */ (item.text));
}

async function main() {
  const [tool = "", rawArgs = "{}"] = process.argv.slice(2);
  if (!SUPPORTED_TOOLS.has(tool)) {
    process.stderr.write("usage: node eval/official-mcp.mjs <list_products|get_product|get_product_specs> '<json-args>'\n");
    process.exitCode = 2;
    return;
  }
  /** @type {unknown} */
  let args;
  try {
    args = JSON.parse(rawArgs);
  } catch (error) {
    throw new Error(`parse CLI JSON arguments failed: ${error instanceof Error ? error.message : String(error)}`);
  }
  const client = new McpSseClient({ baseUrl: process.env.LILYGO_OFFICIAL_MCP_URL });
  const started = performance.now();
  try {
    const result = await client.callTool(
      /** @type {"list_products" | "get_product" | "get_product_specs"} */ (tool),
      requireObject(args),
    );
    process.stdout.write(`${JSON.stringify({
      status: "PASS",
      tool,
      elapsed_ms: Math.round(performance.now() - started),
      result,
    }, null, 2)}\n`);
  } finally {
    await client.close();
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main().catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`);
    process.exitCode = 1;
  });
}
