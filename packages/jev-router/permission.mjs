/**
 * Permission Choice catalogs (PR-B). Shadow default.
 *
 * Wraps the existing loop-stop / writer (check) / route-workflow families.
 * Same TypeSafe client, same three policy ids. No orphan policyId.
 *
 * allow → continue, deny → stop, ask → escalate
 * (write ask stays on the writer parent instead of a new policy).
 *
 * A new Choice catalog exists only for shell: the command body is not a
 * loop-stop label. Write wraps the check writer. MCP wraps route-workflow.
 * Typed Calls (calls.mjs) are that same policy id, not a new one. A workflow
 * pick is still not permission. While typed-call mode is unset, a read-shaped
 * MCP call follows loop-stop. A mutating MCP call stays ask. A honoring typed
 * call (JEV_TYPED_CALL_MODE=active) is the permission surface for that read.
 *
 * JEV_PERMISSION_MODE defaults to shadow. `active` honors this surface only
 * after this surface is re-checked, and only when TYPESAFE_API_KEY is set.
 * Key absent forces shadow. JEV_MODE does not flip this surface.
 * Code deny wins over a model allow. Nothing here emits decision "allow".
 */

import { deterministicFlags, fileKind, parseUnifiedDiff } from "./check.mjs";
import {
  CHECK_POLICY,
  LOOP_STOP_POLICY,
  ROUTING_POLICY,
  applyLoopStopThresholds,
} from "./policy.mjs";

export const PERMISSION_LABELS = ["allow", "deny", "ask"];

export const PARENT_POLICY_IDS = [
  LOOP_STOP_POLICY.version,
  CHECK_POLICY.version,
  ROUTING_POLICY.version,
];

/** Deterministic write findings. A model allow cannot override these. */
export const WRITE_CODE_DENY_FLAGS = [
  "secret_path",
  "skip_marker_added",
  "assertions_removed",
  "test_file_deleted",
];

export const PERMISSION_SURFACES = [
  {
    id: "shell",
    policyId: LOOP_STOP_POLICY.version,
    parent: "loop-stop",
    catalog: "permission",
    newCatalog: true,
    contentGap: "shell command text is not a loop-stop label",
  },
  {
    id: "write",
    policyId: CHECK_POLICY.version,
    parent: "writer",
    catalog: "writer",
    newCatalog: false,
    contentGap: null,
  },
  {
    id: "mcp",
    policyId: ROUTING_POLICY.version,
    parent: "route-workflow",
    catalog: "route-workflow",
    newCatalog: false,
    contentGap: "a read follows loop-stop until typed-call mode is the permission; a mutating call stays ask",
  },
];

const PARENT_POLICY_SET = new Set(PARENT_POLICY_IDS);

export function permissionSurface(classId) {
  const key = String(classId || "");
  return PERMISSION_SURFACES.find((row) => row.id === key) || null;
}

/** Only the literal "active" honors. Unset, "shadow", "yes", "true" stay shadow. */
export function resolvePermissionMode(value) {
  return String(value ?? "shadow").toLowerCase() === "active" ? "active" : "shadow";
}

/**
 * Key absent → shadow, even if the operator asked for active.
 * This is the permission surface only. It does not read JEV_MODE.
 */
export function effectivePermissionMode({ requested = "shadow", hasKey = false } = {}) {
  if (!hasKey) return "shadow";
  return resolvePermissionMode(requested);
}

export function mapPermissionLabel(label, surface) {
  if (label === "allow") return "continue";
  if (label === "deny") return "stop";
  if (label === "ask") {
    if (surface && surface.parent === "writer") return "writer";
    return "escalate";
  }
  return null;
}

export function toolInputPresent(value) {
  return String(value || "").trim().length > 0;
}

export function clipToolInput(text, policy = LOOP_STOP_POLICY) {
  const max = Number(policy.maxDigestChars || 4000);
  return String(text || "").slice(0, max);
}

/**
 * Shell permission question for the existing systemOne call.
 * `choice` is the SDK helper the router already imported. This module does
 * not construct a client.
 */
