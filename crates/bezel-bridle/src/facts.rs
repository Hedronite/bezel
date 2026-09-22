//! Deterministic facts. Proposed edits are clipped. `--check` still reads git.
//! No client is constructed here.

use crate::policy::MAX_HUNK_CHARS;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFacts {
    pub text: String,
    pub present: bool,
    pub source: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub old_string: String,
    pub new_string: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteBody {
    pub path: String,
    pub file_path: Option<String>,
    pub old_string: Option<String>,
    pub new_string: Option<String>,
    pub contents: Option<String>,
    pub edits: Option<Vec<Edit>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gathered {
    pub proposed: String,
    pub diff: DiffFacts,
    pub available: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GatherOpts {
    pub repo: PathBuf,
    pub base: String,
    pub diff_file: Option<PathBuf>,
    pub diff_text: Option<String>,
    pub write: Option<WriteBody>,
}

impl Default for GatherOpts {
    fn default() -> Self {
        Self {
            repo: PathBuf::from("."),
            base: String::new(),
            diff_file: None,
            diff_text: None,
            write: None,
        }
    }
}

pub fn clip_to_max_hunk_chars(text: &str, max: Option<usize>) -> String {
    let cap = max.unwrap_or(MAX_HUNK_CHARS);
    text.chars().take(cap).collect()
}

pub fn proposed_write_diff(input: &WriteBody) -> (String, String) {
    let path = write_path(input);
    if let Some(edits) = &input.edits {
        if edits.is_empty() {
            return (path, String::new());
        }
        let mut parts = diff_header(&path, false);
        for edit in edits {
            parts.push("@@".to_string());
            parts.extend(prefixed_lines("-", &edit.old_string));
            parts.extend(prefixed_lines("+", &edit.new_string));
        }
        return (path, parts.join("\n"));
    }
    if input.old_string.is_some() || input.new_string.is_some() {
        let mut parts = diff_header(&path, false);
        parts.push("@@".to_string());
        parts.extend(prefixed_lines("-", input.old_string.as_deref().unwrap_or("")));
        parts.extend(prefixed_lines("+", input.new_string.as_deref().unwrap_or("")));
        return (path, parts.join("\n"));
    }
    if let Some(contents) = &input.contents {
        let mut parts = diff_header(&path, true);
        parts.push("@@".to_string());
        parts.extend(prefixed_lines("+", contents));
        return (path, parts.join("\n"));
    }
    (path, String::new())
}

pub fn write_tool_wire(input: &WriteBody) -> String {
    let (path, text) = proposed_write_diff(input);
    if text.is_empty() {
        return path;
    }
    let proposed = clip_to_max_hunk_chars(&text, None);
    format!(
        "{{\"path\":{},\"proposed\":{}}}",
        json_string(&path),
        json_string(&proposed)
    )
}

pub fn gather_diff(opts: &GatherOpts) -> DiffFacts {
    if let Some(text) = &opts.diff_text {
        return DiffFacts {
            present: diff_present(text),
            text: text.clone(),
            source: "injected".to_string(),
            error: None,
        };
    }
    if let Some(path) = &opts.diff_file {
        if !path.exists() {
            return DiffFacts {
                text: String::new(),
                present: false,
                source: "file".to_string(),
                error: Some(format!("diff file not found: {}", path.display())),
            };
        }
        let text = std::fs::read_to_string(path).unwrap_or_default();
        return DiffFacts {
            present: diff_present(&text),
            text,
            source: "file".to_string(),
            error: None,
        };
    }
    match git_worktree(&opts.repo, &opts.base) {
        Ok(text) => DiffFacts {
            present: diff_present(&text),
            source: if opts.base.is_empty() {
                "git-worktree".to_string()
            } else {
                format!("git-diff:{}", opts.base)
            },
            text,
            error: None,
        },
        Err(error) => DiffFacts {
            text: String::new(),
            present: false,
            source: "git".to_string(),
            error: Some(error),
        },
    }
}

/// Shipped gather: clipped proposed edit plus the git or file diff.
pub fn gather(opts: &GatherOpts) -> Gathered {
    let diff = gather_diff(opts);
    let proposed = opts.write.as_ref().map(write_tool_wire).unwrap_or_default();
    let available = if diff.present {
        vec!["check".to_string(), "review".to_string()]
    } else {
        Vec::new()
    };
    Gathered {
        proposed,
        diff,
        available,
    }
}

fn diff_present(text: &str) -> bool {
    !text.trim().is_empty()
}

fn write_path(input: &WriteBody) -> String {
    let raw = if !input.path.is_empty() {
        input.path.as_str()
    } else {
        input.file_path.as_deref().unwrap_or("")
    };
    raw.replace(['\r', '\n'], "")
}

fn diff_header(path: &str, created: bool) -> Vec<String> {
    let name = if path.is_empty() { "unknown" } else { path };
    vec![
        format!("diff --git a/{name} b/{name}"),
        if created {
            "--- /dev/null".to_string()
        } else {
            format!("--- a/{name}")
        },
        format!("+++ b/{name}"),
    ]
}

fn prefixed_lines(prefix: &str, value: &str) -> Vec<String> {
    value.split('\n').map(|line| {
        let line = line.strip_suffix('\r').unwrap_or(line);
        format!("{prefix}{line}")
    }).collect()
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn git_worktree(repo: &Path, base: &str) -> Result<String, String> {
    let mut args = vec!["diff", "--no-color", "--no-ext-diff"];
    if !base.is_empty() {
        args.push(base);
    }
    let work = run_git(repo, &args)?;
    let staged = if base.is_empty() {
        run_git(repo, &["diff", "--cached", "--no-color", "--no-ext-diff"])?
    } else {
        String::new()
    };
    Ok([work, staged]
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n"))
}

fn run_git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let run = Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .map_err(|err| format!("git {} failed to spawn: {err}", args.join(" ")))?;
    if !run.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&run.stderr).trim()
        ));
    }
    String::from_utf8(run.stdout).map_err(|err| err.to_string())
}
