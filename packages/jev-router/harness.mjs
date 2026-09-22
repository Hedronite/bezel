#!/usr/bin/env node
/**
 * Grok Build harness for the merged Jev P0 controls.
 *
 * Boundary: Grok Build → this process → jev-router. This file does not
 * construct a TypeSafe client and does not keep a second class map.
 * Policy ids, the PreToolUse matcher, permission catalogs, the tiny index,
 * and FACET calls come from policy.mjs / permission.mjs / catalog.mjs / calls.mjs.
 *
 * Stdout of `hook` is only {decision, reason}. decision is defer or deny.
 * Shadow observes. A honoring surface denies. Uncertainty is not approval.
 * JEV_BYPASS is only the isJevBypass predicate ("1" or "true").
 *
 * Commands:
 *   hook      PreToolUse JSON on stdin → defer|deny
 *   catalog   tiny index (no JSON Schema)
 *   schema NAME
 *   config    printable hook JSON (matcher from PRETOOL_MATCHER)
 *   smoke     offline proof of the four controls
 */

import { spawnSync } from "node:child_process";
import { readFileSync, realpathSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { applyCallsVerdict, decideTypedCalls, effectiveTypedCallMode, normalizeCatalog } from "./calls.mjs";
import { schemaDumpCall, tinyCatalog } from "./catalog.mjs";
import { applyPermissionVerdict, decidePermission, effectivePermissionMode } from "./permission.mjs";
import {
  isJevBypass,
  matchPretoolClass,
  PRETOOL_CLASSES,
  PRETOOL_MATCHER,
  pretoolHookDecision,
  pretoolStamp,
} from "./policy.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const BASH_FIXTURE = join(here, "testdata", "grok-pretool-bash.json");
const MCP_FIXTURE = join(here, "testdata", "mcp-tools.json");
const INPUT_CAP = 4000;

export function harnessModes(env = {}) {
  const source = env && typeof env === "object" ? env : {};
  const hasKey = Boolean(source.TYPESAFE_API_KEY);
  return {
    jevMode: String(source.JEV_MODE || "shadow").toLowerCase() === "active" ? "active" : "shadow",
    permissionMode: effectivePermissionMode({ requested: source.JEV_PERMISSION_MODE, hasKey }),
    typedCallMode: effectiveTypedCallMode({ requested: source.JEV_TYPED_CALL_MODE, hasKey }),
    bypass: isJevBypass(source.JEV_BYPASS),
    hasKey,
  };
}

/** True when an empty or unreadable Jev answer must deny. */
export function honors(modes) {
  return modes.jevMode === "active" || modes.permissionMode === "active" || modes.typedCallMode === "active";
}

export function harnessConfig({ command = "grok-build-jev hook" } = {}) {
  return {
    hooks: {
      PreToolUse: [
        {
          matcher: PRETOOL_MATCHER,
          hooks: [{ type: "command", command, timeout: 30 }],
        },
      ],
    },
  };
}

export function parseHookEvent(text) {
  const raw = String(text || "").trim();
  if (!raw) return { ok: false, reason: "empty" };
  let value;
  try {
    value = JSON.parse(raw);
  } catch {
    return { ok: false, reason: "malformed" };
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) return { ok: false, reason: "malformed" };
  const toolName = String(value.toolName || value.tool_name || "");
  const toolInput = value.toolInput !== undefined ? value.toolInput : value.tool_input;
  const intent = String(value.intent || value.prompt || toolName || "");
  return {
    ok: true,
    event: {
      toolName,
      toolInput,
      intent,
      hookEventName: String(value.hookEventName || value.hook_event_name || ""),
    },
  };
}

export function toolInputText(event) {
  const stamp = pretoolStamp({ toolName: event && event.toolName ? event.toolName : "" });
  if (!stamp || !stamp.matched) return "";
  const input = event.toolInput;
  if (typeof input === "string") return input;
  if (!input || typeof input !== "object") return "";
  if (stamp.class === "shell") return String(input.command || input.cmd || "");
  if (stamp.class === "write") return String(input.path || input.file_path || input.filePath || "");
  return "";
}

function done(result) {
  return { ...result, shortCircuit: true };
}

function uncertain(modes, stamp) {
  if (honors(modes)) {
    return done({ decision: "deny", exitCode: 2, reason: "jev uncertain", action: "deny", pretool: stamp });
  }
  return done({ decision: "defer", exitCode: 0, reason: "shadow", action: "defer", pretool: stamp });
}

/**
 * Map one event plus an optional GateVerdict onto a Grok hook decision.
 * `verdict === undefined` means the router has not been asked yet.
 * A missing or unreadable verdict is uncertain: deny when a surface is
 * honoring, defer when every surface is shadow. Never `allow`.
 */
export function decideHook({ event = {}, env = {}, verdict } = {}) {
  const modes = harnessModes(env);
  const toolName = String(event.toolName || "");
  const stamp = toolName ? pretoolStamp({ toolName }) : null;
  if (modes.bypass) {
    return done({ decision: "defer", exitCode: 0, reason: "bypass", action: "defer", pretool: stamp });
  }
  if (!toolName) return uncertain(modes, stamp);
  if (!stamp || !stamp.matched) {
    return done({ decision: "defer", exitCode: 0, reason: "unmatched", action: "defer", pretool: stamp });
  }
  if (verdict === undefined) {
    return { decision: null, exitCode: null, reason: "needs_router", action: null, pretool: stamp, shortCircuit: false };
  }
  if (verdict === null || typeof verdict !== "object" || Array.isArray(verdict)) return uncertain(modes, stamp);
  const withStamp = verdict.pretool && verdict.pretool.policyId ? verdict : { ...verdict, pretool: stamp };
  const decision = pretoolHookDecision(withStamp);
  if (decision.decision !== "defer" && decision.decision !== "deny") return uncertain(modes, stamp);
  return done({ ...decision, pretool: withStamp.pretool });
}

function scrubText(text, env) {
  const secret = env && typeof env.TYPESAFE_API_KEY === "string" ? env.TYPESAFE_API_KEY : "";
  if (secret.length < 8) return String(text || "");
  return String(text || "").split(secret).join("[redacted]");
}

function spawnRouter(event, env) {
  const bin = (env && env.JEV_ROUTER) || "jev-router";
  const intent = scrubText(event.intent || event.toolName || "", env).slice(0, INPUT_CAP);
  const toolInput = scrubText(toolInputText(event), env).slice(0, INPUT_CAP);
  const argv = ["--tool-name", event.toolName, "--intent", intent || event.toolName];
  if (toolInput.trim()) argv.push("--tool-input", toolInput);
  if (env && env.JEV_TOOLS_FILE) argv.push("--tools-file", env.JEV_TOOLS_FILE);
  if (env && env.JEV_CALLS_TOP) argv.push("--top", String(env.JEV_CALLS_TOP));
  const run = spawnSync(bin, argv, {
    env,
    encoding: "utf8",
    input: "",
    maxBuffer: 8 * 1024 * 1024,
  });
  const stdout = scrubText(run.stdout || "", env);
  const line = stdout.trim().split("\n").at(-1) || "";
  if (!line) return { ok: false, reason: "empty_verdict", stderr: scrubText(run.stderr || "", env) };
  try {
    const verdict = JSON.parse(line);
    if (!verdict || typeof verdict !== "object" || Array.isArray(verdict)) {
      return { ok: false, reason: "malformed_verdict", stderr: scrubText(run.stderr || "", env) };
    }
    return { ok: true, verdict, stderr: scrubText(run.stderr || "", env) };
  } catch {
    return { ok: false, reason: "malformed_verdict", stderr: scrubText(run.stderr || "", env) };
  }
}

export function runHook(text, env = process.env) {
  const parsed = parseHookEvent(text);
  const event = parsed.ok ? parsed.event : { toolName: "" };
  const early = decideHook({ event, env });
  if (early.shortCircuit) return early;
  const run = spawnRouter(event, env);
  if (run.stderr) process.stderr.write(run.stderr.endsWith("\n") ? run.stderr : `${run.stderr}\n`);
  if (!run.ok) return decideHook({ event, env, verdict: null });
  return decideHook({ event, env, verdict: run.verdict });
}

function expect(cond, message) {
  if (!cond) throw new Error(message);
}

function hookOf(event, env, verdict) {
  const decision = decideHook({ event, env, verdict });
  expect(decision.decision === "defer" || decision.decision === "deny", "hook decision must be defer or deny");
  expect(decision.decision !== "allow", "uncertainty is not approval");
  return decision;
}

function shellDenyAnswer() {
  return {
    label: "deny",
    confidence: 0.9,
    probabilities: { allow: 0.05, deny: 0.85, ask: 0.1 },
  };
}

/**
 * Offline proof. No TypeSafe client, no network.
 * PreToolUse class → policy id, tiny discovery, MCP intent → FACET tool_call,
 * permission observed in shadow and enforced when that surface is active.
 */
export function runSmoke() {
  const bashEvent = parseHookEvent(readFileSync(BASH_FIXTURE, "utf8"));
  expect(bashEvent.ok, "bash fixture");
  const bashStamp = pretoolStamp({ toolName: bashEvent.event.toolName });
  const bashClass = matchPretoolClass(bashEvent.event.toolName);
  expect(bashStamp && bashStamp.matched && bashStamp.policyId === bashClass.policyId, "fixture stamp");
  expect(bashStamp.class === "shell", "Bash is shell");

  const classes = ["Bash", "search_replace", "linear__list_issues"].map((toolName) => {
    const stamp = pretoolStamp({ toolName });
    expect(stamp && stamp.matched && stamp.policyId === matchPretoolClass(toolName).policyId, toolName);
    return { toolName, class: stamp.class, policyId: stamp.policyId };
  });
  expect(new Set(classes.map((row) => row.policyId)).size === 3, "three existing policy ids");

  const catalog = tinyCatalog();
  const catalogBlob = JSON.stringify(catalog);
  expect(catalog.kind === "tiny", "tiny catalog");
  expect(!catalogBlob.includes("$schema"), "catalog has no schema");
  expect(!catalogBlob.includes('"decision":"allow"'), "catalog does not allow");
  expect(catalogBlob.length < 900, "catalog stays compact");
  expect(catalog.entries.length === PRETOOL_CLASSES.length, "index matches the class table");

  const mcpDump = schemaDumpCall("linear__list_issues");
  expect(mcpDump.decision === "defer" && mcpDump.autoAllow === false && mcpDump.typedCall === false, "schema dump is not a call");

  const tools = normalizeCatalog(JSON.parse(readFileSync(MCP_FIXTURE, "utf8")));
  expect(tools.error == null && tools.tools.length === 2, "mcp fixture");
  const readCalls = decideTypedCalls({
    catalog: tools,
    requestedMode: "active",
    hasKey: true,
    topX: 3,
    answer: {
      choice: "linear__list_issues",
      confidence: 0.9,
      probabilities: { linear__list_issues: 0.85, linear__save_issue: 0.15 },
    },
  });
  expect(readCalls.transport === "facet" && readCalls.facet && readCalls.facet.op === "tool_call", "typed call");
  expect(readCalls.initiated === false && readCalls.autoAllow === false, "call is not initiated");
  expect(readCalls.policyId === matchPretoolClass("linear__list_issues").policyId, "call uses the mcp policy id");

  const readPerm = decidePermission({
    classId: "mcp",
    requestedMode: "active",
    hasKey: true,
    routingOutcome: "check",
    typed: readCalls,
  });
  const readVerdict = applyPermissionVerdict(
    applyCallsVerdict(
      {
        mode: "shadow",
        choice: "continue",
        gate: "auto",
        blocked: false,
        exec: true,
        pretool: pretoolStamp({ toolName: "linear__list_issues" }),
      },
      readCalls,
    ),
    readPerm,
  );
  const readHook = hookOf({ toolName: "linear__list_issues" }, {}, readVerdict);
  expect(readHook.decision === "defer" && readHook.reason === "exec", "a read call defers; it does not allow");

  const stopped = applyCallsVerdict(
    {
      mode: "active",
      choice: "stop",
      gate: "hold",
      blocked: true,
      exec: false,
      pretool: pretoolStamp({ toolName: "linear__list_issues" }),
    },
    readCalls,
  );
  const loopStopWins = hookOf({ toolName: "linear__list_issues" }, {}, stopped);
  expect(loopStopWins.decision === "deny", "a typed continue does not clear a loop stop");

  const irreversibleCalls = decideTypedCalls({
    catalog: tools,
    requestedMode: "active",
    hasKey: true,
    answer: {
      choice: "linear__save_issue",
      confidence: 0.9,
      probabilities: { linear__save_issue: 0.85, linear__list_issues: 0.15 },
    },
  });
  expect(irreversibleCalls.code === "F454" && irreversibleCalls.mapped === "stop", "write effect denies");
  expect(irreversibleCalls.initiated === false && irreversibleCalls.autoPromote === false, "denied winner stays put");
  expect(irreversibleCalls.best == null, "denied winner is not replaced");
  const irreversibleVerdict = applyCallsVerdict(
    {
      mode: "shadow",
      choice: "continue",
      gate: "auto",
      blocked: false,
      exec: true,
      pretool: pretoolStamp({ toolName: "linear__save_issue" }),
    },
    irreversibleCalls,
  );
  const irreversibleHook = hookOf({ toolName: "linear__save_issue" }, {}, irreversibleVerdict);
  expect(irreversibleHook.decision === "deny", "irreversible call is denied when honored");

  const observed = decidePermission({
    classId: "shell",
    requestedMode: "shadow",
    hasKey: true,
    contentPresent: true,
    ...shellDenyAnswer(),
  });
  expect(observed.honor === false && observed.blocked === false && observed.mode === "shadow", "shadow observes");
  const observedVerdict = applyPermissionVerdict(
    {
      mode: "shadow",
      choice: "continue",
      gate: "auto",
      blocked: false,
      exec: true,
      pretool: bashStamp,
    },
    observed,
  );
  const shadowHook = hookOf(bashEvent.event, { JEV_MODE: "shadow" }, observedVerdict);
  expect(shadowHook.decision === "defer", "shadow permission does not block");

  const enforced = decidePermission({
    classId: "shell",
    requestedMode: "active",
    hasKey: true,
    contentPresent: true,
    ...shellDenyAnswer(),
  });
  expect(enforced.honor === true && enforced.blocked === true && enforced.choice === "stop", "active enforces deny");
  const enforcedVerdict = applyPermissionVerdict(
    {
      mode: "shadow",
      choice: "continue",
      gate: "auto",
      blocked: false,
      exec: true,
      pretool: bashStamp,
    },
    enforced,
  );
  const activeHook = hookOf(bashEvent.event, {}, enforcedVerdict);
  expect(activeHook.decision === "deny" && activeHook.pretool.policyId === bashStamp.policyId, "active deny keeps the policy id");

  const shaky = decidePermission({
    classId: "shell",
    requestedMode: "active",
    hasKey: true,
    contentPresent: true,
    label: "allow",
    confidence: 0.4,
    probabilities: { allow: 0.4, deny: 0.35, ask: 0.25 },
  });
  expect(shaky.label === "ask" && shaky.mapped === "escalate", "uncertain allow is ask");
  const shakyHook = hookOf(
    bashEvent.event,
    {},
    applyPermissionVerdict(
      { mode: "shadow", choice: "continue", gate: "auto", blocked: false, exec: true, pretool: bashStamp },
      shaky,
    ),
  );
  expect(shakyHook.decision === "deny", "uncertainty is not approval");

  const emptyWrite = decidePermission({
    classId: "write",
    requestedMode: "active",
    hasKey: true,
    diffText: "",
    path: "src/app.js",
  });
  expect(emptyWrite.reason === "empty_findings_not_approval" && emptyWrite.label === "ask", "empty findings are ask");
  const emptyHook = hookOf(
    { toolName: "search_replace" },
    {},
    applyPermissionVerdict(
      {
        mode: "shadow",
        choice: "continue",
        gate: "auto",
        blocked: false,
        exec: true,
        pretool: pretoolStamp({ toolName: "search_replace" }),
      },
      emptyWrite,
    ),
  );
  expect(emptyHook.decision === "deny", "empty findings do not allow");

  const keyAbsent = decidePermission({
    classId: "shell",
    requestedMode: "active",
    hasKey: false,
    contentPresent: true,
    ...shellDenyAnswer(),
  });
  expect(keyAbsent.mode === "shadow" && keyAbsent.honor === false && keyAbsent.blocked === false && keyAbsent.missingKey === true, "key absent stays shadow");

  const modes = harnessModes({ JEV_MODE: "active", JEV_PERMISSION_MODE: "yes", JEV_TYPED_CALL_MODE: "true" });
  expect(modes.jevMode === "active" && modes.permissionMode === "shadow" && modes.typedCallMode === "shadow", "jev active does not flip the other surfaces");
  expect(harnessModes({ JEV_PERMISSION_MODE: "active" }).permissionMode === "shadow", "permission active needs a key");
  expect(effectiveTypedCallMode({ requested: "active", hasKey: false }) === "shadow", "calls active needs a key");

  const bypass = hookOf(
    bashEvent.event,
    { JEV_BYPASS: "1" },
    { mode: "active", choice: "stop", gate: "hold", blocked: true },
  );
  expect(bypass.decision === "defer" && bypass.reason === "bypass", "bypass defers");
  const notBypass = hookOf(
    bashEvent.event,
    { JEV_BYPASS: "yes" },
    { mode: "active", choice: "stop", gate: "hold", blocked: true, pretool: bashStamp },
  );
  expect(notBypass.decision === "deny", "yes is not a bypass");

  const uncertainActive = hookOf(bashEvent.event, { JEV_MODE: "active" }, null);
  const uncertainShadow = hookOf(bashEvent.event, { JEV_MODE: "shadow" }, null);
  expect(uncertainActive.decision === "deny" && uncertainActive.reason === "jev uncertain", "active empty verdict denies");
  expect(uncertainShadow.decision === "defer" && uncertainShadow.reason === "shadow", "shadow empty verdict stays shadow");

  const unmapped = hookOf({ toolName: "read_file" }, { JEV_MODE: "active" }, { choice: "continue", gate: "auto" });
  expect(unmapped.decision === "defer" && unmapped.reason === "unmatched", "unmapped tools are outside the policy map");

  const config = harnessConfig();
  expect(config.hooks.PreToolUse.length === 1 && config.hooks.PreToolUse[0].matcher === PRETOOL_MATCHER, "config matcher");
  const configBlob = JSON.stringify(config);
  expect(!configBlob.includes("TYPESAFE_API_KEY") && !configBlob.includes("JEV_MODE"), "config sets neither key nor mode");

  const report = {
    ok: true,
    pretool: {
      toolName: bashStamp.toolName,
      class: bashStamp.class,
      policyId: bashStamp.policyId,
      matched: true,
    },
    classes,
    catalog: {
      kind: catalog.kind,
      entries: catalog.entries.length,
      bytes: catalogBlob.length,
      schemaInline: false,
    },
    schemaDump: {
      tool: "linear__list_issues",
      decision: mcpDump.decision,
      typedCall: mcpDump.typedCall,
      autoAllow: mcpDump.autoAllow,
    },
    calls: {
      transport: readCalls.transport,
      op: readCalls.facet.op,
      initiated: readCalls.initiated,
      policyId: readCalls.policyId,
      autoAllow: readCalls.autoAllow,
    },
    permission: {
      shadow: { decision: shadowHook.decision, honor: observed.honor, blocked: observed.blocked },
      active: { decision: activeHook.decision, honor: enforced.honor, blocked: enforced.blocked },
      uncertain: { decision: shakyHook.decision },
      emptyFindings: {
        reason: emptyWrite.reason,
        decision: emptyHook.decision,
        approval: false,
      },
      keyAbsent: {
        mode: keyAbsent.mode,
        honor: keyAbsent.honor,
        blocked: keyAbsent.blocked,
        missingKey: true,
      },
    },
    bypass: { decision: bypass.decision, reason: bypass.reason },
    notBypass: { decision: notBypass.decision },
    irreversible: {
      decision: irreversibleHook.decision,
      code: irreversibleCalls.code,
      initiated: irreversibleCalls.initiated,
      autoPromote: irreversibleCalls.autoPromote,
    },
    uncertainActive: { decision: uncertainActive.decision, reason: uncertainActive.reason },
    uncertainShadow: { decision: uncertainShadow.decision, reason: uncertainShadow.reason },
    readStaysDefer: { decision: readHook.decision },
    loopStopWins: { decision: loopStopWins.decision },
    unmapped: { decision: unmapped.decision, reason: unmapped.reason },
  };
  const found = [];
  const walk = (value) => {
    if (!value || typeof value !== "object") return;
    if (value.decision === "allow") found.push("allow");
    for (const item of Object.values(value)) walk(item);
  };
  walk(report);
  expect(found.length === 0, "smoke must not allow");
  return report;
}

function emitDecision(result) {
  const stamp = result.pretool;
  const cls = stamp && stamp.class ? stamp.class : "-";
  const policy = stamp && stamp.policyId ? stamp.policyId : "-";
  process.stderr.write(
    `[grok-build-jev] class=${cls} policy=${policy} decision=${result.decision} reason=${result.reason}\n`,
  );
  process.stdout.write(`${JSON.stringify({ decision: result.decision, reason: result.reason })}\n`);
  process.exit(result.exitCode);
}

function main() {
  const cmd = process.argv[2] || "hook";
  if (cmd === "--help" || cmd === "-h" || cmd === "help") {
    process.stderr.write("grok-build-jev hook|catalog|schema NAME|config|smoke\n");
    process.exit(0);
  }
  if (cmd === "smoke") {
    try {
      process.stdout.write(`${JSON.stringify(runSmoke())}\n`);
      process.exit(0);
    } catch (err) {
      process.stderr.write(`[grok-build-jev] smoke failed: ${err && err.message ? err.message : err}\n`);
      process.exit(2);
    }
  }
  if (cmd === "catalog") {
    process.stdout.write(`${JSON.stringify(tinyCatalog())}\n`);
    process.exit(0);
  }
  if (cmd === "schema" || cmd === "schema-dump") {
    const name = process.argv[3];
    if (!name || name.startsWith("-")) {
      process.stdout.write(`${JSON.stringify(schemaDumpCall(""))}\n`);
      process.exit(2);
    }
    const dumped = schemaDumpCall(name);
    process.stdout.write(`${JSON.stringify(dumped)}\n`);
    process.exit(dumped.decision === "deny" ? 2 : 0);
  }
  if (cmd === "config") {
    process.stdout.write(`${JSON.stringify(harnessConfig(), null, 2)}\n`);
    process.exit(0);
  }
  if (cmd !== "hook") {
    process.stderr.write(`[grok-build-jev] unknown command ${cmd}\n`);
    process.exit(2);
  }
  emitDecision(runHook(readFileSync(0, "utf8"), process.env));
}

function isMainModule() {
  const entry = process.argv[1];
  if (!entry) return false;
  try {
    return realpathSync(entry) === realpathSync(fileURLToPath(import.meta.url));
  } catch {
    return false;
  }
}

if (isMainModule()) main();
