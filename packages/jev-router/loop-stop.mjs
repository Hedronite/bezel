/**
 * Loop-stop / escalate envelope (Suraj #4 + #10).
 * Jev answers continue|stop|escalate; code maps to gate / exec / HITL.
 * Escalate is human-in-the-loop — not auto-retry, not auto-promote.
 */

import { LOOP_STOP_POLICY, applyLoopStopThresholds } from "./policy.mjs";

export function clipDigest(text, policy = LOOP_STOP_POLICY) {
  const max = Number(policy.maxDigestChars || 4000);
  return String(text || "").slice(0, max);
}

/**
 * Shadow always execs. Active: stop/escalate do not exec; continue/auto execs.
 */
export function shouldExecAgent({ mode, choice, gate, blocked } = {}) {
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
 * Map a loop-stop Choice answer + mode onto the router JSON contract.
 * Active + uncertain → explicit escalate (HITL, fail closed).
 * Shadow + uncertain → unclassified, does not block.
 */
export function decideLoopStop(answer, { mode = "shadow", intent = "", stepDigest = "" } = {}, policy = LOOP_STOP_POLICY) {
  const decision = applyLoopStopThresholds(answer, policy);
  const m = String(mode || "shadow").toLowerCase();
  const digest = clipDigest(stepDigest, policy);

  let choice;
  let gate;
  let reason = decision.reason;

  if (decision.outcome === "continue") {
    choice = "continue";
    gate = "auto";
  } else if (decision.outcome === "stop") {
    choice = "stop";
    gate = "hold";
  } else if (decision.outcome === "escalate") {
    choice = "escalate";
    gate = "hold";
  } else if (m === "active") {
    choice = "escalate";
    gate = "hold";
    reason = decision.reason === "cannot_tell" ? "cannot_tell" : "model_uncertain";
  } else {
    choice = "unclassified";
    gate = "auto";
  }

  const hitl = choice === "escalate";
  const blocked = m === "active" && (choice === "stop" || choice === "escalate" || gate !== "auto");
  const exec = shouldExecAgent({ mode: m, choice, gate, blocked });

  return {
    choice,
    gate,
    blocked,
    exec,
    hitl,
    autoRetry: false,
    autoPromote: false,
    intent,
    stepDigest: digest,
    loop: {
      policy: policy.version,
      selected: decision.selected,
      outcome: decision.outcome,
      reason,
      confidence: decision.confidence,
      selectedProb: decision.selectedProb,
      margin: decision.margin,
      hitl,
      autoRetry: false,
      autoPromote: false,
    },
  };
}

export function missingKeyLoop(mode, intent = "", stepDigest = "") {
  const m = String(mode || "shadow").toLowerCase();
  if (m === "active") {
    const decided = decideLoopStop({ choice: "cannot_tell" }, { mode: m, intent, stepDigest });
    decided.loop = { ...decided.loop, reason: "missing_key" };
    return decided;
  }
  const digest = clipDigest(stepDigest);
  return {
    choice: "unclassified",
    gate: "auto",
    blocked: false,
    exec: true,
    hitl: false,
    autoRetry: false,
    autoPromote: false,
    intent,
    stepDigest: digest,
    loop: {
      policy: LOOP_STOP_POLICY.version,
      selected: "unclassified",
      outcome: "cannot_tell",
      reason: "missing_key",
      hitl: false,
      autoRetry: false,
      autoPromote: false,
      shadow: true,
      blocked: false,
    },
  };
}
