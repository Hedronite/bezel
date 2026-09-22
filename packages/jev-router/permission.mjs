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
 * pick is still not permission. MCP stays ask until a honoring typed call
 * selects a read tool (JEV_TYPED_CALL_MODE=active).
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
    contentGap: "workflow pick is not a tool call; typed calls stay ask until honored",
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

  return {
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
  };
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

function writeFromFlags(surface, mode, { flags, modelLabel }) {
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
  return pack(surface, mode, {
    mapped: "writer",
    label: "ask",
    modelLabel: modelLabel ?? null,
    reason: flags.length > 0 ? "writer_parent" : "empty_findings_not_approval",
    codeDeny: false,
  });
}

function mcpFromRoute(surface, mode, { routingOutcome, typed }) {
  if (typed && typed.honor === true && typed.policyId === surface.policyId && typed.transport === "facet") {
    const mapped = typed.mapped === "continue" || typed.mapped === "stop" ? typed.mapped : "escalate";
    const label = mapped === "continue" ? "allow" : mapped === "stop" ? "deny" : "ask";
    return pack(surface, mode, {
      mapped,
      label,
      reason: typed.reason || "typed_call",
      codeDeny: typed.codeDeny === true,
    });
  }
  const outcome = routingOutcome ? String(routingOutcome) : "";
  const named = outcome && outcome !== "cannot_tell";
  return pack(surface, mode, {
    mapped: "escalate",
    label: "ask",
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
  routingOutcome = null,
  typed = null,
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
    return writeFromFlags(surface, mode, { flags: writeFlags, modelLabel: label });
  }
  return mcpFromRoute(surface, mode, { routingOutcome, typed });
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
