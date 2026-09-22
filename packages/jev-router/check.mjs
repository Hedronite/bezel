/**
 * Thin check workflow: git diff evidence → optional Jev judge → code thresholds
 * → { findings, parked, notChecked }. Empty findings are not approval.
 *
 * Deterministic pre-pass is adapted from Stanley's check-task idea (skip markers,
 * removed assertions) — patterns only; no stanley-code import.
 */

import { applyCheckThresholds, CHECK_POLICY } from "./policy.mjs";

const SKIP_MARKERS = [
  /\b(?:it|test|describe)\.skip\s*\(/,
  /\b(?:xit|xtest|xdescribe)\s*\(/,
  /@pytest\.mark\.(?:skip|skipif|xfail)\b/,
  /\bpytest\.(?:skip|xfail)\s*\(/,
];

const ASSERTION = /\b(?:expect|assert|assertEqual|XCTAssert)\b/;

const LOCKFILE = /(^|\/)(package-lock\.json|pnpm-lock\.yaml|yarn\.lock|flake\.lock|Cargo\.lock|go\.sum)$/;
const SECRET_PATH = /(^|\/)(\.env(\..+)?|id_rsa|id_ed25519|credentials(\.json)?|.*\.(pem|p12|key))$/i;
const TEST_PATH = /(\.(test|spec)\.[cm]?[jt]sx?$|_test\.go$|(^|\/)(tests?|__tests__)\/)/i;

export function fileKind(path) {
  const p = String(path || "");
  if (LOCKFILE.test(p)) return "lockfile";
  if (SECRET_PATH.test(p)) return "secret";
  if (TEST_PATH.test(p)) return "test";
  if (/(^|\/)\.github\/workflows\//.test(p)) return "ci";
  if (/\.(md|rst)$/i.test(p)) return "docs";
  return "source";
}

export function parseUnifiedDiff(text) {
  const lines = String(text || "").split(/\r?\n/);
  const hunks = [];
  let path = "";
  let buf = [];
  let header = "";
  let gone = false;

  const flush = () => {
    if (!header && buf.length === 0) return;
    const body = buf.join("\n");
    const id = `h${hunks.length + 1}`;
    const goneHeader = gone ? "+++ /dev/null" : "";
    const goneText = gone ? "deleted file mode" : "";
    hunks.push({
      id,
      path: path || "unknown",
      header: [goneHeader, header].filter(Boolean).join("\n"),
      text: [goneText, goneHeader, header, body].filter(Boolean).join("\n"),
      deleted: gone,
      added: buf.filter((line) => line.startsWith("+") && !line.startsWith("+++")),
      removed: buf.filter((line) => line.startsWith("-") && !line.startsWith("---")),
    });
    buf = [];
    header = "";
  };

  for (const line of lines) {
    if (line.startsWith("diff --git ")) {
      flush();
      gone = false;
      const match = line.match(/diff --git a\/(.+) b\/(.+)/);
      path = match ? match[2] : path;
      continue;
    }
    if (line.startsWith("deleted file mode ")) {
      gone = true;
      continue;
    }
    if (line.startsWith("+++ ")) {
      const plus = line.slice(4);
      if (plus === "/dev/null") gone = true;
      else path = plus.replace(/^b\//, "");
      continue;
    }
    if (line.startsWith("@@")) {
      flush();
      header = line;
      continue;
    }
    if (header) buf.push(line);
  }
  flush();
  return hunks;
}

export function deterministicFlags(hunk) {
  const kind = fileKind(hunk.path);
  const added = hunk.added || [];
  const removed = hunk.removed || [];
  const squash = (rows) => rows.join("").replace(/\s+/g, "");
  const flags = [];

  if (kind === "lockfile") flags.push("lockfile_changed");
  if (kind === "ci") flags.push("ci_config_changed");
  if (kind === "secret") flags.push("secret_path");
  if ((added.length > 0 || removed.length > 0) && squash(added) === squash(removed)) {
    flags.push("formatting_only");
  }

  const skipAdded = added.filter((line) => SKIP_MARKERS.some((re) => re.test(line))).length;
  const skipRemoved = removed.filter((line) => SKIP_MARKERS.some((re) => re.test(line))).length;
  if (skipAdded > skipRemoved) flags.push("skip_marker_added");

  const assertRemoved = removed.filter((line) => ASSERTION.test(line)).length;
  const assertAdded = added.filter((line) => ASSERTION.test(line)).length;
  if (kind === "test" && assertRemoved > assertAdded) flags.push("assertions_removed");

  const deleted =
    hunk.deleted === true ||
    /(?:^|\n)\+\+\+ \/dev\/null(?:\n|$)/.test(hunk.header || "") ||
    /deleted file mode/i.test(hunk.text || "");
  if (kind === "test" && deleted) flags.push("test_file_deleted");

  return { kind, flags };
}

const WARN_FLAGS = new Set(["skip_marker_added", "assertions_removed", "test_file_deleted", "secret_path"]);

function finding(flag, hunk, source, extra = {}) {
  return {
    flag,
    id: hunk.id,
    source,
    severity: WARN_FLAGS.has(flag) ? "warn" : "info",
    path: hunk.path,
    ...extra,
  };
}

/**
 * Run the check workflow over a unified diff.
 * `judge` is optional: ({ hunk, intent }) => judgment | null.
 * Missing judge / missing key: hunks go to notChecked; shadow continues.
 */
export async function runCheck({
  diffText,
  intent = "",
  judge = null,
  missingKey = false,
  mode = "shadow",
  policy = CHECK_POLICY,
} = {}) {
  const notChecked = [
    "empty findings are not approval",
    "tests were not executed",
    "correctness of the change",
  ];
  const findings = [];
  const parked = [];

  const allHunks = parseUnifiedDiff(diffText);
  if (allHunks.length === 0) {
    notChecked.push("no git diff — check unavailable");
    return envelope({
      status: "no_diff",
      findings,
      parked,
      notChecked,
      missingKey,
      mode,
      intent,
      hunkCount: 0,
      judged: 0,
    });
  }

  if (allHunks.length > policy.maxHunks) {
    notChecked.push(
      `only the first ${policy.maxHunks} of ${allHunks.length} hunks are eligible (policy hunk limit)`,
    );
  }

  const eligible = allHunks.slice(0, policy.maxHunks);
  let judged = 0;

  for (const hunk of eligible) {
    const { kind, flags } = deterministicFlags(hunk);
    for (const flag of flags) {
      findings.push(finding(flag, hunk, "deterministic"));
    }

    if (kind === "lockfile") {
      notChecked.push(`${hunk.id} ${hunk.path}: lockfile settled in code (not sent to Jev)`);
      continue;
    }
    if (kind === "secret") {
      notChecked.push(`${hunk.id} ${hunk.path}: secret-shaped path skipped`);
      continue;
    }

    if (!judge || missingKey) {
      notChecked.push(
        missingKey
          ? `${hunk.id} ${hunk.path}: TYPESAFE_API_KEY unset — unjudged (shadow continues)`
          : `${hunk.id} ${hunk.path}: no Jev judge — unjudged`,
      );
      continue;
    }

    let judgment;
    try {
      judgment = await judge({ hunk, intent, kind, flags });
    } catch (err) {
      const message = err && err.message ? err.message : String(err);
      notChecked.push(`${hunk.id} ${hunk.path}: judge failed — ${message}`);
      continue;
    }

    if (!judgment) {
      notChecked.push(`${hunk.id} ${hunk.path}: judge returned no answer`);
      continue;
    }

    judged += 1;
    const decision = applyCheckThresholds(judgment, policy);
    if (decision.bucket === "finding") {
      findings.push(finding(decision.flag, hunk, "jev", { concern: decision.concern, kind: decision.kind }));
    } else if (decision.bucket === "parked") {
      parked.push({
        id: hunk.id,
        path: hunk.path,
        reason: decision.reason,
        concern: decision.concern,
        kind: decision.kind,
      });
    }
  }

  return envelope({
    status: missingKey || !judge ? "incomplete" : "complete",
    findings,
    parked,
    notChecked,
    missingKey,
    mode,
    intent,
    hunkCount: allHunks.length,
    judged,
  });
}

function envelope({ status, findings, parked, notChecked, missingKey, mode, intent, hunkCount, judged }) {
  return {
    ok: true,
    workflow: "check",
    policy: CHECK_POLICY.version,
    mode,
    missingKey,
    approval: false,
    emptyFindingsAreNotApproval: true,
    status,
    findings,
    parked,
    notChecked,
    facts: { diffPresent: hunkCount > 0, hunkCount, judged },
    intent,
  };
}
