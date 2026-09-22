/**
 * Typed Calls (PR-C). MCP / tool intent → best and top-X → one FACET tool_call.
 *
 * Same route-workflow policy id (omapi-route-workflow-policy@1). No new policyId.
 * No TypeSafe client in this module. The router asks `call` on the existing
 * systemOne. This module ranks, fills closed-set args, and builds the transport.
 *
 * Transport is a FACET v2.1.3 tool_call (OpDesc + GuardDecision). This module
 * does not speak another request protocol and does not initiate MCP. `initiated` is false.
 * The guard decision is made before any call. This process does not perform one.
 *
 * Host defaults (deterministic, tested):
 *   obvious read | write | filesystem | network | external → continue, no F454
 *   payment → deny (F454)
 *   missing or invalid effect → deny (F456)
 *   invalid function name → deny (F452), no OpDesc
 *   tool_expose lists those obvious effects; payment and unknown effects are omitted
 *
 * Code deny wins. A denied winner is not replaced by the next-best tool
 * (autoPromote is false). Uncertain, open args, empty catalog, and a missing
 * key stay ask, not allow.
 *
 * JEV_TYPED_CALL_MODE defaults to shadow. Only the literal "active" honors,
 * and only when a key is set. Shadow logs the call and does not block.
 * The router reads that variable; this module does not read the environment.
 */

import { createHash } from "node:crypto";
import { ROUTING_POLICY, applyRoutingThresholds } from "./policy.mjs";

export const FACET_VERSION = "2.1.3";
export const POLICY_VERSION = "1";
export const HOST_PROFILE_ID = "omapi-jev-router";
export const FACET_PROFILE = "hypervisor";
export const FACET_MODE = "pure";
export const CALL_TOP_DEFAULT = 3;
export const CALL_TOP_MAX = 8;
export const MAX_TOOLS = 64;
export const MAX_ARGS = 16;
export const MAX_OPTIONS = 32;
export const STATED_MIN = 0.5;

export const FACET_IDENT = /^[A-Za-z_][A-Za-z0-9_]*$/;
export const NAMESPACED_EFFECT = /^x\.[A-Za-z_][A-Za-z0-9_.-]*$/;
export const STANDARD_EFFECTS = ["read", "write", "external", "payment", "filesystem", "network"];
export const DEFAULT_DENY_EFFECTS = ["payment"];
export const HOST_ALLOW_EFFECTS = ["read", "write", "filesystem", "network", "external"];

const STANDARD_EFFECT_SET = new Set(STANDARD_EFFECTS);
const SECRET_KEY = /^(typesafe_api_key|api_key|apikey|token|secret|password|authorization)$/i;

/**
 * Effective policy hashed into the FACET artifact. Obvious effects
 * continue. Payment stays denied. It is not an auto-allow of the hook.
 */
export const EFFECTIVE_POLICY = {
  tool_call: {
    allow: HOST_ALLOW_EFFECTS.slice(),
    deny: DEFAULT_DENY_EFFECTS.slice(),
  },
  tool_expose: {
    allow: HOST_ALLOW_EFFECTS.slice(),
  },
};

export function jcs(value) {
  return JSON.stringify(sortValue(value));
}

function sortValue(value) {
  if (value === null) return null;
  if (typeof value !== "object") return value;
  if (Array.isArray(value)) return value.map(sortValue);
  const out = {};
  for (const key of Object.keys(value).sort()) {
    if (value[key] === undefined) continue;
    out[key] = sortValue(value[key]);
  }
  return out;
}

export function sha256Text(text) {
  return createHash("sha256").update(text).digest("hex");
}

export function sha256Prefixed(text) {
  return `sha256:${sha256Text(text)}`;
}

export function clampTopX(value) {
  const n = Number(value);
  if (!Number.isInteger(n)) return CALL_TOP_DEFAULT;
  if (n < 1) return 1;
  if (n > CALL_TOP_MAX) return CALL_TOP_MAX;
  return n;
}

/** Only the literal "active" honors. Key absent forces shadow. */
export function effectiveTypedCallMode({ requested = "shadow", hasKey = false } = {}) {
  if (!hasKey) return "shadow";
  return String(requested ?? "shadow").toLowerCase() === "active" ? "active" : "shadow";
}

