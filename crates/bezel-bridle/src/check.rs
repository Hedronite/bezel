#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub path: String,
    pub header: String,
    pub text: String,
    pub deleted: bool,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    pub status: &'static str,
    pub approval: bool,
    pub empty_findings_are_not_approval: bool,
}

pub fn parse_unified_diff(text: &str) -> Vec<Hunk> {
    let mut hunks = Vec::new();
    let mut path = String::new();
    let mut buf: Vec<String> = Vec::new();
    let mut header = String::new();
    let mut gone = false;

    let flush = |hunks: &mut Vec<Hunk>,
                 path: &str,
                 buf: &mut Vec<String>,
                 header: &mut String,
                 gone: bool| {
        if header.is_empty() && buf.is_empty() {
            return;
        }
        let body = buf.join("\n");
        let gone_header = if gone { "+++ /dev/null" } else { "" };
        let gone_text = if gone { "deleted file mode" } else { "" };
        let header_out = [gone_header, header.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let text_out = [gone_text, gone_header, header.as_str(), body.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let added = buf
            .iter()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .cloned()
            .collect();
        let removed = buf
            .iter()
            .filter(|line| line.starts_with('-') && !line.starts_with("---"))
            .cloned()
            .collect();
        hunks.push(Hunk {
            path: if path.is_empty() {
                "unknown".to_string()
            } else {
                path.to_string()
            },
            header: header_out,
            text: text_out,
            deleted: gone,
            added,
            removed,
        });
        buf.clear();
        header.clear();
    };

    for line in text.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.starts_with("diff --git ") {
            flush(&mut hunks, &path, &mut buf, &mut header, gone);
            gone = false;
            path = git_b_path(line).unwrap_or(path);
            continue;
        }
        if line.starts_with("deleted file mode ") {
            gone = true;
            continue;
        }
        if let Some(plus) = line.strip_prefix("+++ ") {
            if plus == "/dev/null" {
                gone = true;
            } else if let Some(rest) = plus.strip_prefix("b/") {
                path = rest.to_string();
            }
            continue;
        }
        if line.starts_with("@@") {
            flush(&mut hunks, &path, &mut buf, &mut header, gone);
            header = line.to_string();
            continue;
        }
        if !header.is_empty() {
            buf.push(line.to_string());
        }
    }
    flush(&mut hunks, &path, &mut buf, &mut header, gone);
    hunks
}

fn git_b_path(line: &str) -> Option<String> {
    let rest = line.strip_prefix("diff --git a/")?;
    let idx = rest.find(" b/")?;
    Some(rest[idx + 3..].to_string())
}

pub fn file_kind(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name == ".env"
        || name.starts_with(".env.")
        || name == "id_rsa"
        || name == "id_ed25519"
        || name == "credentials"
        || name == "credentials.json"
        || name.ends_with(".pem")
        || name.ends_with(".p12")
        || name.ends_with(".key")
    {
        return "secret";
    }
    if is_test_path(path) {
        return "test";
    }
    "source"
}

pub fn is_test_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".test.js")
        || lower.ends_with(".test.jsx")
        || lower.ends_with(".test.ts")
        || lower.ends_with(".test.tsx")
        || lower.ends_with(".spec.js")
        || lower.ends_with(".spec.ts")
        || lower.ends_with("_test.go")
        || lower.contains("/test/")
        || lower.contains("/tests/")
        || lower.contains("/__tests__/")
}

fn is_assertion(line: &str) -> bool {
    let body = line.trim_start_matches(['+', '-']).trim();
    body.contains("expect(")
        || body.contains("assert(")
        || body.contains("assertEqual")
        || body.contains("XCTAssert")
}

pub fn write_flags(diff: &str, path: &str) -> Vec<String> {
    let mut flags = Vec::new();
    if file_kind(path) == "secret" {
        flags.push("secret_path".to_string());
    }
    for hunk in parse_unified_diff(diff) {
        for flag in deterministic_flags(&hunk) {
            if !flags.iter().any(|got| got == &flag) {
                flags.push(flag);
            }
        }
    }
    flags
}

