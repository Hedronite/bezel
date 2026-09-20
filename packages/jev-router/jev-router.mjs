#!/usr/bin/env node
/**
 * Router-only Jev Choice gate (no jev-lab vendor, no config secrets).
 *
 * Runtime env — never baked at build, never read from this package:
 *   TYPESAFE_API_KEY  host card store / export only
 *   JEV_MODE          shadow (default) | active
 *   JEV_BYPASS        1|true skips the Choice call
 *   JEV_MODEL         optional, default jev-latest
 *   JEV_INTENT        fallback when no argv intent
 *
 * Shadow: log Choice, never block.
 * Active: honor gate=auto only (else exit 2).
 */

import { choice, TypeSafeClient } from "@typesafe-ai/sdk";

function parseIntent(argv) {
  const args = argv.slice(2);
  if (args[0] === "--intent" && args[1]) {
    return args[1];
  }
  if (args[0] && !args[0].startsWith("-")) {
    return args[0];
  }
  return process.env.JEV_INTENT || "";
}

function emit(obj) {
  process.stdout.write(`${JSON.stringify(obj)}\n`);
}

function log(msg) {
  process.stderr.write(`[jev-router] ${msg}\n`);
}

const mode = String(process.env.JEV_MODE || "shadow").toLowerCase();
const bypass = process.env.JEV_BYPASS === "1" || process.env.JEV_BYPASS === "true";
const intent = parseIntent(process.argv);

if (bypass) {
  log("JEV_BYPASS set — skip Choice");
  emit({
    ok: true,
    mode,
    bypass: true,
    choice: "bypass",
    gate: "auto",
    blocked: false,
    intent,
  });
  process.exit(0);
}

const hasKey = Boolean(process.env.TYPESAFE_API_KEY);
if (!hasKey) {
  if (mode === "shadow") {
    log("TYPESAFE_API_KEY unset — shadow continues unclassified (does not block)");
    emit({
      ok: true,
      mode,
      bypass: false,
      missingKey: true,
      choice: "unclassified",
      gate: "auto",
      blocked: false,
      intent,
    });
    process.exit(0);
  }
  log("TYPESAFE_API_KEY unset — active mode fail-closed");
  emit({
    ok: false,
    mode,
    missingKey: true,
    choice: "unclassified",
    gate: "hold",
    blocked: true,
    intent,
  });
  process.exit(2);
}

const client = new TypeSafeClient();

try {
  const response = await client.systemOne({
    model: process.env.JEV_MODEL || "jev-latest",
    state: { intent },
    questions: {
      route: choice("Which writer lane should handle `intent`?", {
        cursor_default: "Default Cursor/omp writer (Kimi/GLM via host Cursor auth)",
        hold_for_human: "Needs a human before any writer runs",
        other: "None of the listed lanes",
      }),
      gate: choice("What AutoMode / tool gate applies to `intent`?", {
        auto: "Safe to proceed with the requested agent tools",
        hold: "Hold — risky or unclear; do not auto-run tools",
      }),
    },
  });

  const route = response.answers.route;
  const gateAns = response.answers.gate;
  const gate = gateAns.choice;
  const blocked = mode === "active" && gate !== "auto";

  log(`Choice route=${route.choice} gate=${gate} mode=${mode} blocked=${blocked}`);
  emit({
    ok: true,
    mode,
    bypass: false,
    choice: route.choice,
    gate,
    confidence: {
      route: route.confidence ?? null,
      gate: gateAns.confidence ?? null,
    },
    blocked,
    intent,
  });
  process.exit(blocked ? 2 : 0);
} catch (err) {
  const message = err && err.message ? err.message : String(err);
  log(`Choice call failed: ${message}`);
  if (mode === "shadow") {
    emit({
      ok: false,
      mode,
      error: message,
      choice: "unclassified",
      gate: "auto",
      blocked: false,
      intent,
    });
    process.exit(0);
  }
  emit({
    ok: false,
    mode,
    error: message,
    choice: "unclassified",
    gate: "hold",
    blocked: true,
    intent,
  });
  process.exit(2);
}
