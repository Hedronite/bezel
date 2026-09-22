/**
 * Versioned thresholds. Jev classifies; code decides.
 * Pattern source: Stanley (MIT) route-intent / check-task — reimplemented thin, not vendored.
 */

export const ROUTING_POLICY = {
  version: "omapi-route-workflow-policy@1",
  minConfidence: 0.6,
  minProbability: 0.55,
  minMargin: 0.15,
};

export const CHECK_POLICY = {
  version: "omapi-check-policy@1",
  maxHunks: 32,
  maxHunkChars: 4000,
  concernFinding: 0.7,
  concernPark: 0.4,
  kindMinConfidence: 0.6,
  kindMinProbability: 0.55,
};

/**
 * Loop-stop / escalate (Suraj #4 + #10). Sibling of the routing policy.
 * Jev classifies continue|stop|escalate; code applies thresholds.
 * Escalate is HITL — never auto-retry or auto-promote.
 */
export const LOOP_STOP_POLICY = {
  version: "omapi-loop-stop-policy@1",
  minConfidence: 0.6,
  minProbability: 0.55,
  minMargin: 0.15,
  maxDigestChars: 4000,
};

export const WORKFLOW_LABELS = ["check", "review", "cannot_tell"];
export const LOOP_STOP_LABELS = ["continue", "stop", "escalate"];
export const CHECK_KIND_LABELS = ["none", "test_safety", "task_mismatch", "cannot_tell"];

/**
 * JEV_BYPASS kill switch. Same spelling as packages/cursor-agent-jev.nix:
 * only "1" or "true". Anything else (unset, "0", "false", "yes") is not a bypass.
 * The wrap then execs cursor-agent. The Grok hook must defer, not emit allow.
 */
export function isJevBypass(value) {
  return value === "1" || value === "true";
}

/**
 * Shared exec table for a GateVerdict. Shadow always execs.
 * Active: stop/escalate do not; blocked does not; continue or gate=auto does.
 * This is not an auto-allow. Callers that sit in front of a permission prompt
 * must defer, not emit decision "allow".
 */
export function gateVerdictAllowsExec({ mode, choice, gate, blocked } = {}) {
  const m = String(mode || "shadow").toLowerCase();
  if (m !== "active") return true;
  const c = String(choice || "");
  const g = String(gate || "");
  const b = blocked === true || blocked === "true";
  if (c === "stop" || c === "escalate") return false;
  if (b) return false;
  return c === "continue" || g === "auto";
}

/**
 * Map a jev-router GateVerdict onto one Grok PreToolUse decision.
 * Code denies. Bypass wins over a stop-shaped payload (the wrap skips the router).
 * Non-deny is `defer` — never `allow`.
 *
 * `JEV_MODE` shadow defers unless `permission.honor` or `calls.honor` is set.
 * Permission honor is `JEV_PERMISSION_MODE=active` and a key. Calls honor is
 * `JEV_TYPED_CALL_MODE=active` and a key. A continue does not override a loop
 * stop. A stop/escalate on either honoring surface denies even when the loop
 * would exec. The decision is still defer or deny, never allow.
 */
export function pretoolHookDecision(verdict = {}) {
  if (verdict.bypass === true) {
    return { action: "defer", exitCode: 0, decision: "defer", reason: "bypass" };
  }

  const permission = verdict.permission && typeof verdict.permission === "object" ? verdict.permission : null;
  const permissionHonors = Boolean(
    permission && permission.honor === true && String(permission.mode || "").toLowerCase() === "active",
  );
  const calls = verdict.calls && typeof verdict.calls === "object" ? verdict.calls : null;
  const callsHonors = Boolean(calls && calls.honor === true && String(calls.mode || "").toLowerCase() === "active");
  const modeActive = String(verdict.mode || "shadow").toLowerCase() === "active";
  if (!modeActive && !permissionHonors && !callsHonors) {
    return { action: "defer", exitCode: 0, decision: "defer", reason: "shadow" };
  }

  const loopAllows = gateVerdictAllowsExec({ ...verdict, mode: "active" });
  const permBlocks = Boolean(
    permissionHonors &&
      (permission.blocked === true || permission.choice === "stop" || permission.choice === "escalate"),
  );
  const callsBlocks = Boolean(
    callsHonors && (calls.blocked === true || calls.choice === "stop" || calls.choice === "escalate"),
  );

  let choice = verdict.choice != null ? String(verdict.choice) : "unclassified";
  let gate = verdict.gate != null ? String(verdict.gate) : "hold";
  let blocked = verdict.blocked === true || verdict.blocked === "true";
  let allows = false;

  if (permBlocks || callsBlocks) {
    const blocker = permBlocks ? permission : calls;
    choice = blocker.choice != null ? String(blocker.choice) : "stop";
    gate = blocker.gate != null ? String(blocker.gate) : "hold";
    blocked = true;
    allows = false;
  } else if (!loopAllows) {
    allows = false;
  } else {
    if (permissionHonors && permission.choice) choice = String(permission.choice);
    if (permissionHonors && permission.gate) gate = String(permission.gate);
    blocked = false;
    allows = gateVerdictAllowsExec({ mode: "active", choice, gate, blocked });
  }

  if (allows) {
    return { action: "defer", exitCode: 0, decision: "defer", reason: "exec" };
  }

  const policyId =
    (verdict.pretool && verdict.pretool.policyId) ||
    (permBlocks && permission && permission.policyId) ||
    (callsBlocks && calls && calls.policyId) ||
    (permission && permission.policyId) ||
    "";
  const policy = policyId ? ` policy=${policyId}` : "";
  return {
    action: "deny",
    exitCode: 2,
    decision: "deny",
    reason: `jev choice=${choice} gate=${gate}${policy}`,
  };
}