export function shellPermissionQuestion(choice) {
  return choice(
    "Permission for this shell command only. allow continues, deny stops, ask escalates to a human. Unclear is ask. Never treat this as an auto-allow.",
    {
      allow: "Continue: the command is in scope for the intent.",
      deny: "Stop: do not run the command.",
      ask: "Escalate to a human. Not an auto-retry and not an allow.",
    },
  );
}

export function collectWriteFlags({ diffText = "", path = "" } = {}) {
  const flags = new Set();
  if (path && fileKind(path) === "secret") flags.add("secret_path");
  if (String(diffText || "").trim()) {
    for (const hunk of parseUnifiedDiff(diffText)) {
      for (const flag of deterministicFlags(hunk).flags) flags.add(flag);
    }
  }
  return [...flags];
}

function holdQuestion(reason, choice) {
  switch (reason) {
    case "secret_path":
      return "Is this secret path intentional?";
    case "skip_marker_added":
      return "Should this test stay skipped?";
    case "assertions_removed":
      return "Should these assertions be removed?";
    case "test_file_deleted":
      return "Should this test file be deleted?";
    case "empty_findings_not_approval":
      return "Is this write in scope to continue?";
    case "no_content":
      return "What command should run?";
    case "missing_key":
      return "Is the judge available for this action?";
    default:
      if (choice === "stop") return "Should this action stop?";
      return "Should a human review this before it continues?";
  }
}

/** Human hold suffix. The machine `reason` stays the flag or code. */
export function holdSuffix({ reason = "", choice = "" } = {}) {
  const detail = String(reason || choice || "hold").replace(/[\r\n=]/g, "_");
  return `detail=${detail} question=${holdQuestion(reason, choice)}`;
}

function holdOn(row) {
  const held = row.gate === "hold" || row.choice === "stop" || row.choice === "escalate";
  if (!held) return row;
  const hold = holdSuffix({ reason: row.reason, choice: row.choice });
  const detail = hold.slice("detail=".length, hold.indexOf(" question="));
  const question = hold.slice(hold.indexOf("question=") + "question=".length);
  return { ...row, detail, question, hold };
}

function pack(surface, mode, fields) {
  const honor = mode === "active";
  let mapped = fields.mapped ?? null;
  let choice = "unclassified";
  let gate = "auto";
  let blocked = false;
  let exec = true;
  let hitl = false;

  if (honor) {
    if (mapped === "continue") {
      choice = "continue";
      gate = "auto";
      blocked = false;
      exec = true;
      hitl = false;
    } else if (mapped === "stop") {
      choice = "stop";
      gate = "hold";
      blocked = true;
      exec = false;
      hitl = false;
    } else {
      if (mapped !== "writer") mapped = "escalate";
      choice = "escalate";
      gate = "hold";
      blocked = true;
      exec = false;
      hitl = true;
    }
  }

  return holdOn({
    surface: surface.id,
    policyId: surface.policyId,
    parent: surface.parent,
    catalog: surface.catalog,
    newCatalog: surface.newCatalog === true,
    mode,
    honor,
    shadow: !honor,
    blocked,
    exec,
    hitl,
    choice,
    gate,
    label: fields.label ?? null,
    modelLabel: fields.modelLabel ?? null,
    mapped,
    reason: fields.reason,
    codeDeny: fields.codeDeny === true,
    skipped: fields.skipped === true,
    missingKey: fields.missingKey === true,
    autoAllow: false,
  });
}

function parentProbabilities(probabilities) {
  const probs = probabilities || {};
  return {
    continue: Number(probs.allow ?? 0),
    stop: Number(probs.deny ?? 0),
    escalate: Number(probs.ask ?? 0),
  };
}

