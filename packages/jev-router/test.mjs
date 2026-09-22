#!/usr/bin/env node
/**
 * Unit / smoke for jev-router SPIKE pieces. No live Jev, no TYPESAFE_API_KEY.
 */

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  applyCallsVerdict,
  buildCallQuestions,
  callsBypass,
  clampTopX,
  decideTypedCalls,
  facetHead,
  guardEffect,
  normalizeCatalog,
  scrubSecrets,
} from "./calls.mjs";
import {
  POLICY_IDS,
  buildGateState,
  fullSchema,
  mappedToolNames,
  orphanPolicyIds,
  schemaDumpCall,
  tinyCatalog,
} from "./catalog.mjs";
import { runCheck } from "./check.mjs";
import { decideHook, harnessConfig, harnessModes, parseHookEvent, runSmoke } from "./harness.mjs";
import { availableCapabilities, diffPresent, gatherDiff, gatherFacts } from "./facts.mjs";
import { clipDigest, decideLoopStop, missingKeyLoop, shouldExecAgent } from "./loop-stop.mjs";
import {
  applyPermissionVerdict,
  collectWriteFlags,
  decidePermission,
  mapPermissionLabel,
  PARENT_POLICY_IDS,
  permissionBypass,
  permissionSurface,
  PERMISSION_SURFACES,
  resolvePermissionMode,
} from "./permission.mjs";
import {
  applyCheckThresholds,
  applyLoopStopThresholds,
  applyRoutingThresholds,
  CHECK_KIND_LABELS,
  CHECK_POLICY,
  isJevBypass,
  LOOP_STOP_LABELS,
  LOOP_STOP_POLICY,
  matchPretoolClass,
  MCP_TOOL_PATTERN,
  PRETOOL_CLASSES,
  PRETOOL_MATCHER,
  pretoolHookDecision,
  pretoolStamp,
  ROUTING_POLICY,
  WORKFLOW_LABELS,
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

  test("JEV_BYPASS is only 1 or true", () => {
    assert.equal(isJevBypass("1"), true);
    assert.equal(isJevBypass("true"), true);
    for (const value of ["", "0", "false", "yes", "TRUE", " 1", undefined]) {
      assert.equal(isJevBypass(value), false, String(value));
    }
  }),

  test("PreToolUse map stays on the three existing policyIds", () => {
    const ids = new Set(PRETOOL_CLASSES.map((row) => row.policyId));
    assert.deepEqual(
      [...ids].sort(),
      ["omapi-check-policy@1", "omapi-loop-stop-policy@1", "omapi-route-workflow-policy@1"].sort(),
    );
    assert.equal(PRETOOL_CLASSES.length, 5);
    const byId = Object.fromEntries(PRETOOL_CLASSES.map((row) => [row.id, row]));
    assert.equal(byId.shell.policyId, LOOP_STOP_POLICY.version);
    assert.deepEqual(byId.shell.choiceFamily, LOOP_STOP_LABELS);
    assert.equal(byId.web.policyId, LOOP_STOP_POLICY.version);
    assert.equal(byId.subagent.policyId, LOOP_STOP_POLICY.version);
    assert.equal(byId.write.policyId, CHECK_POLICY.version);
    assert.deepEqual(byId.write.choiceFamily, CHECK_KIND_LABELS);
    assert.equal(byId.mcp.policyId, ROUTING_POLICY.version);
    assert.deepEqual(byId.mcp.choiceFamily, WORKFLOW_LABELS);
    assert.equal(byId.mcp.pattern, MCP_TOOL_PATTERN);
  }),

  test("matcher covers existing hook tokens plus shell, write, and MCP", () => {
    const legacy = ["web_search", "WebFetch", "spawn_subagent", "Task"];
    const shell = ["Bash", "run_terminal_command", "run_terminal_cmd"];
    const write = ["Write", "Edit", "MultiEdit", "search_replace"];
    const mcp = ["linear__save_issue", "mcp__filesystem__read_file"];
    for (const name of [...legacy, ...shell, ...write, ...mcp]) {
      assert.ok(matchPretoolClass(name), name);
      assert.match(name, new RegExp(PRETOOL_MATCHER));
    }
    assert.equal(matchPretoolClass("web_search").id, "web");
    assert.equal(matchPretoolClass("WebFetch").id, "web");
    assert.equal(matchPretoolClass("web_fetch").id, "web");
    assert.equal(matchPretoolClass("Task").id, "subagent");
    assert.equal(matchPretoolClass("spawn_subagent").id, "subagent");
    assert.equal(matchPretoolClass("run_terminal_cmd").id, "shell");
    assert.equal(matchPretoolClass("search_replace").id, "write");
    assert.equal(matchPretoolClass("linear__save_issue").id, "mcp");
    assert.equal(matchPretoolClass("read_file"), null);
    assert.equal(matchPretoolClass("grep"), null);
    assert.equal(matchPretoolClass("use_tool"), null);
    assert.equal(matchPretoolClass("search_tool"), null);
    assert.equal(matchPretoolClass("todo_write"), null);
    assert.ok(PRETOOL_MATCHER.includes("WebFetch"));
    assert.ok(PRETOOL_MATCHER.includes("run_terminal_command"));
    assert.ok(PRETOOL_MATCHER.includes("search_replace"));
    assert.ok(PRETOOL_MATCHER.includes(MCP_TOOL_PATTERN));
  }),

  test("pretoolHookDecision defers or denies and never allows", () => {
    const samples = [
      [{ bypass: true, mode: "active", choice: "stop", gate: "hold", blocked: true }, "defer", "bypass"],
      [{ mode: "shadow", choice: "stop", gate: "hold", blocked: false }, "defer", "shadow"],
      [{ mode: "active", choice: "continue", gate: "auto", blocked: false }, "defer", "exec"],
      [{ mode: "active", choice: "unclassified", gate: "auto", blocked: false }, "defer", "exec"],
      [
        {
          mode: "active",
          choice: "stop",
          gate: "hold",
          blocked: true,
          pretool: { policyId: "omapi-loop-stop-policy@1" },
        },
        "deny",
        "jev choice=stop gate=hold policy=omapi-loop-stop-policy@1",
      ],
      [{ mode: "active", choice: "escalate", gate: "hold", blocked: true }, "deny", "jev choice=escalate gate=hold"],
      [{ mode: "active", choice: "unclassified", gate: "hold", blocked: false }, "deny", "jev choice=unclassified gate=hold"],
    ];
    for (const [verdict, decision, reason] of samples) {
      const hook = pretoolHookDecision(verdict);
      assert.equal(hook.decision, decision);
      assert.notEqual(hook.decision, "allow");
      assert.equal(hook.reason, reason);
      assert.equal(hook.exitCode, decision === "deny" ? 2 : 0);
      assert.equal(hook.action, decision);
    }
    const stamp = pretoolStamp({ toolName: "search_replace" });
    assert.equal(stamp.class, "write");
    assert.equal(stamp.policyId, CHECK_POLICY.version);
    assert.equal(pretoolStamp({}), null);
    assert.equal(pretoolStamp({ toolName: "read_file" }).matched, false);
    const mismatch = pretoolStamp({ toolName: "Bash", toolClass: "write" });
    assert.equal(mismatch.class, "write");
    assert.equal(mismatch.policyId, CHECK_POLICY.version);
    assert.equal(mismatch.mismatch, true);
  }),

  test("router source keeps one client and the shared bypass predicate", () => {
    const routerSrc = readFileSync(join(here, "jev-router.mjs"), "utf8");
    assert.match(routerSrc, /isJevBypass\(process\.env\.JEV_BYPASS\)/);
    assert.equal((routerSrc.match(/new TypeSafeClient\(/g) || []).length, 2);
    assert.equal((routerSrc.match(/client\.systemOne\(/g) || []).length, 3);
    assert.match(routerSrc, /shellPermissionQuestion\(choice\)/);
    assert.match(routerSrc, /JEV_PERMISSION_MODE/);
    assert.match(routerSrc, /requestedMode: process\.env\.JEV_PERMISSION_MODE/);
    assert.match(routerSrc, /JEV_TYPED_CALL_MODE/);
    assert.match(routerSrc, /buildCallQuestions\(choice, noul/);
    assert.match(routerSrc, /scrubSecrets\(/);
    assert.match(routerSrc, /buildGateState\(/);
    assert.match(routerSrc, /schemaDumpCall\(/);
    assert.match(routerSrc, /tinyCatalog\(/);
    const catalogSrc = readFileSync(join(here, "catalog.mjs"), "utf8");
    assert.equal((catalogSrc.match(/new TypeSafeClient\(/g) || []).length, 0);
    const callsSrc = readFileSync(join(here, "calls.mjs"), "utf8");
    assert.equal((callsSrc.match(/new TypeSafeClient\(/g) || []).length, 0);
    assert.equal((callsSrc.match(/client\.systemOne\(/g) || []).length, 0);
    assert.doesNotMatch(callsSrc, /process\.env/);
    assert.doesNotMatch(callsSrc, /jsonrpc/);
    assert.doesNotMatch(callsSrc, /tools\/call/);
    assert.match(callsSrc, /tool_call/);
  }),

  test("POLICY-MAP.md quotes the matcher and the hook files", () => {
    const docPath = [join(here, "POLICY-MAP.md"), join(here, "../../docs/POLICY-MAP.md")].find((path) =>
      existsSync(path),
    );
    assert.ok(docPath, "POLICY-MAP.md missing");
    const doc = readFileSync(docPath, "utf8");
    const block = doc.match(/```pretool-matcher\n([\s\S]*?)\n```/);
    assert.ok(block, "pretool-matcher fence missing");
    assert.equal(block[1], PRETOOL_MATCHER);
    assert.match(doc, /~\/\.grok\/hooks\/jev-omapi\.json/);
    assert.match(doc, /~\/\.grok\/hooks\/bin\/jev-pretool\.sh/);
    assert.match(doc, /hooks\.PreToolUse\[0\]\.matcher/);
    assert.match(doc, /JEV_BYPASS/);
    assert.match(doc, /never `allow`|never emits `allow`|do not emit `allow`|Never emit `\{"decision":"allow"\}`/);
    assert.match(doc, /JEV_PERMISSION_MODE/);
    assert.match(doc, /until this permission surface has been re-checked/);
    assert.match(doc, /allow→continue/);
    assert.match(doc, /Key absent forces this surface to shadow/);
    assert.match(doc, /JEV_MODE=active` does not honor the catalog/);
    assert.match(doc, /JEV_TYPED_CALL_MODE/);
    assert.match(doc, /tool_call/);
    assert.match(doc, /top-X/);
    assert.match(doc, /Castle Grok hooks leave MCP on ask until this surface is merged/);
  }),

  test("JEV_BYPASS=true stamps shell; yes does not bypass; check ignores bypass", () => {
    const bin = process.env.JEV_ROUTER_BIN;
    if (!bin) return;
    const bypass = spawnSync(bin, ["--tool-name", "run_terminal_command", "probe intent"], {
      env: { ...process.env, JEV_MODE: "shadow", JEV_BYPASS: "true", TYPESAFE_API_KEY: "" },
      encoding: "utf8",
    });
    assert.equal(bypass.status, 0, bypass.stderr);
    const bypassPayload = JSON.parse(bypass.stdout.trim().split("\n").at(-1));
    assert.equal(bypassPayload.bypass, true);
    assert.equal(bypassPayload.choice, "bypass");
    assert.equal(bypassPayload.blocked, false);
    assert.equal(bypassPayload.pretool.class, "shell");
    assert.equal(bypassPayload.pretool.policyId, "omapi-loop-stop-policy@1");
    assert.equal(bypassPayload.pretool.matched, true);

    const notBypass = spawnSync(bin, ["probe intent"], {
      env: { ...process.env, JEV_MODE: "shadow", JEV_BYPASS: "yes", TYPESAFE_API_KEY: "" },
      encoding: "utf8",
    });
    assert.equal(notBypass.status, 0, notBypass.stderr);
    const notPayload = JSON.parse(notBypass.stdout.trim().split("\n").at(-1));
    assert.equal(notPayload.bypass, false);
    assert.equal(notPayload.missingKey, true);
    assert.equal(notPayload.pretool, undefined);

    const writeStamp = spawnSync(bin, ["--tool-name", "search_replace", "probe intent"], {
      env: { ...process.env, JEV_MODE: "shadow", JEV_BYPASS: "", TYPESAFE_API_KEY: "" },
      encoding: "utf8",
    });
    assert.equal(writeStamp.status, 0, writeStamp.stderr);
    const writePayload = JSON.parse(writeStamp.stdout.trim().split("\n").at(-1));
    assert.equal(writePayload.pretool.class, "write");
    assert.equal(writePayload.pretool.policyId, "omapi-check-policy@1");
    assert.equal(writePayload.bypass, false);

    const mcpStamp = spawnSync(bin, ["--tool-name", "linear__save_issue", "probe intent"], {
      env: { ...process.env, JEV_MODE: "shadow", JEV_BYPASS: "1", TYPESAFE_API_KEY: "" },
      encoding: "utf8",
    });
    assert.equal(mcpStamp.status, 0, mcpStamp.stderr);
    const mcpPayload = JSON.parse(mcpStamp.stdout.trim().split("\n").at(-1));
    assert.equal(mcpPayload.bypass, true);
    assert.equal(mcpPayload.pretool.class, "mcp");
    assert.equal(mcpPayload.pretool.policyId, "omapi-route-workflow-policy@1");

    const check = spawnSync(bin, ["--check", "--intent", "probe", "--diff-file", skipDiff], {
      env: { ...process.env, JEV_MODE: "shadow", JEV_BYPASS: "1", TYPESAFE_API_KEY: "" },
      encoding: "utf8",
    });
    assert.equal(check.status, 0, check.stderr);
    const checkPayload = JSON.parse(check.stdout.trim().split("\n").at(-1));
    assert.equal(checkPayload.workflow, "check");
    assert.equal(checkPayload.approval, false);
    assert.notEqual(checkPayload.choice, "bypass");
    assert.equal(checkPayload.catalog.kind, "tiny");
    assert.equal(JSON.stringify(checkPayload.catalog).includes("$schema"), false);
  }),

  test("tiny catalog is the PreToolUse index and has no orphan policyId", () => {
    const catalog = tinyCatalog();
    assert.equal(catalog.kind, "tiny");
    assert.equal(catalog.entries.length, PRETOOL_CLASSES.length);
    const ids = catalog.entries.map((entry) => entry.policyId);
    assert.deepEqual(orphanPolicyIds(ids), []);
    for (const id of POLICY_IDS) assert.ok(ids.includes(id));
    const blob = JSON.stringify(catalog);
    assert.ok(blob.length < 900, `tiny catalog is ${blob.length} chars`);
    assert.equal(blob.includes("$schema"), false);
    assert.equal(blob.includes("properties"), false);
    assert.equal(blob.includes('"decision":"allow"'), false);
    const mcp = catalog.entries.find((entry) => entry.class === "mcp");
    assert.equal(mcp.policyId, ROUTING_POLICY.version);
    assert.equal(mcp.pattern, MCP_TOOL_PATTERN);
    assert.equal(mcp.tools, undefined);
    const shell = catalog.entries.find((entry) => entry.class === "shell");
    assert.deepEqual(shell.tools, ["Bash", "run_terminal_command", "run_terminal_cmd"]);
  }),

  test("every mapped tool has a full schema that the tiny index does not inline", () => {
    const names = mappedToolNames();
    assert.ok(names.length > 0);
    for (const name of names) {
      const schema = fullSchema(name, matchPretoolClass(name));
      assert.ok(schema, name);
      assert.equal(schema.title, name);
      assert.equal(String(schema.$schema).includes("json-schema"), true);
    }
  }),

  test("schemaDumpCall defers a known tool and denies anything outside the map", () => {
    const bash = schemaDumpCall("Bash");
    const bashHook = pretoolHookDecision(bash);
    assert.notEqual(bashHook.decision, "allow");
    assert.equal(bash.call, "schema-dump");
    assert.equal(bash.found, true);
    assert.equal(bash.class, "shell");
    assert.equal(bash.policyId, LOOP_STOP_POLICY.version);
    assert.equal(bash.decision, "defer");
    assert.equal(bash.autoAllow, false);
    assert.equal(bash.schema.properties.command.type, "string");
    assert.equal(bash.schema.required.includes("command"), true);
    assert.equal(JSON.stringify(bash).includes('"decision":"allow"'), false);

    const edit = schemaDumpCall("search_replace");
    assert.equal(edit.class, "write");
    assert.equal(edit.policyId, CHECK_POLICY.version);
    assert.equal(edit.decision, "defer");
    assert.equal(edit.autoAllow, false);

    const mcp = schemaDumpCall("linear__save_issue");
    assert.equal(mcp.class, "mcp");
    assert.equal(mcp.policyId, ROUTING_POLICY.version);
    assert.equal(mcp.typedCall, false);
    assert.equal(mcp.decision, "defer");
    assert.equal(mcp.schema.required.includes("server"), true);
    assert.deepEqual(orphanPolicyIds([mcp.policyId]), []);

    for (const name of ["read_file", "use_tool", ""]) {
      const denied = schemaDumpCall(name);
      assert.equal(denied.decision, "deny");
      assert.equal(denied.found, false);
      assert.equal(denied.schema, null);
      assert.equal(denied.autoAllow, false);
      assert.equal(denied.ok, false);
      assert.notEqual(denied.decision, "allow");
    }
  }),

  test("buildGateState keeps the tiny catalog and strips the API key", () => {
    const previous = process.env.TYPESAFE_API_KEY;
    const secret = ["catalog", "test", "secret"].join("-");
    process.env.TYPESAFE_API_KEY = secret;
    try {
      const state = buildGateState({
        intent: `ship ${secret} please`,
        TYPESAFE_API_KEY: secret,
        apiKey: secret,
        schema: { type: "object", properties: { command: { type: "string" } } },
        loopStopPolicy: LOOP_STOP_POLICY,
      });
      const blob = JSON.stringify(state);
      assert.equal(blob.includes(secret), false);
      assert.equal(state.intent.includes("[redacted]"), true);
      assert.equal(state.catalog.kind, "tiny");
      assert.equal(state.schema, undefined);
      assert.equal(state.TYPESAFE_API_KEY, undefined);
      assert.equal(state.apiKey, undefined);
      assert.equal(blob.includes("$schema"), false);
      assert.equal(state.loopStopPolicy.version, LOOP_STOP_POLICY.version);
    } finally {
      if (previous === undefined) delete process.env.TYPESAFE_API_KEY;
      else process.env.TYPESAFE_API_KEY = previous;
    }
  }),

  test("wrap script does not flip permission catalogs or bake a key", () => {
    const wrapPath = [join(here, "cursor-agent-jev.nix"), join(here, "..", "cursor-agent-jev.nix")].find((path) =>
      existsSync(path),
    );
    assert.ok(wrapPath, "cursor-agent-jev.nix missing");
    const wrap = readFileSync(wrapPath, "utf8");
    assert.equal(wrap.includes("JEV_PERMISSION_MODE"), false);
    assert.equal(wrap.includes("--schema"), true);
    assert.equal(wrap.includes("--catalog"), true);
    assert.equal(wrap.includes("JEV_TOOL_CATALOG"), true);
    assert.equal(/--set(=|\s)TYPESAFE_API_KEY/.test(wrap), false);
  }),

  test("--catalog and --schema do not call Jev and do not print the key", () => {
    const bin = process.env.JEV_ROUTER_BIN;
    if (!bin) return;
    const env = {
      ...process.env,
      JEV_MODE: "shadow",
      JEV_BYPASS: "",
      TYPESAFE_API_KEY: "catalog-test-secret",
    };
    const catalog = spawnSync(bin, ["--catalog"], { env, encoding: "utf8" });
    assert.equal(catalog.status, 0, catalog.stderr);
    const catalogPayload = JSON.parse(catalog.stdout.trim().split("\n").at(-1));
    assert.equal(catalogPayload.kind, "tiny");
    assert.equal(catalog.stdout.includes("$schema"), false);
    assert.equal(catalog.stdout.includes("catalog-test-secret"), false);
    assert.equal(catalog.stderr.includes("catalog-test-secret"), false);
    assert.equal(catalog.stdout.includes('"decision":"allow"'), false);

    const dumped = spawnSync(bin, ["--schema", "Bash"], { env, encoding: "utf8" });
    assert.equal(dumped.status, 0, dumped.stderr);
    const dumpPayload = JSON.parse(dumped.stdout.trim().split("\n").at(-1));
    assert.equal(dumpPayload.call, "schema-dump");
    assert.equal(dumpPayload.decision, "defer");
    assert.equal(dumpPayload.autoAllow, false);
    assert.equal(dumpPayload.policyId, "omapi-loop-stop-policy@1");
    assert.equal(dumpPayload.schema.properties.command.type, "string");
    assert.equal(dumped.stdout.includes("catalog-test-secret"), false);
    assert.equal(dumped.stderr.includes("catalog-test-secret"), false);

    const denied = spawnSync(bin, ["--schema", "read_file"], { env, encoding: "utf8" });
    assert.equal(denied.status, 2, denied.stderr);
    const deniedPayload = JSON.parse(denied.stdout.trim().split("\n").at(-1));
    assert.equal(deniedPayload.decision, "deny");
    assert.equal(deniedPayload.schema, null);
    assert.equal(denied.stdout.includes("catalog-test-secret"), false);

    const missing = spawnSync(bin, ["--schema"], { env, encoding: "utf8" });
    assert.equal(missing.status, 2);

    const shadow = spawnSync(bin, ["probe intent"], {
      env: { ...env, TYPESAFE_API_KEY: "" },
      encoding: "utf8",
    });
    assert.equal(shadow.status, 0, shadow.stderr);
    const shadowPayload = JSON.parse(shadow.stdout.trim().split("\n").at(-1));
    assert.equal(shadowPayload.catalog.kind, "tiny");
    assert.equal(JSON.stringify(shadowPayload.catalog).includes("$schema"), false);
    assert.equal(shadowPayload.choice, "unclassified");
    assert.equal(shadow.stdout.includes("catalog-test-secret"), false);
  }),

  test("permission catalogs wrap the three policy ids and do not orphan one", () => {
    assert.deepEqual(
      PERMISSION_SURFACES.map((row) => row.policyId),
      [LOOP_STOP_POLICY.version, CHECK_POLICY.version, ROUTING_POLICY.version],
    );
    for (const id of PERMISSION_SURFACES.map((row) => row.policyId)) {
      assert.ok(PARENT_POLICY_IDS.includes(id), id);
    }
    assert.equal(permissionSurface("shell").newCatalog, true);
    assert.equal(permissionSurface("shell").parent, "loop-stop");
    assert.equal(permissionSurface("write").newCatalog, false);
    assert.equal(permissionSurface("write").parent, "writer");
    assert.equal(permissionSurface("mcp").newCatalog, false);
    assert.equal(permissionSurface("mcp").catalog, "route-workflow");
    assert.equal(permissionSurface("web"), null);
    assert.equal(permissionSurface("subagent"), null);
    assert.equal(decidePermission({ classId: "read_file", hasKey: true, requestedMode: "active" }), null);
    assert.equal(decidePermission({ classId: "web", hasKey: true, label: "allow" }), null);
  }),

  test("allow→continue, deny→stop, ask→escalate or writer parent", () => {
    const shell = permissionSurface("shell");
    const write = permissionSurface("write");
    assert.equal(mapPermissionLabel("allow", shell), "continue");
    assert.equal(mapPermissionLabel("deny", shell), "stop");
    assert.equal(mapPermissionLabel("ask", shell), "escalate");
    assert.equal(mapPermissionLabel("ask", write), "writer");
    assert.equal(mapPermissionLabel("allow", write), "continue");
    assert.equal(mapPermissionLabel("deny", permissionSurface("mcp")), "stop");
    assert.equal(resolvePermissionMode(undefined), "shadow");
    assert.equal(resolvePermissionMode("yes"), "shadow");
    assert.equal(resolvePermissionMode("true"), "shadow");
    assert.equal(resolvePermissionMode("ACTIVE"), "active");
  }),

  test("key absent forces permission shadow even if active was requested", () => {
    const shell = decidePermission({
      classId: "shell",
      requestedMode: "active",
      hasKey: false,
      contentPresent: true,
      label: "deny",
      confidence: 0.99,
      probabilities: { allow: 0.01, deny: 0.98, ask: 0.01 },
    });
    assert.equal(shell.mode, "shadow");
    assert.equal(shell.honor, false);
    assert.equal(shell.shadow, true);
    assert.equal(shell.blocked, false);
    assert.equal(shell.missingKey, true);
    assert.equal(shell.reason, "missing_key");
    assert.equal(shell.choice, "unclassified");
    assert.equal(shell.policyId, "omapi-loop-stop-policy@1");
    assert.equal(shell.autoAllow, false);

    const write = decidePermission({
      classId: "write",
      requestedMode: "active",
      hasKey: false,
      path: ".env",
      label: "allow",
    });
    assert.equal(write.mode, "shadow");
    assert.equal(write.blocked, false);
    assert.equal(write.honor, false);
    assert.equal(write.missingKey, true);
    assert.equal(write.codeDeny, true);
    assert.equal(write.mapped, "stop");
    assert.equal(write.label, "deny");
    assert.equal(write.modelLabel, "allow");
    assert.equal(write.policyId, "omapi-check-policy@1");
    assert.equal(write.choice, "unclassified");
  }),

  test("shell permission uses loop-stop thresholds and does not allow without content", () => {
    const confident = {
      confidence: 0.9,
      probabilities: { allow: 0.8, deny: 0.1, ask: 0.1 },
    };
    const allowed = decidePermission({
      classId: "shell",
      requestedMode: "shadow",
      hasKey: true,
      contentPresent: true,
      label: "allow",
      ...confident,
    });
    assert.equal(allowed.mode, "shadow");
    assert.equal(allowed.honor, false);
    assert.equal(allowed.blocked, false);
    assert.equal(allowed.choice, "unclassified");
    assert.equal(allowed.mapped, "continue");
    assert.equal(allowed.label, "allow");
    assert.equal(allowed.policyId, LOOP_STOP_POLICY.version);
    assert.equal(allowed.newCatalog, true);
    assert.equal(allowed.autoAllow, false);

    const honored = decidePermission({
      classId: "shell",
      requestedMode: "active",
      hasKey: true,
      contentPresent: true,
      label: "allow",
      ...confident,
    });
    assert.equal(honored.honor, true);
    assert.equal(honored.choice, "continue");
    assert.equal(honored.gate, "auto");
    assert.equal(honored.blocked, false);
    assert.equal(honored.mapped, "continue");

    const uncertain = decidePermission({
      classId: "shell",
      requestedMode: "active",
      hasKey: true,
      contentPresent: true,
      label: "allow",
      confidence: 0.4,
      probabilities: { allow: 0.4, deny: 0.35, ask: 0.25 },
    });
    assert.equal(uncertain.mapped, "escalate");
    assert.equal(uncertain.choice, "escalate");
    assert.equal(uncertain.blocked, true);
    assert.equal(uncertain.modelLabel, "allow");
    assert.notEqual(uncertain.choice, "continue");

    const noContent = decidePermission({
      classId: "shell",
      requestedMode: "active",
      hasKey: true,
      contentPresent: false,
      label: "allow",
      ...confident,
    });
    assert.equal(noContent.label, "ask");
    assert.equal(noContent.mapped, "escalate");
    assert.equal(noContent.reason, "no_content");
    assert.equal(noContent.modelLabel, "allow");
    assert.equal(noContent.choice, "escalate");

    const ask = decidePermission({
      classId: "shell",
      requestedMode: "active",
      hasKey: true,
      contentPresent: true,
      label: "ask",
      confidence: 0.2,
      probabilities: { allow: 0.2, deny: 0.2, ask: 0.6 },
    });
    assert.equal(ask.mapped, "escalate");
    assert.equal(ask.choice, "escalate");
    assert.equal(ask.hitl, true);
  }),

  test("write permission wraps the writer and never treats empty findings as allow", () => {
    const flags = collectWriteFlags({ diffText: readFileSync(skipDiff, "utf8") });
    assert.ok(flags.includes("skip_marker_added"));
    const denied = decidePermission({
      classId: "write",
      requestedMode: "active",
      hasKey: true,
      flags,
      label: "allow",
    });
    assert.equal(denied.codeDeny, true);
    assert.equal(denied.label, "deny");
    assert.equal(denied.modelLabel, "allow");
    assert.equal(denied.mapped, "stop");
    assert.equal(denied.choice, "stop");
    assert.equal(denied.blocked, true);
    assert.equal(denied.newCatalog, false);
    assert.equal(denied.policyId, CHECK_POLICY.version);
    assert.equal(denied.parent, "writer");

    const empty = decidePermission({
      classId: "write",
      requestedMode: "active",
      hasKey: true,
      diffText: "",
      path: "src/app.js",
      label: "allow",
    });
    assert.equal(empty.codeDeny, false);
    assert.equal(empty.label, "ask");
    assert.equal(empty.mapped, "writer");
    assert.equal(empty.choice, "escalate");
    assert.equal(empty.reason, "empty_findings_not_approval");
    assert.notEqual(empty.mapped, "continue");
    assert.equal(empty.autoAllow, false);

    const shadowEmpty = decidePermission({
      classId: "write",
      requestedMode: "shadow",
      hasKey: true,
      path: "src/app.js",
    });
    assert.equal(shadowEmpty.honor, false);
    assert.equal(shadowEmpty.blocked, false);
    assert.equal(shadowEmpty.choice, "unclassified");
    assert.equal(shadowEmpty.mapped, "writer");
  }),

  test("mcp permission wraps route-workflow and does not add a catalog", () => {
    const mcp = decidePermission({
      classId: "mcp",
      requestedMode: "active",
      hasKey: true,
      routingOutcome: "check",
      label: "allow",
    });
    assert.equal(mcp.newCatalog, false);
    assert.equal(mcp.catalog, "route-workflow");
    assert.equal(mcp.policyId, ROUTING_POLICY.version);
    assert.equal(mcp.label, "ask");
    assert.equal(mcp.mapped, "escalate");
    assert.equal(mcp.choice, "escalate");
    assert.equal(mcp.blocked, true);
    assert.equal(mcp.reason, "workflow_is_not_permission");
    assert.equal(mcp.autoAllow, false);

    const unknown = decidePermission({
      classId: "mcp",
      requestedMode: "shadow",
      hasKey: true,
    });
    assert.equal(unknown.honor, false);
    assert.equal(unknown.blocked, false);
    assert.equal(unknown.reason, "cannot_tell");
    assert.equal(unknown.policyId, "omapi-route-workflow-policy@1");
  }),

  test("permission continue does not auto-allow over a parent stop", () => {
    const permission = decidePermission({
      classId: "shell",
      requestedMode: "active",
      hasKey: true,
      contentPresent: true,
      label: "allow",
      confidence: 0.9,
      probabilities: { allow: 0.8, deny: 0.1, ask: 0.1 },
    });
    const merged = applyPermissionVerdict(
      { choice: "stop", gate: "hold", blocked: true, exec: false, hitl: false, mode: "active" },
      permission,
    );
    assert.equal(merged.choice, "stop");
    assert.equal(merged.blocked, true);
    assert.equal(merged.permission.mapped, "continue");
    assert.equal(merged.permission.autoAllow, false);

    const deny = decidePermission({
      classId: "shell",
      requestedMode: "active",
      hasKey: true,
      contentPresent: true,
      label: "deny",
      confidence: 0.9,
      probabilities: { allow: 0.05, deny: 0.9, ask: 0.05 },
    });
    const stopped = applyPermissionVerdict(
      { choice: "continue", gate: "auto", blocked: false, exec: true, mode: "shadow" },
      deny,
    );
    assert.equal(deny.choice, "stop");
    assert.equal(stopped.choice, "stop");
    assert.equal(stopped.blocked, true);
    assert.equal(stopped.exec, false);

    const shadowDeny = decidePermission({
      classId: "shell",
      requestedMode: "shadow",
      hasKey: true,
      contentPresent: true,
      label: "deny",
      confidence: 0.9,
      probabilities: { allow: 0.05, deny: 0.9, ask: 0.05 },
    });
    const logged = applyPermissionVerdict(
      { choice: "continue", gate: "auto", blocked: false, exec: true, mode: "shadow" },
      shadowDeny,
    );
    assert.equal(logged.choice, "continue");
    assert.equal(logged.blocked, false);
    assert.equal(logged.permission.mapped, "stop");
    assert.equal(logged.permission.honor, false);
  }),

  test("pretool hook honors an active permission surface and still never allows", () => {
    const stopPerm = decidePermission({
      classId: "shell",
      requestedMode: "active",
      hasKey: true,
      contentPresent: true,
      label: "deny",
      confidence: 0.9,
      probabilities: { allow: 0.05, deny: 0.9, ask: 0.05 },
    });
    const denied = pretoolHookDecision({
      mode: "shadow",
      choice: "continue",
      gate: "auto",
      blocked: false,
      pretool: { policyId: stopPerm.policyId },
      permission: stopPerm,
    });
    assert.equal(denied.decision, "deny");
    assert.notEqual(denied.decision, "allow");
    assert.equal(denied.exitCode, 2);
    assert.match(denied.reason, /choice=stop/);
    assert.match(denied.reason, /policy=omapi-loop-stop-policy@1/);

    const allowPerm = decidePermission({
      classId: "shell",
      requestedMode: "active",
      hasKey: true,
      contentPresent: true,
      label: "allow",
      confidence: 0.9,
      probabilities: { allow: 0.8, deny: 0.1, ask: 0.1 },
    });
    const deferred = pretoolHookDecision({
      mode: "shadow",
      choice: "continue",
      gate: "auto",
      blocked: false,
      permission: allowPerm,
    });
    assert.equal(deferred.decision, "defer");
    assert.equal(deferred.reason, "exec");
    assert.notEqual(deferred.decision, "allow");

    const noOverride = pretoolHookDecision({
      mode: "shadow",
      choice: "stop",
      gate: "hold",
      blocked: false,
      pretool: { policyId: "omapi-loop-stop-policy@1" },
      permission: allowPerm,
    });
    assert.equal(noOverride.decision, "deny");
    assert.match(noOverride.reason, /choice=stop/);

    const shadowCatalog = pretoolHookDecision({
      mode: "active",
      choice: "continue",
      gate: "auto",
      blocked: false,
      permission: { ...stopPerm, mode: "shadow", honor: false, blocked: false, choice: "unclassified" },
    });
    assert.equal(shadowCatalog.decision, "defer");
    assert.equal(shadowCatalog.reason, "exec");

    const bypass = permissionBypass("mcp");
    assert.equal(bypass.skipped, true);
    assert.equal(bypass.reason, "bypass");
    assert.equal(bypass.blocked, false);
    assert.equal(bypass.policyId, ROUTING_POLICY.version);
    assert.equal(permissionBypass("web"), null);
  }),

  test("shadow permission spawn stays unblocked without a key", () => {
    const bin = process.env.JEV_ROUTER_BIN;
    if (!bin) return;
    const shell = spawnSync(bin, ["--tool-name", "Bash", "--tool-input", "npm test", "probe intent"], {
      env: {
        ...process.env,
        JEV_MODE: "shadow",
        JEV_PERMISSION_MODE: "active",
        JEV_BYPASS: "",
        TYPESAFE_API_KEY: "",
      },
      encoding: "utf8",
    });
    assert.equal(shell.status, 0, shell.stderr);
    const shellPayload = JSON.parse(shell.stdout.trim().split("\n").at(-1));
    assert.equal(shellPayload.missingKey, true);
    assert.equal(shellPayload.blocked, false);
    assert.equal(shellPayload.permission.mode, "shadow");
    assert.equal(shellPayload.permission.honor, false);
    assert.equal(shellPayload.permission.missingKey, true);
    assert.equal(shellPayload.permission.reason, "missing_key");
    assert.equal(shellPayload.permission.policyId, "omapi-loop-stop-policy@1");
    assert.equal(shellPayload.permission.autoAllow, false);
    assert.equal(shellPayload.pretool.policyId, shellPayload.permission.policyId);

    const write = spawnSync(bin, ["--tool-name", "Write", "--tool-input", ".env", "probe intent"], {
      env: {
        ...process.env,
        JEV_MODE: "shadow",
        JEV_PERMISSION_MODE: "active",
        JEV_BYPASS: "",
        TYPESAFE_API_KEY: "",
      },
      encoding: "utf8",
    });
    assert.equal(write.status, 0, write.stderr);
    const writePayload = JSON.parse(write.stdout.trim().split("\n").at(-1));
    assert.equal(writePayload.blocked, false);
    assert.equal(writePayload.permission.mode, "shadow");
    assert.equal(writePayload.permission.codeDeny, true);
    assert.equal(writePayload.permission.mapped, "stop");
    assert.equal(writePayload.permission.blocked, false);
    assert.equal(writePayload.permission.policyId, "omapi-check-policy@1");
    assert.equal(writePayload.choice, "unclassified");
  }),

  test("typed calls rank best and top-X on the route-workflow policy", () => {
    const catalog = normalizeCatalog({
      interface: "Mcp",
      tools: [
        {
          name: "linear__list_issues",
          description: "List Linear issues",
          effect: "read",
          args: [
            {
              name: "team",
              question: "Which team should the issues come from?",
              stated: "Does the intent name a team?",
              optional: true,
              options: { eng: "Engineering", ops: "Operations" },
            },
          ],
        },
        {
          name: "linear__save_issue",
          description: "Create or update an issue",
          effect: "write",
          args: [],
        },
        {
          name: "notes__add",
          description: "Add a free-text note",
          effect: "read",
          args: [{ name: "body", question: "What is the note text?", optional: false }],
        },
      ],
    });
    assert.equal(catalog.error, null);
    const readAnswer = {
      choice: "linear__list_issues",
      confidence: 0.91,
      probabilities: {
        linear__list_issues: 0.8,
        linear__save_issue: 0.12,
        notes__add: 0.05,
        cannot_tell: 0.03,
      },
    };
    const argAnswers = {
      "linear__list_issues.team?": { noul: 0.92 },
      "linear__list_issues.team": {
        choice: "eng",
        confidence: 0.88,
        probabilities: { eng: 0.84, ops: 0.16 },
      },
    };
    const picked = decideTypedCalls({
      catalog,
      requestedMode: "active",
      hasKey: true,
      topX: 2,
      answer: readAnswer,
      argAnswers,
    });
    assert.equal(picked.transport, "facet");
    assert.equal(picked.policyId, ROUTING_POLICY.version);
    assert.equal(picked.honor, true);
    assert.equal(picked.label, "allow");
    assert.equal(picked.mapped, "continue");
    assert.equal(picked.autoAllow, false);
    assert.equal(picked.autoPromote, false);
    assert.equal(picked.initiated, false);
    assert.equal(picked.best.name, "linear__list_issues");
    assert.equal(picked.best.args.team, "eng");
    assert.equal(picked.best.initiated, false);
    assert.equal(picked.best.rank, 1);
    assert.equal(picked.top.length, 2);
    assert.equal(picked.top[0].name, "linear__list_issues");
    assert.equal(picked.top[1].name, "linear__save_issue");
    assert.equal(picked.facet.op, "tool_call");
    assert.equal(picked.facet.name, "Mcp.linear__list_issues");
    assert.equal(picked.facet.guard.decision, "allowed");
    assert.equal(picked.facet.initiated, false);
    assert.equal(picked.canonical.tools.length, 2);
    assert.ok(picked.canonical.tools.every((tool) => tool.effect === "read"));
    assert.equal(
      picked.canonical.tools.some((tool) => tool.name === "Mcp.linear__save_issue"),
      false,
    );
    const events = picked.artifact.provenance.events;
    assert.deepEqual(
      events.map((event) => event.seq),
      events.map((_, index) => index + 1),
    );
    assert.equal(events.at(-1).op, "tool_call");
    assert.equal(events.at(-1).decision, "allowed");
    assert.equal(facetHead(picked.canonical.metadata, events), picked.artifact.provenance.hash_chain.head);
    assert.equal(JSON.stringify(picked).includes("jsonrpc"), false);
    assert.equal(JSON.stringify(picked).includes("tools/call"), false);

    const questions = buildCallQuestions(
      (instructions, criteria) => ({ instructions, criteria }),
      (instructions) => ({ instructions }),
      catalog,
    );
    assert.equal(questions.call.criteria.cannot_tell.length > 0, true);
    assert.ok(questions["linear__list_issues.team"]);
    assert.ok(questions["linear__list_issues.team?"]);
    assert.equal(questions["notes__add.body"], undefined);
  }),

  test("typed calls deny side effects and do not promote the next tool", () => {
    assert.equal(guardEffect("read").allow, true);
    assert.equal(guardEffect("write").code, "F454");
    assert.equal(guardEffect("payment").allow, false);
    assert.equal(guardEffect("filesystem").code, "F454");
    assert.equal(guardEffect("external").allow, false);
    assert.equal(guardEffect("network").code, "F454");
    assert.equal(guardEffect("").code, "F456");
    assert.equal(guardEffect("not-an-effect").code, "F456");
    assert.equal(guardEffect("x.acme.audit").allow, false);
    assert.equal(clampTopX("nope"), 3);
    assert.equal(clampTopX(0), 1);
    assert.equal(clampTopX(99), 8);

    const catalog = normalizeCatalog({
      tools: [
        { name: "linear__list_issues", description: "List issues", effect: "read", args: [] },
        { name: "linear__save_issue", description: "Save an issue", effect: "write", args: [] },
        { name: "linear.save", description: "Bad name", effect: "read", args: [] },
        { name: "bare__tool", description: "No effect", args: [] },
      ],
    });
    const denied = decideTypedCalls({
      catalog,
      requestedMode: "active",
      hasKey: true,
      answer: {
        choice: "linear__save_issue",
        confidence: 0.93,
        probabilities: { linear__save_issue: 0.84, linear__list_issues: 0.16 },
      },
    });
    assert.equal(denied.best, null);
    assert.equal(denied.label, "deny");
    assert.equal(denied.mapped, "stop");
    assert.equal(denied.choice, "stop");
    assert.equal(denied.blocked, true);
    assert.equal(denied.code, "F454");
    assert.equal(denied.codeDeny, true);
    assert.equal(denied.reason, "effect_deny");
    assert.equal(denied.autoPromote, false);
    assert.equal(denied.facet.guard.decision, "denied");
    assert.equal(denied.facet.initiated, false);
    assert.equal(denied.facet.args && Object.keys(denied.facet.args).length, 0);
    assert.equal(
      denied.canonical.tools.some((tool) => String(tool.name).includes("save")),
      false,
    );

    const invalid = decideTypedCalls({
      catalog,
      requestedMode: "active",
      hasKey: true,
      answer: {
        choice: "linear.save",
        confidence: 0.9,
        probabilities: { "linear.save": 0.8, linear__list_issues: 0.2 },
      },
    });
    assert.equal(invalid.code, "F452");
    assert.equal(invalid.facet, null);
    assert.equal(invalid.best, null);
    assert.equal(invalid.label, "deny");

    const missingEffect = decideTypedCalls({
      catalog,
      requestedMode: "active",
      hasKey: true,
      answer: {
        choice: "bare__tool",
        confidence: 0.9,
        probabilities: { bare__tool: 0.8, linear__list_issues: 0.2 },
      },
    });
    assert.equal(missingEffect.code, "F456");
    assert.equal(missingEffect.best, null);
    assert.equal(missingEffect.facet.guard.decision, "denied");
  }),

  test("typed calls ask instead of allowing when the pick is not a read call", () => {
    const catalog = normalizeCatalog({
      tools: [
        {
          name: "linear__list_issues",
          description: "List issues",
          effect: "read",
          args: [{ name: "team", question: "Which team?", options: { eng: "Engineering", ops: "Operations" } }],
        },
        {
          name: "notes__add",
          description: "Add a note",
          effect: "read",
          args: [{ name: "body", question: "What should the note say?", optional: false }],
        },
      ],
    });
    const uncertain = decideTypedCalls({
      catalog,
      requestedMode: "active",
      hasKey: true,
      answer: {
        choice: "linear__list_issues",
        confidence: 0.42,
        probabilities: { linear__list_issues: 0.4, notes__add: 0.35, cannot_tell: 0.25 },
      },
    });
    assert.equal(uncertain.reason, "model_uncertain");
    assert.equal(uncertain.label, "ask");
    assert.equal(uncertain.mapped, "escalate");
    assert.equal(uncertain.best, null);
    assert.equal(uncertain.facet, null);
    assert.equal(uncertain.blocked, true);
    assert.notEqual(uncertain.label, "allow");

    const abstain = decideTypedCalls({
      catalog,
      requestedMode: "shadow",
      hasKey: true,
      answer: {
        choice: "cannot_tell",
        confidence: 0.9,
        probabilities: { cannot_tell: 0.9, linear__list_issues: 0.1 },
      },
    });
    assert.equal(abstain.reason, "cannot_tell");
    assert.equal(abstain.honor, false);
    assert.equal(abstain.blocked, false);
    assert.equal(abstain.choice, "unclassified");

    const openArg = decideTypedCalls({
      catalog,
      requestedMode: "active",
      hasKey: true,
      answer: {
        choice: "notes__add",
        confidence: 0.9,
        probabilities: { notes__add: 0.8, linear__list_issues: 0.2 },
      },
    });
    assert.equal(openArg.reason, "open_arg");
    assert.equal(openArg.label, "ask");
    assert.equal(openArg.best, null);

    const unknown = decideTypedCalls({
      catalog,
      requestedMode: "active",
      hasKey: true,
      answer: {
        choice: "not_a_tool",
        confidence: 0.9,
        probabilities: { not_a_tool: 0.9, linear__list_issues: 0.1 },
      },
    });
    assert.equal(unknown.reason, "unknown_tool");
    assert.equal(unknown.codeDeny, true);
    assert.equal(unknown.best, null);

    const noCatalog = decideTypedCalls({ requestedMode: "active", hasKey: true });
    assert.equal(noCatalog.reason, "no_catalog");
    assert.equal(noCatalog.label, "ask");
    assert.equal(noCatalog.blocked, true);

    const unreadable = decideTypedCalls({
      catalogError: "catalog_unreadable",
      requestedMode: "active",
      hasKey: true,
    });
    assert.equal(unreadable.reason, "catalog_unreadable");
    assert.equal(unreadable.label, "ask");

    const bypass = callsBypass();
    assert.equal(bypass.skipped, true);
    assert.equal(bypass.reason, "bypass");
    assert.equal(bypass.honor, false);
    assert.equal(bypass.policyId, ROUTING_POLICY.version);
  }),

  test("missing key forces typed calls to shadow and scrubs secrets", () => {
    const secret = "supersecretvalue";
    const scrubbed = scrubSecrets(
      {
        TYPESAFE_API_KEY: secret,
        api_key: secret,
        tools: [{ name: "linear__list_issues", description: `List ${secret} issues`, effect: "read" }],
      },
      [secret],
    );
    assert.equal(scrubbed.TYPESAFE_API_KEY, undefined);
    assert.equal(scrubbed.api_key, undefined);
    assert.equal(scrubbed.tools[0].description.includes(secret), false);
    const catalog = normalizeCatalog(scrubbed);
    assert.equal(catalog.tools.length, 1);

    const forced = decideTypedCalls({
      catalog,
      requestedMode: "active",
      hasKey: false,
      answer: {
        choice: "linear__list_issues",
        confidence: 0.99,
        probabilities: { linear__list_issues: 0.99 },
      },
    });
    assert.equal(forced.mode, "shadow");
    assert.equal(forced.honor, false);
    assert.equal(forced.missingKey, true);
    assert.equal(forced.blocked, false);
    assert.equal(forced.best, null);
    assert.equal(forced.initiated, false);
    assert.equal(forced.label, "ask");
    assert.notEqual(forced.mapped, "continue");
    assert.equal(JSON.stringify(forced).includes(secret), false);

    const shadowAllow = decideTypedCalls({
      catalog,
      requestedMode: "yes",
      hasKey: true,
      answer: {
        choice: "linear__list_issues",
        confidence: 0.9,
        probabilities: { linear__list_issues: 0.8, cannot_tell: 0.2 },
      },
    });
    assert.equal(shadowAllow.mode, "shadow");
    assert.equal(shadowAllow.label, "allow");
    assert.equal(shadowAllow.honor, false);
    assert.equal(shadowAllow.blocked, false);
    const logged = pretoolHookDecision({
      mode: "shadow",
      choice: "continue",
      gate: "auto",
      blocked: false,
      calls: shadowAllow,
    });
    assert.equal(logged.decision, "defer");
    assert.equal(logged.reason, "shadow");
    assert.notEqual(logged.decision, "allow");
  }),

  test("a honoring typed call does not auto-allow over a stop", () => {
    const catalog = normalizeCatalog({
      tools: [
        { name: "linear__list_issues", description: "List issues", effect: "read", args: [] },
        { name: "linear__save_issue", description: "Save an issue", effect: "write", args: [] },
      ],
    });
    const allowed = decideTypedCalls({
      catalog,
      requestedMode: "active",
      hasKey: true,
      answer: {
        choice: "linear__list_issues",
        confidence: 0.9,
        probabilities: { linear__list_issues: 0.8, linear__save_issue: 0.2 },
      },
    });
    const kept = applyCallsVerdict(
      { choice: "stop", gate: "hold", blocked: true, exec: false, mode: "active" },
      allowed,
    );
    assert.equal(kept.choice, "stop");
    assert.equal(kept.blocked, true);
    assert.equal(kept.calls.mapped, "continue");
    assert.equal(kept.calls.autoAllow, false);
    const hook = pretoolHookDecision({
      ...kept,
      mode: "shadow",
      pretool: { policyId: "omapi-loop-stop-policy@1" },
    });
    assert.equal(hook.decision, "deny");
    assert.match(hook.reason, /choice=stop/);
    assert.notEqual(hook.decision, "allow");

    const deferred = pretoolHookDecision(
      applyCallsVerdict(
        { mode: "shadow", choice: "continue", gate: "auto", blocked: false, exec: true },
        allowed,
      ),
    );
    assert.equal(deferred.decision, "defer");
    assert.equal(deferred.reason, "exec");
    assert.notEqual(deferred.decision, "allow");

    const denied = decideTypedCalls({
      catalog,
      requestedMode: "active",
      hasKey: true,
      answer: {
        choice: "linear__save_issue",
        confidence: 0.9,
        probabilities: { linear__save_issue: 0.85, linear__list_issues: 0.15 },
      },
    });
    const stopped = applyCallsVerdict(
      { mode: "shadow", choice: "continue", gate: "auto", blocked: false, exec: true },
      denied,
    );
    assert.equal(stopped.choice, "stop");
    assert.equal(stopped.blocked, true);
    assert.equal(stopped.exec, false);
    const deniedHook = pretoolHookDecision(stopped);
    assert.equal(deniedHook.decision, "deny");
    assert.match(deniedHook.reason, /policy=omapi-route-workflow-policy@1/);
    assert.notEqual(deniedHook.decision, "allow");

    const adopted = decidePermission({
      classId: "mcp",
      requestedMode: "active",
      hasKey: true,
      routingOutcome: "check",
      typed: allowed,
    });
    assert.equal(adopted.policyId, ROUTING_POLICY.version);
    assert.equal(adopted.mapped, "continue");
    assert.equal(adopted.label, "allow");
    assert.equal(adopted.autoAllow, false);
    const stillAsk = decidePermission({
      classId: "mcp",
      requestedMode: "active",
      hasKey: true,
      routingOutcome: "check",
      typed: { ...allowed, honor: false, mode: "shadow" },
    });
    assert.equal(stillAsk.reason, "workflow_is_not_permission");
    assert.equal(stillAsk.label, "ask");
  }),

  test("typed call CLI stays shadow without a key and does not print a secret", () => {
    const dir = mkdtempSync(join(tmpdir(), "jev-calls-"));
    const file = join(dir, "tools.json");
    const secret = "supersecretvalue";
    writeFileSync(
      file,
      JSON.stringify({
        interface: "Mcp",
        TYPESAFE_API_KEY: secret,
        tools: [{ name: "linear__list_issues", description: "List issues", effect: "read", args: [] }],
      }),
    );
    const argv = ["--calls", "--tools-file", file, "--tool-name", "linear__list_issues", "--top", "2", "list the issues"];
    const env = {
      ...process.env,
      JEV_MODE: "shadow",
      JEV_TYPED_CALL_MODE: "active",
      JEV_PERMISSION_MODE: "active",
      JEV_BYPASS: "",
      TYPESAFE_API_KEY: "",
    };
    const bin = process.env.JEV_ROUTER_BIN;
    const run = bin
      ? spawnSync(bin, argv, { env, encoding: "utf8" })
      : spawnSync(process.execPath, [join(here, "jev-router.mjs"), ...argv], { env, encoding: "utf8" });
    rmSync(dir, { recursive: true, force: true });
    assert.equal(run.status, 0, run.stderr);
    assert.equal(run.stdout.includes(secret), false, run.stdout);
    assert.equal(run.stderr.includes(secret), false, run.stderr);
    const payload = JSON.parse(run.stdout.trim().split("\n").at(-1));
    assert.equal(payload.blocked, false);
    assert.equal(payload.pretool.class, "mcp");
    assert.equal(payload.pretool.policyId, payload.calls.policyId);
    assert.equal(payload.calls.transport, "facet");
    assert.equal(payload.calls.mode, "shadow");
    assert.equal(payload.calls.honor, false);
    assert.equal(payload.calls.missingKey, true);
    assert.equal(payload.calls.blocked, false);
    assert.equal(payload.calls.initiated, false);
    assert.equal(payload.calls.autoAllow, false);
    assert.equal(payload.calls.best, null);
    assert.equal(payload.calls.reason, "missing_key");
    assert.equal(payload.calls.policyId, "omapi-route-workflow-policy@1");
    assert.equal(payload.calls.canonical.tools.length, 1);
    assert.ok(payload.calls.artifact.provenance.events.every((event) => event.op === "tool_expose"));
    assert.equal(payload.permission.missingKey, true);
    assert.equal(payload.permission.honor, false);
    assert.equal(payload.permission.blocked, false);
    assert.equal(payload.permission.policyId, "omapi-route-workflow-policy@1");
    const hookLine = run.stdout.trim().split("\n").at(-1);
    assert.equal(hookLine.includes("jsonrpc"), false);
    assert.equal(hookLine.includes("tools/call"), false);
  }),

  test("grok build harness uses the existing map and does not add a client", () => {
    const src = readFileSync(join(here, "harness.mjs"), "utf8");
    assert.equal((src.match(/new TypeSafeClient\(/g) || []).length, 0);
    assert.match(src, /from "\.\/policy\.mjs"/);
    assert.match(src, /PRETOOL_MATCHER/);
    assert.match(src, /isJevBypass/);
    assert.match(src, /pretoolHookDecision/);
    assert.match(src, /tinyCatalog/);
    assert.match(src, /decideTypedCalls/);
    assert.match(src, /decidePermission/);
    assert.doesNotMatch(src, /PRETOOL_CLASSES\s*=/);
    assert.doesNotMatch(src, /web_search\|WebSearch/);
    const config = harnessConfig();
    assert.equal(config.hooks.PreToolUse.length, 1);
    assert.equal(config.hooks.PreToolUse[0].matcher, PRETOOL_MATCHER);
    const blob = JSON.stringify(config);
    assert.equal(blob.includes("TYPESAFE_API_KEY"), false);
    assert.equal(blob.includes("JEV_MODE"), false);
    assert.equal(blob.includes('"allow"'), false);
    const modes = harnessModes({ JEV_MODE: "active", JEV_PERMISSION_MODE: "yes", JEV_TYPED_CALL_MODE: "true" });
    assert.equal(modes.jevMode, "active");
    assert.equal(modes.permissionMode, "shadow");
    assert.equal(modes.typedCallMode, "shadow");
    assert.equal(modes.bypass, false);
    const snake = parseHookEvent('{"tool_name":"Bash","tool_input":{"command":"npm test"}}');
    assert.equal(snake.ok, true);
    assert.equal(snake.event.toolName, "Bash");
    assert.equal(snake.event.toolInput.command, "npm test");
  }),

  test("grok build smoke proves class, catalog, typed call, and permission mode", () => {
    const report = runSmoke();
    assert.equal(report.ok, true);
    assert.equal(report.pretool.class, "shell");
    assert.equal(report.pretool.policyId, LOOP_STOP_POLICY.version);
    assert.equal(report.pretool.matched, true);
    const ids = [...new Set(report.classes.map((row) => row.policyId))].sort();
    assert.deepEqual(ids, [CHECK_POLICY.version, LOOP_STOP_POLICY.version, ROUTING_POLICY.version].sort());
    assert.equal(report.catalog.kind, "tiny");
    assert.equal(report.catalog.schemaInline, false);
    assert.ok(report.catalog.bytes < 900);
    assert.equal(report.schemaDump.decision, "defer");
    assert.equal(report.schemaDump.typedCall, false);
    assert.equal(report.schemaDump.autoAllow, false);
    assert.equal(report.calls.transport, "facet");
    assert.equal(report.calls.op, "tool_call");
    assert.equal(report.calls.initiated, false);
    assert.equal(report.calls.autoAllow, false);
    assert.equal(report.calls.policyId, ROUTING_POLICY.version);
    assert.equal(report.permission.shadow.decision, "defer");
    assert.equal(report.permission.shadow.honor, false);
    assert.equal(report.permission.shadow.blocked, false);
    assert.equal(report.permission.active.decision, "deny");
    assert.equal(report.permission.active.honor, true);
    assert.equal(report.permission.uncertain.decision, "deny");
    assert.equal(report.permission.emptyFindings.reason, "empty_findings_not_approval");
    assert.equal(report.permission.emptyFindings.decision, "deny");
    assert.equal(report.permission.emptyFindings.approval, false);
    assert.equal(report.permission.keyAbsent.mode, "shadow");
    assert.equal(report.permission.keyAbsent.blocked, false);
    assert.equal(report.bypass.decision, "defer");
    assert.equal(report.bypass.reason, "bypass");
    assert.equal(report.notBypass.decision, "deny");
    assert.equal(report.irreversible.decision, "deny");
    assert.equal(report.irreversible.code, "F454");
    assert.equal(report.irreversible.initiated, false);
    assert.equal(report.uncertainActive.decision, "deny");
    assert.equal(report.uncertainShadow.decision, "defer");
    assert.equal(report.readStaysDefer.decision, "defer");
    assert.equal(report.loopStopWins.decision, "deny");
    assert.equal(JSON.stringify(report).includes('"decision":"allow"'), false);
    const docPath = [join(here, "GROK-BUILD.md"), join(here, "../../docs/GROK-BUILD.md")].find((path) =>
      existsSync(path),
    );
    assert.ok(docPath, "GROK-BUILD.md missing");
    const doc = readFileSync(docPath, "utf8");
    assert.match(doc, /node packages\/jev-router\/harness\.mjs smoke/);
    assert.match(doc, /JEV_BYPASS/);
    assert.match(doc, /JEV_PERMISSION_MODE/);
    assert.match(doc, /JEV_TYPED_CALL_MODE/);
    assert.match(doc, /JEV_MODE/);
    assert.match(doc, /shadow/);
    assert.match(doc, /not approval/);
  }),

  test("grok build hook maps a class to a policy id and fail-closes an empty verdict", () => {
    const dir = mkdtempSync(join(tmpdir(), "grok-harness-"));
    const router = join(dir, "router");
    const argvLog = join(dir, "argv");
    const called = join(dir, "called");
    writeFileSync(
      router,
      `#!/bin/sh
if [ "\${HARNESS_MARK:-}" = "poison" ]; then
  : > "\${HARNESS_CALLED_FILE:?}"
  exit 99
fi
printf '%s\\n' "$*" > "\${HARNESS_ARGV_LOG:-/dev/null}"
printf '%s\\n' '{"ok":true,"mode":"active","choice":"stop","gate":"hold","blocked":true,"exec":false,"bypass":false}'
`,
    );
    chmodSync(router, 0o755);
    const event = readFileSync(join(here, "testdata", "grok-pretool-bash.json"), "utf8");
    const baseEnv = {
      ...process.env,
      JEV_ROUTER: router,
      JEV_BYPASS: "",
      JEV_MODE: "active",
      JEV_PERMISSION_MODE: "",
      JEV_TYPED_CALL_MODE: "",
      TYPESAFE_API_KEY: "",
      HARNESS_ARGV_LOG: argvLog,
    };
    const hook = spawnSync(process.execPath, [join(here, "harness.mjs"), "hook"], {
      input: event,
      env: baseEnv,
      encoding: "utf8",
    });
    assert.equal(hook.status, 2, hook.stderr);
    const denied = JSON.parse(hook.stdout.trim());
    assert.equal(denied.decision, "deny");
    assert.match(denied.reason, /policy=omapi-loop-stop-policy@1/);
    assert.equal(denied.decision === "allow", false);
    const argv = readFileSync(argvLog, "utf8");
    assert.match(argv, /--tool-name/);
    assert.match(argv, /Bash/);
    assert.match(argv, /--tool-input/);
    assert.match(argv, /npm test/);

    const bypass = spawnSync(process.execPath, [join(here, "harness.mjs"), "hook"], {
      input: event,
      env: { ...baseEnv, JEV_BYPASS: "1", HARNESS_MARK: "poison", HARNESS_CALLED_FILE: called },
      encoding: "utf8",
    });
    assert.equal(bypass.status, 0, bypass.stderr);
    assert.deepEqual(JSON.parse(bypass.stdout.trim()), { decision: "defer", reason: "bypass" });
    assert.equal(existsSync(called), false);

    const yes = decideHook({
      event: { toolName: "Bash" },
      env: { JEV_BYPASS: "yes" },
      verdict: { mode: "active", choice: "stop", gate: "hold", blocked: true },
    });
    assert.equal(yes.decision, "deny");

    writeFileSync(join(dir, "garbage"), "#!/bin/sh\nprintf '%s\\n' 'not-json'\n");
    chmodSync(join(dir, "garbage"), 0o755);
    const activeEmpty = spawnSync(process.execPath, [join(here, "harness.mjs"), "hook"], {
      input: event,
      env: { ...baseEnv, JEV_ROUTER: join(dir, "garbage"), JEV_MODE: "active", JEV_BYPASS: "" },
      encoding: "utf8",
    });
    assert.equal(activeEmpty.status, 2, activeEmpty.stderr);
    assert.deepEqual(JSON.parse(activeEmpty.stdout.trim()), { decision: "deny", reason: "jev uncertain" });
    const shadowEmpty = spawnSync(process.execPath, [join(here, "harness.mjs"), "hook"], {
      input: event,
      env: { ...baseEnv, JEV_ROUTER: join(dir, "garbage"), JEV_MODE: "shadow", JEV_BYPASS: "" },
      encoding: "utf8",
    });
    assert.equal(shadowEmpty.status, 0, shadowEmpty.stderr);
    assert.deepEqual(JSON.parse(shadowEmpty.stdout.trim()), { decision: "defer", reason: "shadow" });

    const smoke = spawnSync(process.execPath, [join(here, "harness.mjs"), "smoke"], { encoding: "utf8" });
    assert.equal(smoke.status, 0, smoke.stderr);
    assert.equal(JSON.parse(smoke.stdout).ok, true);
    assert.equal(smoke.stdout.includes('"decision":"allow"'), false);

    const mark = ["omapi", "mark"].join("-");
    const banner = ["oma", "on"].join(" ");
    const quiet = (label, args, input) => {
      const run = spawnSync(process.execPath, [join(here, "harness.mjs"), ...args], {
        input,
        env: { ...baseEnv, JEV_BYPASS: "", JEV_MODE: "shadow" },
        encoding: "utf8",
      });
      const blob = `${run.stdout}\n${run.stderr}`;
      assert.equal(blob.includes(mark), false, label);
      assert.equal(blob.includes(banner), false, label);
      return run;
    };
    const catalog = quiet("catalog", ["catalog"]);
    assert.equal(catalog.status, 0, catalog.stderr);
    assert.equal(catalog.stderr, "");
    assert.equal(JSON.parse(catalog.stdout).kind, "tiny");
    const config = quiet("config", ["config"]);
    assert.equal(config.status, 0, config.stderr);
    assert.equal(config.stderr, "");
    rmSync(dir, { recursive: true, force: true });
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