/**
 * Grok Build PreToolUse tool class → existing omapi policyId / Choice family.
 * One map for the Grok hook (~/.grok/hooks/jev-omapi.json) and this router.
 * Not a new policy and not a second TypeSafe client.
 * Permission catalogs wrap these ids (packages/jev-router/permission.mjs).
 *
 * Shell / web / subagent stay on loop-stop (the exec gate).
 * Write stays on check (the diff/hunk gate).
 * MCP stays on route-workflow (the capability gate). Typed Calls rank tools
 * on that same id; they do not add a policy.
 *
 * MCP names are the qualified `server__tool` form Grok puts on the event
 * (e.g. linear__save_issue), not the `use_tool` dispatcher.
 * Shell includes both public spellings: hook alias `run_terminal_command`
 * and the shell tool id `run_terminal_cmd`.
 */
export const MCP_TOOL_PATTERN = "[A-Za-z0-9][A-Za-z0-9_.-]*__[A-Za-z0-9_.-]+";

export const PRETOOL_CLASSES = [
  {
    id: "web",
    policyId: LOOP_STOP_POLICY.version,
    choiceFamily: LOOP_STOP_LABELS,
    tools: ["web_search", "WebSearch", "web_fetch", "WebFetch"],
  },
  {
    id: "subagent",
    policyId: LOOP_STOP_POLICY.version,
    choiceFamily: LOOP_STOP_LABELS,
    tools: ["spawn_subagent", "Task"],
  },
  {
    id: "shell",
    policyId: LOOP_STOP_POLICY.version,
    choiceFamily: LOOP_STOP_LABELS,
    tools: ["Bash", "run_terminal_command", "run_terminal_cmd"],
  },
  {
    id: "write",
    policyId: CHECK_POLICY.version,
    choiceFamily: CHECK_KIND_LABELS,
    tools: ["Write", "Edit", "MultiEdit", "search_replace"],
  },
  {
    id: "mcp",
    policyId: ROUTING_POLICY.version,
    choiceFamily: WORKFLOW_LABELS,
    tools: [],
    pattern: MCP_TOOL_PATTERN,
  },
];

export const PRETOOL_MATCHER = PRETOOL_CLASSES.flatMap((row) => row.tools).concat(MCP_TOOL_PATTERN).join("|");

const MCP_TOOL_RE = new RegExp(`^${MCP_TOOL_PATTERN}$`);

export function pretoolClassById(id) {
  const key = String(id || "");
  return PRETOOL_CLASSES.find((row) => row.id === key) || null;
}

/** Full-string class match. Null when the tool is outside the PreToolUse map. */
export function matchPretoolClass(toolName) {
  const name = String(toolName || "");
  if (!name) return null;
  for (const row of PRETOOL_CLASSES) {
    if (row.tools.includes(name)) return row;
  }
  if (MCP_TOOL_RE.test(name)) return pretoolClassById("mcp");
  return null;
}

export function pretoolStamp({ toolName = "", toolClass = "" } = {}) {
  const name = String(toolName || "");
  const requested = String(toolClass || "");
  if (!name && !requested) return null;
  const byName = name ? matchPretoolClass(name) : null;
  const byClass = requested ? pretoolClassById(requested) : null;
  const row = byClass || byName;
  const mismatch = Boolean(byClass && byName && byClass.id !== byName.id);
  if (!row) {
    return {
      matched: false,
      class: null,
      policyId: null,
      choiceFamily: [],
      toolName: name || null,
      mismatch,
    };
  }
  return {
    matched: true,
    class: row.id,
    policyId: row.policyId,
    choiceFamily: row.choiceFamily.slice(),
    toolName: name || null,
    mismatch,
  };
}