function shellFromLabel(surface, mode, { label, confidence, probabilities, contentPresent }) {
  const modelLabel = label ?? null;
  if (!contentPresent) {
    return pack(surface, mode, {
      mapped: "escalate",
      label: "ask",
      modelLabel,
      reason: "no_content",
      codeDeny: false,
    });
  }

  const parentLabel = mapPermissionLabel(label, surface);
  if (!parentLabel || parentLabel === "writer") {
    return pack(surface, mode, {
      mapped: mode === "active" ? "escalate" : null,
      label: mode === "active" ? "ask" : null,
      modelLabel,
      reason: "cannot_tell",
    });
  }

  const decision = applyLoopStopThresholds(
    {
      choice: parentLabel,
      confidence: Number(confidence ?? 0),
      probabilities: parentProbabilities(probabilities),
    },
    LOOP_STOP_POLICY,
  );

  if (decision.outcome === "continue" || decision.outcome === "stop" || decision.outcome === "escalate") {
    const mapped = decision.outcome;
    return pack(surface, mode, {
      mapped,
      label: mapped === "continue" ? "allow" : mapped === "stop" ? "deny" : "ask",
      modelLabel,
      reason: decision.reason,
    });
  }

  return pack(surface, mode, {
    mapped: mode === "active" ? "escalate" : null,
    label: mode === "active" ? "ask" : null,
    modelLabel,
    reason: decision.reason === "cannot_tell" ? "cannot_tell" : "model_uncertain",
  });
}

/** A missing or non-numeric concern is not under the park bar. */
function concernUnderPark(concern) {
  return typeof concern === "number" && Number.isFinite(concern) && concern < CHECK_POLICY.concernPark;
}

function writeFromFlags(surface, mode, { flags, modelLabel, concern }) {
  const denyFlag = WRITE_CODE_DENY_FLAGS.find((flag) => flags.includes(flag)) || null;
  if (denyFlag) {
    return pack(surface, mode, {
      mapped: "stop",
      label: "deny",
      modelLabel: modelLabel ?? null,
      reason: denyFlag,
      codeDeny: true,
    });
  }
  if (concernUnderPark(concern)) {
    return pack(surface, mode, {
      mapped: "continue",
      label: "allow",
      modelLabel: modelLabel ?? null,
      reason: "below_park",
      codeDeny: false,
    });
  }
  return pack(surface, mode, {
    mapped: "writer",
    label: "ask",
    modelLabel: modelLabel ?? null,
    reason: flags.length > 0 ? "writer_parent" : "empty_findings_not_approval",
    codeDeny: false,
  });
}

const MCP_READ_NAME = /(?:^|__)(?:search|list|get|read|fetch|find)(?:__|_|$)/i;
const MCP_MUTATE_NAME = /(?:^|__)(?:save|create|update|delete|write|set|remove|send|add|put|post)(?:__|_|$)/i;

function mcpToolName(toolName, typed) {
  if (toolName) return String(toolName);
  if (typed && typed.best && typed.best.name) return String(typed.best.name);
  return "";
}

function mcpEffect(effect, typed) {
  if (effect) return String(effect).toLowerCase();
  if (typed && typed.best && typed.best.effect) return String(typed.best.effect).toLowerCase();
  return "";
}

/** read, mutate, or unknown. Effect wins over the name. lapis__search is a read. */
function mcpKind({ toolName = "", effect = "", typed = null } = {}) {
  const name = mcpToolName(toolName, typed);
  const eff = mcpEffect(effect, typed);
  if (eff === "read") return "read";
  if (eff && eff !== "read") return "mutate";
  if (MCP_READ_NAME.test(name)) return "read";
  if (MCP_MUTATE_NAME.test(name)) return "mutate";
  return "unknown";
}