export function knownEffect(effect) {
  const value = typeof effect === "string" ? effect : "";
  if (STANDARD_EFFECT_SET.has(value)) return true;
  return NAMESPACED_EFFECT.test(value);
}

/**
 * tool_call guard for one declared effect.
 * Obvious read, write, filesystem, network, and external continue.
 * Payment and any other known effect stay denied.
 */
export function guardEffect(effect) {
  if (effect == null || effect === "") {
    return { allow: false, code: "F456", reason: "effect_missing" };
  }
  if (!knownEffect(effect)) {
    return { allow: false, code: "F456", reason: "effect_invalid" };
  }
  if (HOST_ALLOW_EFFECTS.includes(effect)) {
    return { allow: true, code: null, reason: "obvious_effect" };
  }
  if (effect === "payment") {
    return { allow: false, code: "F454", reason: "effect_deny" };
  }
  return { allow: false, code: "F454", reason: "effect_deny" };
}

function effectAllowsCall(effect) {
  return knownEffect(effect) && HOST_ALLOW_EFFECTS.includes(effect);
}

function clipText(value, max) {
  return String(value ?? "").slice(0, max);
}

function scrubString(text, secrets) {
  let out = String(text);
  for (const secret of secrets) {
    if (secret && secret.length >= 8) out = out.split(secret).join("[redacted]");
  }
  return out;
}

/**
 * Drop secret-shaped keys and replace planted secret values.
 * The API key is not a catalog field and not Jev state.
 */
export function scrubSecrets(value, secrets = []) {
  const list = (secrets || []).filter((secret) => typeof secret === "string" && secret.length >= 8);
  const walk = (current) => {
    if (typeof current === "string") return scrubString(current, list);
    if (current === null || typeof current !== "object") return current;
    if (Array.isArray(current)) return current.map(walk);
    const out = {};
    for (const [key, item] of Object.entries(current)) {
      if (key === "__proto__" || key === "prototype" || key === "constructor") continue;
      if (SECRET_KEY.test(key)) continue;
      out[key] = walk(item);
    }
    return out;
  };
  return walk(value);
}

function normalizeOptions(raw) {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const options = Object.create(null);
  let count = 0;
  for (const [key, value] of Object.entries(raw)) {
    if (count >= MAX_OPTIONS) break;
    if (!key || key === "__proto__" || key === "constructor" || key === "prototype") continue;
    if (typeof value !== "string" || !value.trim()) continue;
    options[key] = clipText(value, 240);
    count += 1;
  }
  return count >= 2 ? options : null;
}

function normalizeArgs(raw) {
  if (!Array.isArray(raw)) return [];
  const args = [];
  const seen = new Set();
  for (const row of raw) {
    if (args.length >= MAX_ARGS) break;
    if (!row || typeof row !== "object" || Array.isArray(row)) continue;
    const name = typeof row.name === "string" ? row.name : "";
    if (!FACET_IDENT.test(name) || seen.has(name)) continue;
    seen.add(name);
    const options = normalizeOptions(row.options);
    args.push({
      name,
      question: clipText(row.question || `Which ${name} does the intent ask for?`, 240),
      stated: clipText(row.stated || `Does the intent state a value for ${name}?`, 240),
      optional: row.optional === true,
      options,
      closed: Boolean(options),
    });
  }
  return args;
}

/**
 * Operator catalog → closed tool list. Order is the expose order.
 * A tool named cannot_tell is dropped; that label is the abstain choice.
 */
export function normalizeCatalog(raw) {
  const dropped = [];
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) {
    return { interface: "Mcp", tools: [], error: "catalog_invalid", dropped };
  }
  let interfaceName = "Mcp";
  if (typeof raw.interface === "string" && FACET_IDENT.test(raw.interface)) {
    interfaceName = raw.interface;
  } else if (raw.interface != null && raw.interface !== "") {
    dropped.push("interface");
  }
  const toolsIn = Array.isArray(raw.tools) ? raw.tools : [];
  const tools = [];
  const seen = new Set();
  for (const row of toolsIn) {
    if (tools.length >= MAX_TOOLS) {
      dropped.push("truncated");
      break;
    }
    if (!row || typeof row !== "object" || Array.isArray(row)) {
      dropped.push("bad_tool");
      continue;
    }
    const name = typeof row.name === "string" ? row.name : "";
    if (!name || name === "cannot_tell" || seen.has(name)) {
      dropped.push(name || "unnamed");
      continue;
    }
    seen.add(name);
    tools.push({
      name,
      description: clipText(row.description || name, 240),
      effect: typeof row.effect === "string" ? row.effect : "",
      args: normalizeArgs(row.args),
      validFn: FACET_IDENT.test(name),
    });
  }
  return { interface: interfaceName, tools, error: null, dropped };
}

