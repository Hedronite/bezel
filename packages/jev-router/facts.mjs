/**
 * Deterministic facts + available() gates. Hard eligibility before Jev route.
 * No Stanley binary. review is a routing stub this spike (not implemented).
 */

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { CHECK_POLICY } from "./policy.mjs";

export const WORKFLOWS = {
  check: {
    id: "check",
    implemented: true,
    available: (facts) => facts.diffPresent === true,
    unavailableReason: "check requires a git diff",
  },
  review: {
    id: "review",
    implemented: false,
    available: (facts) => facts.diffPresent === true,
    unavailableReason: "review requires a git diff (stub; not implemented this spike)",
  },
};

export function diffPresent(diffText) {
  return String(diffText || "").trim().length > 0;
}

export function availableCapabilities(facts) {
  const capabilities = {};
  const unavailable = {};
  for (const workflow of Object.values(WORKFLOWS)) {
    const ok = workflow.available(facts);
    capabilities[workflow.id] = ok;
    if (!ok) unavailable[workflow.id] = workflow.unavailableReason;
  }
  return { capabilities, unavailable };
}

export function availableIds(facts) {
  return Object.entries(availableCapabilities(facts).capabilities)
    .filter(([, ok]) => ok)
    .map(([id]) => id);
}

function runGit(repo, args, exec = execFileSync) {
  return exec("git", args, {
    cwd: repo,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    timeout: 15_000,
  });
}

/**
 * Read a unified diff from a file, git, or injected text.
 * Secrets stay out of argv: TYPESAFE_API_KEY is never read here.
 */
export function gatherDiff({
  repo = process.cwd(),
  base = "",
  diffFile = "",
  diffText = null,
  exec = execFileSync,
} = {}) {
  if (diffText != null) {
    const text = String(diffText);
    return { text, present: diffPresent(text), source: "injected", error: null };
  }
  if (diffFile) {
    if (!existsSync(diffFile)) {
      return { text: "", present: false, source: "file", error: `diff file not found: ${diffFile}` };
    }
    const text = readFileSync(diffFile, "utf8");
    return { text, present: diffPresent(text), source: "file", error: null };
  }

  try {
    const args = ["diff", "--no-color", "--no-ext-diff"];
    if (base) args.push(base);
    const vsBaseOrWorktree = runGit(repo, args, exec);
    const staged = base ? "" : runGit(repo, ["diff", "--cached", "--no-color", "--no-ext-diff"], exec);
    const text = [vsBaseOrWorktree, staged].filter((part) => part && part.trim()).join("\n");
    return { text, present: diffPresent(text), source: base ? `git-diff:${base}` : "git-worktree", error: null };
  } catch (err) {
    const message = err && err.message ? err.message : String(err);
    return { text: "", present: false, source: "git", error: message };
  }
}

/** Clip write evidence to the check policy hunk cap. Default is 4000. */
export function clipToMaxHunkChars(text, max = CHECK_POLICY.maxHunkChars) {
  const n = Number(max);
  const cap = Number.isFinite(n) && n >= 0 ? Math.trunc(n) : CHECK_POLICY.maxHunkChars;
  return String(text ?? "").slice(0, cap);
}

function writePath(input) {
  return String(input.path || input.file_path || input.filePath || "").replace(/[\r\n]/g, "");
}

function hasOwn(obj, key) {
  return Object.prototype.hasOwnProperty.call(obj, key);
}

function diffHeader(path, created) {
  const name = path || "unknown";
  return [`diff --git a/${name} b/${name}`, created ? "--- /dev/null" : `--- a/${name}`, `+++ b/${name}`];
}

function prefixedLines(prefix, value) {
  return String(value ?? "")
    .split(/\r?\n/)
    .map((line) => `${prefix}${line}`);
}

/**
 * Unified diff of a write tool body. Empty when the input is only a path.
 * This is not the git worktree. `--check` still uses gatherDiff.
 */
export function proposedWriteDiff(input) {
  if (typeof input === "string") return { path: input, text: "" };
  if (!input || typeof input !== "object" || Array.isArray(input)) return { path: "", text: "" };
  const path = writePath(input);
  if (Array.isArray(input.edits)) {
    if (input.edits.length === 0) return { path, text: "" };
    const parts = diffHeader(path, false);
    for (const edit of input.edits) {
      const row = edit && typeof edit === "object" ? edit : {};
      parts.push("@@", ...prefixedLines("-", row.old_string ?? ""), ...prefixedLines("+", row.new_string ?? ""));
    }
    return { path, text: parts.join("\n") };
  }
  if (hasOwn(input, "old_string") || hasOwn(input, "new_string")) {
    const parts = diffHeader(path, false);
    parts.push("@@", ...prefixedLines("-", input.old_string ?? ""), ...prefixedLines("+", input.new_string ?? ""));
    return { path, text: parts.join("\n") };
  }
  if (hasOwn(input, "contents")) {
    const parts = diffHeader(path, true);
    parts.push("@@", ...prefixedLines("+", input.contents ?? ""));
    return { path, text: parts.join("\n") };
  }
  return { path, text: "" };
}

/**
 * Hook wire for a write. A body becomes JSON `{path, proposed}` clipped to
 * maxHunkChars. A path with no body stays the path string.
 */
export function writeToolWire(input, max = CHECK_POLICY.maxHunkChars) {
  const { path, text } = proposedWriteDiff(input);
  if (!text) return path;
  return JSON.stringify({ path, proposed: clipToMaxHunkChars(text, max) });
}

/** Inverse of writeToolWire. Non-JSON input is a path and an empty diff. */
export function parseWriteToolWire(raw, max = CHECK_POLICY.maxHunkChars) {
  const text = String(raw ?? "");
  if (text.startsWith("{")) {
    try {
      const parsed = JSON.parse(text);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed) && typeof parsed.proposed === "string") {
        return {
          path: String(parsed.path || "").replace(/[\r\n]/g, ""),
          diffText: clipToMaxHunkChars(parsed.proposed, max),
        };
      }
    } catch {
      /* A path, not an envelope. */
    }
  }
  return { path: text, diffText: "" };
}

export function gatherFacts(opts = {}) {
  const diff = gatherDiff(opts);
  const { capabilities, unavailable } = availableCapabilities({ diffPresent: diff.present });
  return {
    diffPresent: diff.present,
    diffSource: diff.source,
    diffError: diff.error,
    capabilities,
    unavailable,
    available: Object.entries(capabilities)
      .filter(([, ok]) => ok)
      .map(([id]) => id),
  };
}