function mcpFromRoute(surface, mode, { routingOutcome, typed, toolName, effect, label, confidence, probabilities }) {
  if (typed && typed.honor === true && typed.policyId === surface.policyId && typed.transport === "facet") {
    const mapped = typed.mapped === "continue" || typed.mapped === "stop" ? typed.mapped : "escalate";
    const picked = mapped === "continue" ? "allow" : mapped === "stop" ? "deny" : "ask";
    return pack(surface, mode, {
      mapped,
      label: picked,
      modelLabel: label ?? null,
      reason: typed.reason || "typed_call",
      codeDeny: typed.codeDeny === true,
    });
  }
  const kind = mcpKind({ toolName, effect, typed });
  if (kind === "mutate") {
    return pack(surface, mode, {
      mapped: "escalate",
      label: "ask",
      modelLabel: label ?? null,
      reason: "mutating_mcp",
      codeDeny: false,
    });
  }
  if (kind === "read") {
    return shellFromLabel(surface, mode, {
      label,
      confidence,
      probabilities,
      contentPresent: true,
    });
  }
  const outcome = routingOutcome ? String(routingOutcome) : "";
  const named = outcome && outcome !== "cannot_tell";
  return pack(surface, mode, {
    mapped: "escalate",
    label: "ask",
    modelLabel: label ?? null,
    reason: named ? "workflow_is_not_permission" : "cannot_tell",
    codeDeny: false,
  });
}

/**
 * Permission verdict for one PreToolUse class.
 * Returns null for classes outside shell/write/mcp (no orphan policyId).
 * Does not read JEV_MODE. Does not auto-allow.
 */
export function decidePermission({
  classId,
  requestedMode = "shadow",
  hasKey = false,
  contentPresent = false,
  label = null,
  confidence = 0,
  probabilities = null,
  diffText = "",
  path = "",
  flags = null,
  concern = null,
  routingOutcome = null,
  typed = null,
  toolName = "",
  effect = "",
  failed = false,
} = {}) {
  const surface = permissionSurface(classId);
  if (!surface || !PARENT_POLICY_SET.has(surface.policyId)) return null;

  const writeFlags =
    surface.id === "write" ? (flags == null ? collectWriteFlags({ diffText, path }) : flags) : [];
  const denyFlag =
    surface.id === "write" ? WRITE_CODE_DENY_FLAGS.find((flag) => writeFlags.includes(flag)) || null : null;
  const mode = effectivePermissionMode({ requested: requestedMode, hasKey });

  if (!hasKey) {
    return pack(surface, "shadow", {
      mapped: denyFlag ? "stop" : null,
      label: denyFlag ? "deny" : null,
      modelLabel: label,
      reason: denyFlag || "missing_key",
      codeDeny: Boolean(denyFlag),
      missingKey: true,
    });
  }

  if (surface.id === "write" && denyFlag) {
    return pack(surface, mode, {
      mapped: "stop",
      label: "deny",
      modelLabel: label,
      reason: denyFlag,
      codeDeny: true,
    });
  }

  if (failed) {
    return pack(surface, mode, {
      mapped: mode === "active" ? "escalate" : null,
      label: mode === "active" ? "ask" : null,
      modelLabel: label,
      reason: "judge_failed",
    });
  }

  if (surface.id === "shell") {
    return shellFromLabel(surface, mode, { label, confidence, probabilities, contentPresent });
  }
  if (surface.id === "write") {
    return writeFromFlags(surface, mode, { flags: writeFlags, modelLabel: label, concern });
  }
  return mcpFromRoute(surface, mode, {
    routingOutcome,
    typed,
    toolName,
    effect,
    label,
    confidence,
    probabilities,
  });
}

export function permissionBypass(classId) {
  const surface = permissionSurface(classId);
  if (!surface) return null;
  return pack(surface, "shadow", {
    mapped: null,
    label: null,
    reason: "bypass",
    skipped: true,
  });
}

/**
 * Attach a permission verdict onto a loop-stop payload.
 * Honor deny/ask by rewriting choice to stop/escalate.
 * A permission continue does not clear a parent stop (no auto-allow).
 * Shadow permission never changes choice, gate, or blocked.
 */
export function applyPermissionVerdict(payload, permission) {
  if (!permission) return payload;
  const next = { ...payload, permission: { ...permission, autoAllow: false } };
  if (!permission.honor) return next;
  const permBlocks =
    permission.blocked === true || permission.choice === "stop" || permission.choice === "escalate";
  if (!permBlocks) return next;
  return {
    ...next,
    choice: permission.choice,
    gate: permission.gate || "hold",
    blocked: true,
    exec: false,
    hitl: permission.hitl === true,
  };
}