export function emptyCatalog() {
  return { interface: "Mcp", tools: [], error: null, dropped: [] };
}

/** Names and effects only. Closed-set options live on the Choice, not in state. */
export function catalogState(catalog) {
  return (catalog && catalog.tools ? catalog.tools : []).map((tool) => ({
    name: tool.name,
    description: tool.description,
    effect: tool.effect || null,
  }));
}

/**
 * Questions for the router's existing systemOne. `choice` and `noul` are the
 * SDK helpers the router already imported.
 */
export function buildCallQuestions(choice, noul, catalog) {
  const criteria = { cannot_tell: "None of the listed tools fit the intent." };
  for (const tool of catalog.tools) {
    criteria[tool.name] = tool.description || tool.name;
  }
  const questions = {
    call: choice(
      "Which listed tool should handle `intent`? cannot_tell if none fit. Never invent a tool name. Never treat this as permission to run a side effect.",
      criteria,
    ),
  };
  for (const tool of catalog.tools) {
    for (const arg of tool.args) {
      if (!arg.closed) continue;
      const key = `${tool.name}.${arg.name}`;
      questions[key] = choice(arg.question, arg.options);
      if (arg.optional) questions[`${key}?`] = noul(arg.stated);
    }
  }
  return questions;
}

function catalogSource(catalog) {
  const lines = [`interface ${catalog.interface}`];
  for (const tool of catalog.tools) {
    lines.push(`fn ${tool.name} effect=${tool.effect || ""}`);
  }
  return lines.join("\n");
}

function metadataFor(catalog) {
  const documentHash = sha256Prefixed(catalogSource(catalog));
  const policyHash = sha256Prefixed(jcs({ policy_version: POLICY_VERSION, policy: EFFECTIVE_POLICY }));
  return {
    facet_version: FACET_VERSION,
    profile: FACET_PROFILE,
    mode: FACET_MODE,
    host_profile_id: HOST_PROFILE_ID,
    document_hash: documentHash,
    policy_hash: policyHash,
    policy_version: POLICY_VERSION,
    budget_units: 0,
    target_provider_id: HOST_PROFILE_ID,
  };
}

function exposeInputHash(interfaceName) {
  return sha256Prefixed(
    jcs({
      interface: interfaceName,
      host_profile_id: HOST_PROFILE_ID,
      facet_version: FACET_VERSION,
    }),
  );
}

function callInputHash(interfaceName, fn, args) {
  return sha256Prefixed(
    jcs({
      interface: interfaceName,
      fn,
      args,
      host_profile_id: HOST_PROFILE_ID,
      facet_version: FACET_VERSION,
    }),
  );
}

function guardEvent({ seq, op, name, effectClass, decision, inputHash }) {
  return {
    seq,
    op,
    name,
    effect_class: effectClass,
    mode: FACET_MODE,
    decision,
    policy_rule_id: null,
    input_hash: inputHash,
  };
}

/**
 * Recompute the Appendix F hash chain. Head is sha256 of H0 when there are
 * no events, otherwise sha256 of the last event link.
 */
export function facetHead(meta, events) {
  let hex = sha256Text(
    jcs({
      facet_version: meta.facet_version,
      host_profile_id: meta.host_profile_id,
      document_hash: meta.document_hash,
      policy_hash: meta.policy_hash,
      policy_version: meta.policy_version,
      profile: meta.profile,
      mode: meta.mode,
    }),
  );
  for (const event of events) {
    hex = sha256Text(jcs({ prev: `sha256:${hex}`, event }));
  }
  return `sha256:${hex}`;
}

function qualified(interfaceName, fn) {
  return `${interfaceName}.${fn}`;
}

