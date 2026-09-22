/**
 * Tiny always-on tool catalog + on-demand schema dump (PR-D).
 *
 * The catalog is an index of PRETOOL_CLASSES. Every policyId is one of the
 * three ids already in policy.mjs. This module does not construct a
 * TypeSafe client, does not add a policyId, and does not honor
 * JEV_PERMISSION_MODE (permission catalogs stay PR-B).
 *
 * Always-on: names, class, policyId. No JSON Schema.
 * On demand: schemaDumpCall(tool) returns one schema. That is not an allow.
 */

import {
  CHECK_POLICY,
  LOOP_STOP_POLICY,
  PRETOOL_CLASSES,
  ROUTING_POLICY,
  matchPretoolClass,
} from "./policy.mjs";

export const POLICY_IDS = [
  LOOP_STOP_POLICY.version,
  CHECK_POLICY.version,
  ROUTING_POLICY.version,
];

const POLICY_ID_SET = new Set(POLICY_IDS);

const SECRET_KEY = /api[_-]?key|secret|token|authorization|password|credential/i;

/** Shared input schemas. Dumped only by schemaDumpCall, never by tinyCatalog. */
const SCHEMAS = {
  web_search: {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    type: "object",
    additionalProperties: false,
    required: ["query"],
    properties: {
      query: { type: "string", description: "Search query." },
    },
  },
  web_fetch: {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    type: "object",
    additionalProperties: false,
    required: ["url"],
    properties: {
      url: { type: "string", description: "Absolute URL to fetch." },
    },
  },
  task: {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    type: "object",
    additionalProperties: false,
    required: ["prompt"],
    properties: {
      prompt: { type: "string", description: "Task for the subagent." },
      description: { type: "string", description: "Short label for the task." },
      subagent_type: { type: "string", description: "Subagent kind, when the host has one." },
    },
  },
  shell: {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    type: "object",
    additionalProperties: false,
    required: ["command"],
    properties: {
      command: {
        type: "string",
        description: "Shell command text. Reading this schema does not allow the command.",
      },
    },
  },
  write: {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    type: "object",
    additionalProperties: false,
    required: ["path", "contents"],
    properties: {
      path: { type: "string", description: "File path to write." },
      contents: { type: "string", description: "Full file contents." },
    },
  },
  edit: {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    type: "object",
    additionalProperties: false,
    required: ["path", "old_string", "new_string"],
    properties: {
      path: { type: "string", description: "File path to edit." },
      old_string: { type: "string", description: "Exact text to replace." },
      new_string: { type: "string", description: "Replacement text." },
    },
  },
  multi_edit: {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    type: "object",
    additionalProperties: false,
    required: ["path", "edits"],
    properties: {
      path: { type: "string", description: "File path to edit." },
      edits: {
        type: "array",
        items: {
          type: "object",
          additionalProperties: false,
          required: ["old_string", "new_string"],
          properties: {
            old_string: { type: "string" },
            new_string: { type: "string" },
          },
        },
      },
    },
  },
  mcp: {
    $schema: "https://json-schema.org/draft/2020-12/schema",
    type: "object",
    additionalProperties: false,
    required: ["server", "tool"],
    properties: {
      server: { type: "string", description: "MCP server id from the qualified tool name." },
      tool: { type: "string", description: "Tool name on that server." },
      arguments: { type: "object", description: "Tool arguments. Typed calls are a later change." },
    },
  },
};

const TOOL_SCHEMA_KEY = {
  web_search: "web_search",
  WebSearch: "web_search",
  web_fetch: "web_fetch",
  WebFetch: "web_fetch",
  spawn_subagent: "task",
  Task: "task",
  Bash: "shell",
  run_terminal_command: "shell",
  run_terminal_cmd: "shell",
  Write: "write",
  Edit: "edit",
  MultiEdit: "multi_edit",
  search_replace: "edit",
};

export function mappedToolNames() {
  return PRETOOL_CLASSES.flatMap((row) => row.tools);
}

/** Index only. Safe to put on every GateVerdict and in Jev state. */
export function tinyCatalog() {
  return {
    kind: "tiny",
    entries: PRETOOL_CLASSES.map((row) => {
      const entry = {
        class: row.id,
        policyId: row.policyId,
      };
      if (row.tools.length) entry.tools = row.tools.slice();
      if (row.pattern) entry.pattern = row.pattern;
      return entry;
    }),
  };
}

export function orphanPolicyIds(ids) {
  return [...ids].filter((id) => !POLICY_ID_SET.has(id));
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

export function fullSchema(toolName, row) {
  const key = row && row.id === "mcp" ? "mcp" : TOOL_SCHEMA_KEY[toolName];
  if (!key || !SCHEMAS[key]) return null;
  const schema = clone(SCHEMAS[key]);
  schema.title = String(toolName);
  return schema;
}

function dumpBase(toolName) {
  return {
    ok: false,
    call: "schema-dump",
    tool: toolName ? String(toolName) : null,
    found: false,
    class: null,
    policyId: null,
    schema: null,
    autoAllow: false,
    decision: "deny",
    reason: "unknown_tool",
  };
}

/**
 * On-demand full schema for one mapped tool.
 * Unknown tools and orphan policy ids deny. A hit defers — it does not allow.
 */
export function schemaDumpCall(toolName) {
  const name = String(toolName || "");
  const row = name ? matchPretoolClass(name) : null;
  if (!row) return dumpBase(name);
  if (!POLICY_ID_SET.has(row.policyId)) {
    return {
      ...dumpBase(name),
      class: row.id,
      policyId: row.policyId,
      reason: "orphan_policy",
    };
  }
  const schema = fullSchema(name, row);
  if (!schema) {
    return {
      ...dumpBase(name),
      found: false,
      class: row.id,
      policyId: row.policyId,
      reason: "schema_missing",
    };
  }
  const dumped = {
    ok: true,
    call: "schema-dump",
    tool: name,
    found: true,
    class: row.id,
    policyId: row.policyId,
    schema,
    autoAllow: false,
    decision: "defer",
    reason: "schema_dump",
  };
  if (row.id === "mcp") dumped.typedCall = false;
  return dumped;
}

function redactString(value) {
  const secret = process.env.TYPESAFE_API_KEY;
  if (typeof value !== "string") return value;
  if (!secret) return value;
  if (!value.includes(secret)) return value;
  return value.split(secret).join("[redacted]");
}

/** Drop secret-shaped keys and any copy of TYPESAFE_API_KEY. Does not add the catalog. */
export function scrubState(value, keyName = "") {
  if (keyName && SECRET_KEY.test(keyName)) return undefined;
  if (typeof value === "string") return redactString(value);
  if (value === null || typeof value !== "object") return value;
  if (Array.isArray(value)) return value.map((item) => scrubState(item));
  const out = {};
  for (const [key, inner] of Object.entries(value)) {
    if (SECRET_KEY.test(key)) continue;
    const next = scrubState(inner, key);
    if (next !== undefined) out[key] = next;
  }
  return out;
}

/**
 * Jev state for the existing systemOne call.
 * Tiny catalog only. Full schemas stay on schemaDumpCall.
 */
export function buildGateState(fields = {}) {
  const scrubbed = scrubState(fields) || {};
  delete scrubbed.catalog;
  delete scrubbed.schema;
  delete scrubbed.schemas;
  return { ...scrubbed, catalog: tinyCatalog() };
}
