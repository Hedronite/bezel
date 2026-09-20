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