function exposeCatalog(catalog) {
  const events = [];
  const tools = [];
  let seq = 1;
  const exposeHash = exposeInputHash(catalog.interface);
  for (const tool of catalog.tools) {
    if (!tool.validFn) continue;
    const allow = effectAllowsCall(tool.effect);
    const effectClass = knownEffect(tool.effect) ? tool.effect : null;
    events.push(
      guardEvent({
        seq,
        op: "tool_expose",
        name: qualified(catalog.interface, tool.name),
        effectClass,
        decision: allow ? "allowed" : "denied",
        inputHash: exposeHash,
      }),
    );
    seq += 1;
    if (!allow) continue;
    const properties = {};
    const required = [];
    for (const arg of tool.args) {
      if (!arg.closed) continue;
      properties[arg.name] = { enum: Object.keys(arg.options) };
      if (!arg.optional) required.push(arg.name);
    }
    tools.push({
      name: qualified(catalog.interface, tool.name),
      description: tool.description,
      effect: tool.effect,
      parameters: {
        type: "object",
        properties,
        required,
      },
    });
  }
  return { events, tools, nextSeq: seq };
}

function toolCallEvent(catalog, tool, args, decision, seq) {
  return guardEvent({
    seq,
    op: "tool_call",
    name: qualified(catalog.interface, tool.name),
    effectClass: knownEffect(tool.effect) ? tool.effect : null,
    decision,
    inputHash: callInputHash(catalog.interface, tool.name, args),
  });
}

function facetEnvelope(catalog, tool, args, event) {
  return {
    op: "tool_call",
    name: qualified(catalog.interface, tool.name),
    interface: catalog.interface,
    fn: tool.name,
    effect_class: knownEffect(tool.effect) ? tool.effect : null,
    mode: FACET_MODE,
    profile: FACET_PROFILE,
    args,
    initiated: false,
    guard: event,
  };
}

function artifactFor(meta, events) {
  return {
    metadata: {
      facet_version: meta.facet_version,
      host_profile_id: meta.host_profile_id,
      document_hash: meta.document_hash,
      policy_hash: meta.policy_hash,
      policy_version: meta.policy_version,
    },
    provenance: {
      events,
      hash_chain: {
        algo: "sha256",
        head: facetHead(meta, events),
      },
    },
    attestation: null,
  };
}

function noulOf(answer) {
  if (answer == null) return null;
  if (typeof answer === "number") return answer;
  if (typeof answer === "object" && typeof answer.noul === "number") return answer.noul;
  return null;
}

function fillArgs(tool, argAnswers) {
  const args = {};
  let weakest = null;
  for (const arg of tool.args) {
    if (!arg.closed) {
      if (!arg.optional) return { ok: false, reason: "open_arg", args: {}, weakest: arg.name };
      continue;
    }
    if (arg.optional) {
      const stated = noulOf(argAnswers && argAnswers[`${tool.name}.${arg.name}?`]);
      if (!(stated > STATED_MIN)) continue;
    }
    const ans = argAnswers && argAnswers[`${tool.name}.${arg.name}`];
    if (!ans || typeof ans !== "object") {
      return { ok: false, reason: "arg_uncertain", args: {}, weakest: arg.name };
    }
    const decision = applyRoutingThresholds(
      {
        choice: ans.choice,
        confidence: ans.confidence,
        probabilities: ans.probabilities,
      },
      Object.fromEntries(Object.keys(arg.options).map((key) => [key, true])),
    );
    if (decision.outcome === "cannot_tell" || !Object.prototype.hasOwnProperty.call(arg.options, decision.outcome)) {
      return { ok: false, reason: "arg_uncertain", args: {}, weakest: arg.name };
    }
    args[arg.name] = decision.outcome;
    if (!weakest || decision.confidence < weakest.confidence) {
      weakest = { name: arg.name, confidence: decision.confidence };
    }
  }
  return { ok: true, reason: "selected", args, weakest };
}

