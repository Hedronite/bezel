#!/usr/bin/env node
/**
 * Unit / smoke for jev-router SPIKE pieces. No live Jev, no TYPESAFE_API_KEY.
 */

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { runCheck } from "./check.mjs";
import { availableCapabilities, diffPresent, gatherDiff, gatherFacts } from "./facts.mjs";
import { clipDigest, decideLoopStop, missingKeyLoop, shouldExecAgent } from "./loop-stop.mjs";
import {
  applyCheckThresholds,
  applyLoopStopThresholds,
  applyRoutingThresholds,
  CHECK_POLICY,
  LOOP_STOP_POLICY,
  ROUTING_POLICY,
} from "./policy.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const skipDiff = join(here, "testdata", "skip-marker.diff");
const assertDiff = join(here, "testdata", "assertions-removed.diff");
const emptyDiff = join(here, "testdata", "empty.diff");

function test(name, fn) {
  return { name, fn };
}

const cases = [
  test("routing policy versions and numbers", () => {
    assert.equal(ROUTING_POLICY.version, "omapi-route-workflow-policy@1");
    assert.equal(ROUTING_POLICY.minConfidence, 0.6);
    assert.equal(ROUTING_POLICY.minProbability, 0.55);
    assert.equal(ROUTING_POLICY.minMargin, 0.15);
  }),

  test("applyRoutingThresholds selects a confident available pick", () => {
    const decision = applyRoutingThresholds(
      {
        choice: "check",
        confidence: 0.8,
        probabilities: { check: 0.7, review: 0.2, cannot_tell: 0.1 },
      },
      { check: true, review: true },
    );
    assert.equal(decision.outcome, "check");
    assert.equal(decision.reason, "selected");
  }),

  test("applyRoutingThresholds parks uncertain / unavailable as cannot_tell", () => {
    const uncertain = applyRoutingThresholds(
      {
        choice: "check",
        confidence: 0.4,
        probabilities: { check: 0.4, review: 0.35, cannot_tell: 0.25 },
      },
      { check: true, review: true },
    );
    assert.equal(uncertain.outcome, "cannot_tell");
    assert.equal(uncertain.reason, "model_uncertain");

    const unavailable = applyRoutingThresholds(
      {
        choice: "check",
        confidence: 0.9,
        probabilities: { check: 0.8, review: 0.1, cannot_tell: 0.1 },
      },
      { check: false, review: false },
    );
    assert.equal(unavailable.outcome, "cannot_tell");
    assert.equal(unavailable.reason, "unavailable");
  }),

  test("available-gate: no diff → check/review unavailable", () => {
    assert.equal(diffPresent(""), false);
    assert.equal(diffPresent("   \n"), false);
    const { capabilities, unavailable } = availableCapabilities({ diffPresent: false });
    assert.equal(capabilities.check, false);
    assert.equal(capabilities.review, false);
    assert.match(unavailable.check, /diff/);
  }),

  test("available-gate: diff present → check/review available", () => {
    const { capabilities } = availableCapabilities({ diffPresent: true });
    assert.equal(capabilities.check, true);
    assert.equal(capabilities.review, true);
  }),

  test("gatherDiff from file + facts", () => {
    const diff = gatherDiff({ diffFile: skipDiff });
    assert.equal(diff.present, true);
    assert.equal(diff.source, "file");
    const facts = gatherFacts({ diffFile: skipDiff });
    assert.equal(facts.diffPresent, true);
    assert.deepEqual(facts.available, ["check", "review"]);
    const empty = gatherFacts({ diffFile: emptyDiff });
    assert.equal(empty.diffPresent, false);
    assert.deepEqual(empty.available, []);
  }),

  test("applyCheckThresholds buckets concern", () => {
    assert.equal(CHECK_POLICY.version, "omapi-check-policy@1");
    const finding = applyCheckThresholds({
      concern: 0.85,
      kind: "test_safety",
      confidence: 0.8,
      probabilities: { test_safety: 0.7, none: 0.2, task_mismatch: 0.05, cannot_tell: 0.05 },
    });
    assert.equal(finding.bucket, "finding");
    assert.equal(finding.flag, "test_safety");

    const parked = applyCheckThresholds({
      concern: 0.5,
      kind: "task_mismatch",
      confidence: 0.8,
      probabilities: { task_mismatch: 0.6, none: 0.2, test_safety: 0.1, cannot_tell: 0.1 },
    });
    assert.equal(parked.bucket, "parked");
    assert.equal(parked.reason, "concern_in_band");

    const clear = applyCheckThresholds({
      concern: 0.1,
      kind: "none",
      confidence: 0.8,
      probabilities: { none: 0.8, test_safety: 0.1, task_mismatch: 0.05, cannot_tell: 0.05 },
    });
    assert.equal(clear.bucket, "clear");
  }),

  test("runCheck skip-marker fixture without judge: deterministic finding, not approval", async () => {
    const report = await runCheck({
      diffText: gatherDiff({ diffFile: skipDiff }).text,
      intent: "add skip",
      judge: null,
      missingKey: true,
      mode: "shadow",
    });
    assert.equal(report.approval, false);
    assert.equal(report.emptyFindingsAreNotApproval, true);
    assert.equal(report.workflow, "check");
    assert.ok(report.findings.some((row) => row.flag === "skip_marker_added"));
    assert.ok(report.notChecked.some((row) => /TYPESAFE_API_KEY unset/.test(row)));
    assert.ok(!JSON.stringify(report).toLowerCase().includes("approved"));
  }),

  test("runCheck assertions-removed fixture", async () => {
    const report = await runCheck({
      diffText: gatherDiff({ diffFile: assertDiff }).text,
      missingKey: true,
    });
    assert.ok(report.findings.some((row) => row.flag === "assertions_removed"));
    assert.equal(report.approval, false);
  }),

  test("runCheck empty diff is no_diff and not approval", async () => {
    const report = await runCheck({ diffText: "", missingKey: true });
    assert.equal(report.status, "no_diff");
    assert.equal(report.approval, false);
    assert.ok(report.notChecked.some((row) => /no git diff/.test(row)));
    assert.equal(report.findings.length, 0);
  }),

  test("runCheck with fake judge parks / finds via code thresholds", async () => {
    const report = await runCheck({
      diffText: gatherDiff({ diffFile: skipDiff }).text,
      missingKey: false,
      judge: async () => ({
        concern: 0.5,
        kind: "test_safety",
        confidence: 0.8,
        probabilities: { test_safety: 0.7, none: 0.1, task_mismatch: 0.1, cannot_tell: 0.1 },
      }),
    });
    assert.equal(report.status, "complete");
    assert.ok(report.parked.length >= 1);
    assert.equal(report.approval, false);
  }),

  test("shadow --check without key emits envelope (spawn, no SDK live call)", () => {
    const bin = process.env.JEV_ROUTER_BIN;
    if (!bin) {
      return;
    }
    const result = spawnSync(bin, ["--check", "--intent", "probe", "--diff-file", skipDiff], {
      env: { ...process.env, JEV_MODE: "shadow", TYPESAFE_API_KEY: "" },
      encoding: "utf8",
    });
    assert.equal(result.status, 0, result.stderr);
    const payload = JSON.parse(result.stdout.trim().split("\n").at(-1));
    assert.equal(payload.approval, false);
    assert.equal(payload.emptyFindingsAreNotApproval, true);
    assert.equal(payload.missingKey, true);
    assert.ok(payload.findings.some((row) => row.flag === "skip_marker_added"));
    assert.ok(!result.stdout.toLowerCase().includes("approved"));
    assert.ok(!result.stderr.toLowerCase().includes("approved"));
  }),

  test("shadow router without key continues unclassified when bin provided", () => {
    const bin = process.env.JEV_ROUTER_BIN;
    if (!bin) {
      return;
    }
    const result = spawnSync(bin, ["probe intent"], {
      env: { ...process.env, JEV_MODE: "shadow", JEV_BYPASS: "", TYPESAFE_API_KEY: "" },
      encoding: "utf8",
    });
    assert.equal(result.status, 0, result.stderr);
    const payload = JSON.parse(result.stdout.trim().split("\n").at(-1));
    assert.equal(payload.choice, "unclassified");
    assert.equal(payload.blocked, false);
    assert.equal(payload.missingKey, true);
    assert.equal(payload.workflow.shadow, true);
    assert.equal(typeof payload.facts.diffPresent, "boolean");
    assert.equal(payload.autoRetry, false);
    assert.equal(payload.autoPromote, false);
    assert.equal(payload.loop.reason, "missing_key");
    assert.equal(payload.loopStopPolicy, "omapi-loop-stop-policy@1");
  }),

  test("loop-stop policy versions and numbers", () => {
    assert.equal(LOOP_STOP_POLICY.version, "omapi-loop-stop-policy@1");
    assert.equal(LOOP_STOP_POLICY.minConfidence, 0.6);
    assert.equal(LOOP_STOP_POLICY.minProbability, 0.55);
    assert.equal(LOOP_STOP_POLICY.minMargin, 0.15);
    assert.equal(LOOP_STOP_POLICY.maxDigestChars, 4000);
  }),

  test("applyLoopStopThresholds selects confident continue/stop", () => {
    const cont = applyLoopStopThresholds({
      choice: "continue",
      confidence: 0.85,
      probabilities: { continue: 0.7, stop: 0.2, escalate: 0.1 },
    });
    assert.equal(cont.outcome, "continue");
    assert.equal(cont.reason, "selected");

    const stop = applyLoopStopThresholds({
      choice: "stop",
      confidence: 0.85,
      probabilities: { stop: 0.72, continue: 0.18, escalate: 0.1 },
    });
    assert.equal(stop.outcome, "stop");
    assert.equal(stop.reason, "selected");
  }),

  test("applyLoopStopThresholds: escalate is sticky (low conf still HITL)", () => {
    const sure = applyLoopStopThresholds({
      choice: "escalate",
      confidence: 0.9,
      probabilities: { escalate: 0.8, continue: 0.1, stop: 0.1 },
    });
    assert.equal(sure.outcome, "escalate");
    assert.equal(sure.reason, "selected");

    const unsure = applyLoopStopThresholds({
      choice: "escalate",
      confidence: 0.3,
      probabilities: { escalate: 0.4, continue: 0.35, stop: 0.25 },
    });
    assert.equal(unsure.outcome, "escalate");
    assert.equal(unsure.reason, "escalate_uncertain");
  }),

  test("applyLoopStopThresholds parks uncertain continue as cannot_tell", () => {
    const uncertain = applyLoopStopThresholds({
      choice: "continue",
      confidence: 0.4,
      probabilities: { continue: 0.4, stop: 0.35, escalate: 0.25 },
    });
    assert.equal(uncertain.outcome, "cannot_tell");
    assert.equal(uncertain.reason, "model_uncertain");
  }),

  test("decideLoopStop maps continue/stop/escalate onto gate + HITL", () => {
    const cont = decideLoopStop(
      {
        choice: "continue",
        confidence: 0.85,
        probabilities: { continue: 0.7, stop: 0.2, escalate: 0.1 },
      },
      { mode: "active", intent: "ship it", stepDigest: "step 1 ok" },
    );
    assert.equal(cont.choice, "continue");
    assert.equal(cont.gate, "auto");
    assert.equal(cont.blocked, false);
    assert.equal(cont.exec, true);
    assert.equal(cont.hitl, false);
    assert.equal(cont.autoRetry, false);
    assert.equal(cont.autoPromote, false);
    assert.equal(cont.stepDigest, "step 1 ok");

    const stop = decideLoopStop(
      {
        choice: "stop",
        confidence: 0.85,
        probabilities: { stop: 0.72, continue: 0.18, escalate: 0.1 },
      },
      { mode: "active", intent: "done" },
    );
    assert.equal(stop.choice, "stop");
    assert.equal(stop.gate, "hold");
    assert.equal(stop.blocked, true);
    assert.equal(stop.exec, false);

    const esc = decideLoopStop(
      {
        choice: "escalate",
        confidence: 0.85,
        probabilities: { escalate: 0.8, continue: 0.1, stop: 0.1 },
      },
      { mode: "active", intent: "need a human" },
    );
    assert.equal(esc.choice, "escalate");
    assert.equal(esc.gate, "hold");
    assert.equal(esc.hitl, true);
    assert.equal(esc.autoRetry, false);
    assert.equal(esc.autoPromote, false);
    assert.equal(esc.blocked, true);
    assert.equal(esc.exec, false);
    assert.equal(esc.loop.hitl, true);
  }),

  test("decideLoopStop: shadow uncertain does not block; active uncertain escalates HITL", () => {
    const answer = {
      choice: "continue",
      confidence: 0.3,
      probabilities: { continue: 0.4, stop: 0.3, escalate: 0.3 },
    };
    const shadow = decideLoopStop(answer, { mode: "shadow", intent: "maybe" });
    assert.equal(shadow.choice, "unclassified");
    assert.equal(shadow.gate, "auto");
    assert.equal(shadow.blocked, false);
    assert.equal(shadow.exec, true);
    assert.equal(shadow.hitl, false);

    const active = decideLoopStop(answer, { mode: "active", intent: "maybe" });
    assert.equal(active.choice, "escalate");
    assert.equal(active.gate, "hold");
    assert.equal(active.hitl, true);
    assert.equal(active.blocked, true);
    assert.equal(active.exec, false);
    assert.equal(active.autoRetry, false);
  }),

  test("shadow stop/escalate still exec; active stop/escalate do not", () => {
    const stopAns = {
      choice: "stop",
      confidence: 0.9,
      probabilities: { stop: 0.8, continue: 0.1, escalate: 0.1 },
    };
    const shadowStop = decideLoopStop(stopAns, { mode: "shadow" });
    assert.equal(shadowStop.choice, "stop");
    assert.equal(shadowStop.exec, true);
    assert.equal(shadowStop.blocked, false);

    const activeStop = decideLoopStop(stopAns, { mode: "active" });
    assert.equal(activeStop.exec, false);

    const escAns = {
      choice: "escalate",
      confidence: 0.9,
      probabilities: { escalate: 0.8, continue: 0.1, stop: 0.1 },
    };
    assert.equal(decideLoopStop(escAns, { mode: "shadow" }).exec, true);
    assert.equal(decideLoopStop(escAns, { mode: "active" }).exec, false);
  }),

  test("shouldExecAgent table: shadow always; active continue/auto only", () => {
    assert.equal(shouldExecAgent({ mode: "shadow", choice: "stop", gate: "hold", blocked: true }), true);
    assert.equal(shouldExecAgent({ mode: "shadow", choice: "escalate", gate: "hold" }), true);
    assert.equal(shouldExecAgent({ mode: "active", choice: "continue", gate: "auto", blocked: false }), true);
    assert.equal(shouldExecAgent({ mode: "active", choice: "unclassified", gate: "auto", blocked: false }), true);
    assert.equal(shouldExecAgent({ mode: "active", choice: "stop", gate: "hold" }), false);
    assert.equal(shouldExecAgent({ mode: "active", choice: "escalate", gate: "hold" }), false);
    assert.equal(shouldExecAgent({ mode: "active", choice: "hold_for_human", gate: "hold", blocked: false }), false);
  }),

  test("clipDigest honors maxDigestChars", () => {
    const long = "x".repeat(5000);
    assert.equal(clipDigest(long).length, 4000);
    assert.equal(clipDigest("short"), "short");
  }),

  test("missingKeyLoop: shadow unclassified; active escalate HITL", () => {
    const shadow = missingKeyLoop("shadow", "probe", "digest");
    assert.equal(shadow.choice, "unclassified");
    assert.equal(shadow.exec, true);
    assert.equal(shadow.loop.reason, "missing_key");
    assert.equal(shadow.autoPromote, false);

    const active = missingKeyLoop("active", "probe");
    assert.equal(active.choice, "escalate");
    assert.equal(active.gate, "hold");
    assert.equal(active.hitl, true);
    assert.equal(active.exec, false);
    assert.equal(active.autoRetry, false);
  }),

  test("shadow --loop-stop without key still unclassified (spawn)", () => {
    const bin = process.env.JEV_ROUTER_BIN;
    if (!bin) {
      return;
    }
    const result = spawnSync(bin, ["--loop-stop", "--intent", "probe", "--step-digest", "step one failed"], {
      env: { ...process.env, JEV_MODE: "shadow", JEV_BYPASS: "", TYPESAFE_API_KEY: "" },
      encoding: "utf8",
    });
    assert.equal(result.status, 0, result.stderr);
    const payload = JSON.parse(result.stdout.trim().split("\n").at(-1));
    assert.equal(payload.choice, "unclassified");
    assert.equal(payload.blocked, false);
    assert.equal(payload.exec, true);
    assert.equal(payload.autoRetry, false);
    assert.equal(payload.autoPromote, false);
    assert.equal(payload.stepDigest, "step one failed");
    assert.equal(payload.loopStopPolicy, "omapi-loop-stop-policy@1");
  }),

  test("--check workflow is unchanged when step digest is also passed", () => {
    const bin = process.env.JEV_ROUTER_BIN;
    if (!bin) {
      return;
    }
    const result = spawnSync(
      bin,
      ["--check", "--intent", "probe", "--diff-file", skipDiff, "--step-digest", "ignored-for-check"],
      {
        env: { ...process.env, JEV_MODE: "shadow", TYPESAFE_API_KEY: "" },
        encoding: "utf8",
      },
    );
    assert.equal(result.status, 0, result.stderr);
    const payload = JSON.parse(result.stdout.trim().split("\n").at(-1));
    assert.equal(payload.workflow, "check");
    assert.equal(payload.approval, false);
    assert.equal(payload.emptyFindingsAreNotApproval, true);
    assert.ok(payload.findings.some((row) => row.flag === "skip_marker_added"));
  }),
];

let failed = 0;
for (const { name, fn } of cases) {
  try {
    await fn();
    process.stderr.write(`ok  ${name}\n`);
  } catch (err) {
    failed += 1;
    process.stderr.write(`not ok  ${name}\n${err && err.stack ? err.stack : err}\n`);
  }
}

if (failed) {
  process.stderr.write(`${failed} failed\n`);
  process.exit(1);
}
process.stderr.write(`${cases.length} passed\n`);
