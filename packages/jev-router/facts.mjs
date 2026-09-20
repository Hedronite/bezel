/**
 * Deterministic facts + available() gates. Hard eligibility before Jev route.
 * No Stanley binary. review is a routing stub this spike (not implemented).
 */

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";

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