pub fn deterministic_flags(hunk: &Hunk) -> Vec<String> {
    let mut flags = Vec::new();
    if file_kind(&hunk.path) == "secret" {
        flags.push("secret_path".to_string());
    }
    let skip_added = hunk
        .added
        .iter()
        .filter(|line| line.contains(".skip("))
        .count();
    let skip_removed = hunk
        .removed
        .iter()
        .filter(|line| line.contains(".skip("))
        .count();
    if skip_added > skip_removed {
        flags.push("skip_marker_added".to_string());
    }
    let assert_removed = hunk.removed.iter().filter(|line| is_assertion(line)).count();
    let assert_added = hunk.added.iter().filter(|line| is_assertion(line)).count();
    if is_test_path(&hunk.path) && assert_removed > assert_added {
        flags.push("assertions_removed".to_string());
    }
    let marked = hunk.deleted
        || hunk.header.contains("+++ /dev/null")
        || hunk.text.contains("deleted file mode");
    if is_test_path(&hunk.path) && marked {
        flags.push("test_file_deleted".to_string());
    }
    flags
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckFinding {
    pub flag: String,
    pub id: String,
    pub source: &'static str,
    pub severity: &'static str,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineCheck {
    pub status: &'static str,
    pub approval: bool,
    pub empty_findings_are_not_approval: bool,
    pub workflow: &'static str,
    pub policy: &'static str,
    pub mode: &'static str,
    pub missing_key: bool,
    pub findings: Vec<CheckFinding>,
    pub parked: Vec<String>,
    pub not_checked: Vec<String>,
    pub diff_present: bool,
    pub hunk_count: usize,
    pub judged: usize,
    pub intent: String,
}

/// Offline `--check`. No judge, no client. `approval` stays false.
fn warn_flag(flag: &str) -> bool {
    matches!(
        flag,
        "skip_marker_added" | "assertions_removed" | "test_file_deleted" | "secret_path"
    )
}

fn base_not_checked() -> Vec<String> {
    vec![
        "empty findings are not approval".to_string(),
        "tests were not executed".to_string(),
        "correctness of the change".to_string(),
    ]
}

pub fn run_offline_check(diff: &str) -> OfflineCheck {
    let hunks = parse_unified_diff(diff);
    let mut not_checked = base_not_checked();
    if diff.trim().is_empty() || hunks.is_empty() {
        not_checked.push("no git diff — check unavailable".to_string());
        return OfflineCheck {
            status: "no_diff",
            approval: false,
            empty_findings_are_not_approval: true,
            workflow: "check",
            policy: crate::policy::CHECK_POLICY_ID,
            mode: "shadow",
            missing_key: true,
            findings: Vec::new(),
            parked: Vec::new(),
            not_checked,
            diff_present: false,
            hunk_count: 0,
            judged: 0,
            intent: String::new(),
        };
    }
    let mut findings = Vec::new();
    for hunk in &hunks {
        let id = format!("h{}", hunk_index(&hunks, hunk));
        for flag in deterministic_flags(hunk) {
            let severity = if warn_flag(&flag) { "warn" } else { "info" };
            findings.push(CheckFinding {
                flag,
                id: id.clone(),
                source: "deterministic",
                severity,
                path: hunk.path.clone(),
            });
        }
        not_checked.push(format!(
            "{id} {}: TYPESAFE_API_KEY unset — unjudged (shadow continues)",
            hunk.path
        ));
    }
    let hunk_count = hunks.len();
    OfflineCheck {
        status: "incomplete",
        approval: false,
        empty_findings_are_not_approval: true,
        workflow: "check",
        policy: crate::policy::CHECK_POLICY_ID,
        mode: "shadow",
        missing_key: true,
        findings,
        parked: Vec::new(),
        not_checked,
        diff_present: true,
        hunk_count,
        judged: 0,
        intent: String::new(),
    }
}

fn hunk_index(hunks: &[Hunk], hunk: &Hunk) -> usize {
    hunks.iter().position(|row| row.path == hunk.path && row.header == hunk.header).unwrap_or(0) + 1
}

pub fn offline_check_json(report: &OfflineCheck) -> String {
    let findings = report
        .findings
        .iter()
        .map(|row| {
            format!(
                r#"{{"flag":"{}","id":"{}","source":"{}","severity":"{}","path":"{}"}}"#,
                row.flag, row.id, row.source, row.severity, row.path
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let not_checked = report
        .not_checked
        .iter()
        .map(|line| format!("\"{}\"", line.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"ok":true,"workflow":"{workflow}","policy":"{policy}","mode":"{mode}","missingKey":{missing},"approval":false,"emptyFindingsAreNotApproval":true,"status":"{status}","findings":[{findings}],"parked":[],"notChecked":[{not_checked}],"facts":{{"diffPresent":{present},"hunkCount":{hunks},"judged":{judged}}},"intent":"","catalog":{catalog}}}"#,
        workflow = report.workflow,
        policy = report.policy,
        mode = report.mode,
        missing = if report.missing_key { "true" } else { "false" },
        status = report.status,
        findings = findings,
        not_checked = not_checked,
        present = if report.diff_present { "true" } else { "false" },
        hunks = report.hunk_count,
        judged = report.judged,
        catalog = crate::catalog::tiny_catalog_json(),
    )
}

pub fn check_envelope(diff: &str) -> CheckReport {
    let hunks = parse_unified_diff(diff);
    if diff.trim().is_empty() || hunks.is_empty() {
        return CheckReport {
            status: "no_diff",
            approval: false,
            empty_findings_are_not_approval: true,
        };
    }
    CheckReport {
        status: "complete",
        approval: false,
        empty_findings_are_not_approval: true,
    }
}