function rankTop(probabilities, catalog, topX) {
  const byName = new Map(catalog.tools.map((tool) => [tool.name, tool]));
  const rows = [];
  for (const [name, value] of Object.entries(probabilities || {})) {
    if (!byName.has(name)) continue;
    const probability = Number(value);
    rows.push({
      name,
      probability: Number.isFinite(probability) ? probability : 0,
      tool: byName.get(name),
    });
  }
  rows.sort((a, b) => b.probability - a.probability || (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  return rows.slice(0, topX).map((row, index) => ({
    rank: index + 1,
    name: row.name,
    probability: row.probability,
    effect: row.tool.effect || null,
    effectGuard: effectAllowsCall(row.tool.effect) && row.tool.validFn ? "allow" : "deny",
  }));
}

function bits(honor, mapped) {
  if (!honor) {
    return { choice: "unclassified", gate: "auto", blocked: false, exec: true, hitl: false };
  }
  if (mapped === "continue") {
    return { choice: "continue", gate: "auto", blocked: false, exec: true, hitl: false };
  }
  if (mapped === "ask") {
    return { choice: "unclassified", gate: "auto", blocked: false, exec: true, hitl: false };
  }
  if (mapped === "stop") {
    return { choice: "stop", gate: "hold", blocked: true, exec: false, hitl: false };
  }
  return { choice: "escalate", gate: "hold", blocked: true, exec: false, hitl: true };
}

function packCalls(fields) {
  const honor = fields.mode === "active";
  const mapped = fields.mapped ?? null;
  const flag = bits(honor, mapped);
  return {
    transport: "facet",
    facetVersion: FACET_VERSION,
    policyId: ROUTING_POLICY.version,
    policyVersion: POLICY_VERSION,
    mode: fields.mode,
    honor,
    shadow: !honor,
    ...flag,
    label: fields.label ?? null,
    mapped,
    reason: fields.reason,
    code: fields.code ?? null,
    codeDeny: fields.codeDeny === true,
    skipped: fields.skipped === true,
    missingKey: fields.missingKey === true,
    autoAllow: false,
    autoPromote: false,
    initiated: false,
    ...(fields.question != null ? { question: fields.question } : {}),
    ...(fields.hold != null ? { hold: fields.hold } : {}),
    topX: fields.topX,
    selected: fields.selected ?? null,
    best: fields.best ?? null,
    top: fields.top ?? [],
    facet: fields.facet ?? null,
    canonical: fields.canonical ?? null,
    artifact: fields.artifact ?? null,
  };
}

function withCatalog(catalog, mode, topX, fields) {
  const meta = metadataFor(catalog);
  const exposed = exposeCatalog(catalog);
  const events = exposed.events.slice();
  if (fields.event) events.push(fields.event);
  return packCalls({
    mode,
    topX,
    ...fields,
    canonical: {
      metadata: meta,
      tools: exposed.tools,
      messages: [],
    },
    artifact: artifactFor(meta, events),
  });
}

/**
 * Rank a Choice over the catalog and build the FACET tool_call.
 * Returns null only when the caller should omit the field. This function
 * always returns a calls object.
 */
function agentQuestion(decision) {
  const bars = [];
  if (decision.confidence < ROUTING_POLICY.minConfidence) bars.push(`confidence ${ROUTING_POLICY.minConfidence}`);
  if (decision.selectedProb < ROUTING_POLICY.minProbability) bars.push(`probability ${ROUTING_POLICY.minProbability}`);
  if (decision.margin < ROUTING_POLICY.minMargin) bars.push(`margin ${ROUTING_POLICY.minMargin}`);
  const named = bars.length ? bars.join(", ") : "confidence 0.6, probability 0.55, or margin 0.15";
  return `Ask the agent: ${named} not met.`;
}

export function decideTypedCalls({
  catalog = null,
  catalogError = null,
  requestedMode = "shadow",
  hasKey = false,
  topX = CALL_TOP_DEFAULT,
  answer = null,
  argAnswers = null,
  skipped = false,
  failed = false,
} = {}) {
  const mode = effectiveTypedCallMode({ requested: requestedMode, hasKey });
  const limit = clampTopX(topX);
  const source = catalog && Array.isArray(catalog.tools) ? catalog : emptyCatalog();

  if (skipped) {
    return packCalls({
      mode: "shadow",
      topX: limit,
      reason: "bypass",
      skipped: true,
      label: null,
      mapped: null,
    });
  }

  if (catalogError) {
    return packCalls({
      mode,
      topX: limit,
      reason: catalogError === "catalog_invalid" ? "catalog_invalid" : "catalog_unreadable",
      label: "ask",
      mapped: "escalate",
    });
  }

  if (!source.tools.length) {
    return packCalls({
      mode,
      topX: limit,
      reason: "no_catalog",
      label: "ask",
      mapped: "escalate",
    });
  }

  if (!hasKey) {
    return withCatalog(source, "shadow", limit, {
      reason: "missing_key",
      missingKey: true,
      label: "ask",
      mapped: "escalate",
    });
  }

  if (failed || !answer) {
    return withCatalog(source, mode, limit, {
      reason: "judge_failed",
      label: "ask",
      mapped: "escalate",
    });
  }

  const capabilities = {};
  for (const tool of source.tools) capabilities[tool.name] = true;
  const decision = applyRoutingThresholds(answer, capabilities);
  const top = rankTop(answer.probabilities, source, limit);
  const selected = {
    name: decision.selected,
    outcome: decision.outcome,
    reason: decision.reason,
    confidence: decision.confidence,
    selectedProb: decision.selectedProb,
    margin: decision.margin,
    policy: decision.policy,
  };

  if (decision.outcome === "cannot_tell") {
    const unknown = decision.reason === "unavailable";
    const agent = decision.reason === "model_uncertain";
    const human = decision.reason === "cannot_tell";
    return withCatalog(source, mode, limit, {
      reason: unknown ? "unknown_tool" : decision.reason,
      codeDeny: unknown,
      label: unknown ? "deny" : "ask",
      mapped: unknown ? "stop" : agent ? "ask" : "escalate",
      selected,
      top,
      ...(agent ? { question: agentQuestion(decision) } : {}),
      ...(human
        ? { hold: "detail=cannot_tell question=Which tool should a human choose?" }
        : {}),
    });
  }

  const tool = source.tools.find((row) => row.name === decision.outcome);
  if (!tool || !tool.validFn) {
    return withCatalog(source, mode, limit, {
      reason: "invalid_fn",
      code: "F452",
      codeDeny: true,
      label: "deny",
      mapped: "stop",
      selected,
      top,
    });
  }

  const effect = guardEffect(tool.effect);
  if (!effect.allow) {
    const event = toolCallEvent(source, tool, {}, "denied", exposeCatalog(source).nextSeq);
    return withCatalog(source, mode, limit, {
      reason: effect.reason,
      code: effect.code,
      codeDeny: true,
      label: "deny",
      mapped: "stop",
      selected,
      top,
      event,
      facet: facetEnvelope(source, tool, {}, event),
    });
  }

  const filled = fillArgs(tool, argAnswers);
  if (!filled.ok) {
    return withCatalog(source, mode, limit, {
      reason: filled.reason,
      label: "ask",
      mapped: "escalate",
      selected,
      top,
    });
  }

  const event = toolCallEvent(source, tool, filled.args, "allowed", exposeCatalog(source).nextSeq);
  const confidence = filled.weakest ? Math.min(decision.confidence, filled.weakest.confidence) : decision.confidence;
  return withCatalog(source, mode, limit, {
    reason: "selected",
    label: "allow",
    mapped: "continue",
    selected: { ...selected, weakest: filled.weakest },
    top,
    event,
    facet: facetEnvelope(source, tool, filled.args, event),
    best: {
      name: tool.name,
      rank: (top.find((row) => row.name === tool.name) || {}).rank ?? null,
      probability: decision.selectedProb,
      confidence,
      effect: tool.effect,
      args: filled.args,
      weakest: filled.weakest,
      initiated: false,
    },
  });
}

export function callsBypass(topX = CALL_TOP_DEFAULT) {
  return decideTypedCalls({ skipped: true, topX });
}

/**
 * Attach calls onto a GateVerdict.
 * A honoring stop / escalate rewrites the parent. A continue does not clear
 * a parent stop. Shadow never rewrites.
 */
export function applyCallsVerdict(payload, calls) {
  if (!calls) return payload;
  const next = { ...payload, calls: { ...calls, autoAllow: false, initiated: false } };
  if (!calls.honor) return next;
  const blocks = calls.blocked === true || calls.choice === "stop" || calls.choice === "escalate";
  if (!blocks) return next;
  return {
    ...next,
    choice: calls.choice,
    gate: calls.gate || "hold",
    blocked: true,
    exec: false,
    hitl: calls.hitl === true,
  };
}