/**
 * Apply routing thresholds to a Jev Choice answer.
 * Unavailable or uncertain picks become cannot_tell. Code, not the model, decides.
 */
export function applyRoutingThresholds(answer, capabilities, policy = ROUTING_POLICY) {
  const selected = answer && answer.choice != null ? String(answer.choice) : "cannot_tell";
  const probabilities = (answer && answer.probabilities) || {};
  const selectedProb = Number(probabilities[selected] ?? 0);
  const confidence = Number(answer && answer.confidence != null ? answer.confidence : 0);
  const alts = Object.entries(probabilities)
    .filter(([label]) => label !== selected)
    .map(([, value]) => Number(value ?? 0));
  const margin = selectedProb - (alts.length ? Math.max(...alts) : 0);

  const base = {
    policy: policy.version,
    selected,
    confidence,
    selectedProb,
    margin,
  };

  if (selected === "cannot_tell") {
    return { ...base, outcome: "cannot_tell", reason: "cannot_tell" };
  }
  if (capabilities[selected] !== true) {
    return { ...base, outcome: "cannot_tell", reason: "unavailable" };
  }
  if (
    confidence < policy.minConfidence ||
    selectedProb < policy.minProbability ||
    margin < policy.minMargin
  ) {
    return { ...base, outcome: "cannot_tell", reason: "model_uncertain" };
  }
  return { ...base, outcome: selected, reason: "selected" };
}

/**
 * Apply check-policy thresholds to one hunk judgment (noul + kind Choice).
 * Clear hunks are not findings. Empty findings are never approval.
 */
export function applyCheckThresholds(judgment, policy = CHECK_POLICY) {
  const concern = Number(judgment && judgment.concern != null ? judgment.concern : 0);
  const kind = judgment && judgment.kind ? String(judgment.kind) : "cannot_tell";
  const confidence = Number(judgment && judgment.confidence != null ? judgment.confidence : 0);
  const probabilities = (judgment && judgment.probabilities) || {};
  const kindProb = Number(probabilities[kind] ?? 0);

  if (kind === "cannot_tell" || confidence < policy.kindMinConfidence || kindProb < policy.kindMinProbability) {
    return { bucket: "parked", reason: "model_uncertain", flag: null, concern, kind };
  }
  if (concern >= policy.concernFinding) {
    const flag = kind === "none" ? "jev_concern" : kind;
    return { bucket: "finding", reason: "concern_above_threshold", flag, concern, kind };
  }
  if (concern >= policy.concernPark) {
    return { bucket: "parked", reason: "concern_in_band", flag: null, concern, kind };
  }
  return { bucket: "clear", reason: "below_park", flag: null, concern, kind };
}

function choiceStats(answer) {
  const selected = answer && answer.choice != null ? String(answer.choice) : "cannot_tell";
  const probabilities = (answer && answer.probabilities) || {};
  const selectedProb = Number(probabilities[selected] ?? 0);
  const confidence = Number(answer && answer.confidence != null ? answer.confidence : 0);
  const alts = Object.entries(probabilities)
    .filter(([label]) => label !== selected)
    .map(([, value]) => Number(value ?? 0));
  const margin = selectedProb - (alts.length ? Math.max(...alts) : 0);
  return { selected, probabilities, selectedProb, confidence, margin };
}

function meetsChoiceThresholds({ confidence, selectedProb, margin }, policy) {
  return (
    confidence >= policy.minConfidence &&
    selectedProb >= policy.minProbability &&
    margin >= policy.minMargin
  );
}

/**
 * Apply loop-stop thresholds to a Jev Choice over {continue, stop, escalate}.
 * Escalate is sticky: low confidence still HITL, never auto-continue.
 * Uncertain continue/stop become cannot_tell — code, not the model, decides.
 */
export function applyLoopStopThresholds(answer, policy = LOOP_STOP_POLICY) {
  const { selected, selectedProb, confidence, margin } = choiceStats(answer);
  const base = {
    policy: policy.version,
    selected,
    confidence,
    selectedProb,
    margin,
  };
  const confident = meetsChoiceThresholds({ confidence, selectedProb, margin }, policy);

  if (selected === "escalate") {
    return { ...base, outcome: "escalate", reason: confident ? "selected" : "escalate_uncertain" };
  }
  if (selected !== "continue" && selected !== "stop") {
    return { ...base, outcome: "cannot_tell", reason: "cannot_tell" };
  }
  if (!confident) {
    return { ...base, outcome: "cannot_tell", reason: "model_uncertain" };
  }
  return { ...base, outcome: selected, reason: "selected" };
}
