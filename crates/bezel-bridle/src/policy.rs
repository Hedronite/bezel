//! Thresholds and the hook map. Jev classifies. Code authorizes.
//! No client is constructed here.

pub const CHECK_POLICY_ID: &str = "omapi-check-policy@1";
pub const LOOP_STOP_POLICY_ID: &str = "omapi-loop-stop-policy@1";
pub const ROUTING_POLICY_ID: &str = "omapi-route-workflow-policy@1";
pub const CONCERN_PARK: f64 = 0.4;
pub const CONCERN_FINDING: f64 = 0.7;
pub const KIND_MIN_CONFIDENCE: f64 = 0.6;
pub const KIND_MIN_PROBABILITY: f64 = 0.55;
pub const MAX_HUNK_CHARS: usize = 4000;
pub const MAX_DIGEST_CHARS: usize = 4000;
pub const PRETOOL_MATCHER: &str = "web_search|WebSearch|web_fetch|WebFetch|spawn_subagent|Task|Bash|run_terminal_command|run_terminal_cmd|Write|Edit|MultiEdit|search_replace|[A-Za-z0-9][A-Za-z0-9_.-]*__[A-Za-z0-9_.-]+";
pub const AUTO_ALLOW: bool = false;

pub const LOOP_STOP_MIN_CONFIDENCE: f64 = 0.6;
pub const LOOP_STOP_MIN_PROBABILITY: f64 = 0.55;
pub const LOOP_STOP_MIN_MARGIN: f64 = 0.15;

pub const WRITE_CODE_DENY_FLAGS: &[&str] = &[
    "secret_path",
    "skip_marker_added",
    "assertions_removed",
    "test_file_deleted",
];

/// Active continue defers. This function never returns `allow`.
pub fn hook_decision(mapped: &str, gate: &str) -> &'static str {
    if mapped == "continue" && gate == "auto" {
        "defer"
    } else {
        "deny"
    }
}

pub fn is_jev_bypass(value: &str) -> bool {
    value == "1" || value == "true"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub matched: bool,
    pub class_id: Option<&'static str>,
    pub policy_id: Option<&'static str>,
}

pub fn match_pretool_class(tool_name: &str) -> Option<(&'static str, &'static str)> {
    const NAMED: &[(&str, &str, &str)] = &[
        ("web_search", "web", LOOP_STOP_POLICY_ID),
        ("WebSearch", "web", LOOP_STOP_POLICY_ID),
        ("web_fetch", "web", LOOP_STOP_POLICY_ID),
        ("WebFetch", "web", LOOP_STOP_POLICY_ID),
        ("spawn_subagent", "subagent", LOOP_STOP_POLICY_ID),
        ("Task", "subagent", LOOP_STOP_POLICY_ID),
        ("Bash", "shell", LOOP_STOP_POLICY_ID),
        ("run_terminal_command", "shell", LOOP_STOP_POLICY_ID),
        ("run_terminal_cmd", "shell", LOOP_STOP_POLICY_ID),
        ("Write", "write", CHECK_POLICY_ID),
        ("Edit", "write", CHECK_POLICY_ID),
        ("MultiEdit", "write", CHECK_POLICY_ID),
        ("search_replace", "write", CHECK_POLICY_ID),
    ];
    if let Some((_, class_id, policy_id)) = NAMED.iter().find(|(name, _, _)| *name == tool_name) {
        return Some((*class_id, *policy_id));
    }
    if is_mcp_name(tool_name) {
        return Some(("mcp", ROUTING_POLICY_ID));
    }
    None
}

pub fn pretool_stamp(tool_name: &str) -> Option<Stamp> {
    if tool_name.is_empty() {
        return None;
    }
    match match_pretool_class(tool_name) {
        Some((class_id, policy_id)) => Some(Stamp {
            matched: true,
            class_id: Some(class_id),
            policy_id: Some(policy_id),
        }),
        None => Some(Stamp {
            matched: false,
            class_id: None,
            policy_id: None,
        }),
    }
}

fn is_mcp_name(name: &str) -> bool {
    let Some((left, right)) = name.split_once("__") else {
        return false;
    };
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let mut chars = left.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    let ok = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-';
    left.chars().all(ok) && right.chars().all(ok)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceVerdict {
    pub honor: bool,
    pub mode: String,
    pub choice: String,
    pub gate: String,
    pub blocked: bool,
    pub policy_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateVerdict {
    pub bypass: bool,
    pub mode: String,
    pub choice: Option<String>,
    pub gate: Option<String>,
    pub blocked: bool,
    pub policy_id: String,
    pub permission: Option<SurfaceVerdict>,
    pub calls: Option<SurfaceVerdict>,
}

impl GateVerdict {
    pub fn simple(mode: &str, choice: &str, gate: &str, blocked: bool) -> Self {
        Self {
            bypass: false,
            mode: mode.to_string(),
            choice: Some(choice.to_string()),
            gate: Some(gate.to_string()),
            blocked,
            policy_id: String::new(),
            permission: None,
            calls: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookOut {
    pub decision: &'static str,
    pub reason: String,
    pub exit_code: i32,
}

/// Shadow always execs. Active stop, escalate, and blocked do not.
pub fn should_exec_agent(mode: &str, choice: &str, gate: &str, blocked: bool) -> bool {
    if !mode.eq_ignore_ascii_case("active") {
        return true;
    }
    if choice == "stop" || choice == "escalate" || blocked {
        return false;
    }
    choice == "continue" || gate == "auto"
}

fn allows_exec(choice: &str, gate: &str, blocked: bool) -> bool {
    if choice == "stop" || choice == "escalate" || blocked {
        return false;
    }
    choice == "continue" || gate == "auto"
}

fn surface_honors(surface: &Option<SurfaceVerdict>) -> bool {
    matches!(surface, Some(row) if row.honor && row.mode.eq_ignore_ascii_case("active"))
}

/// Map a gate verdict onto one hook decision. Never `allow`.
pub fn pretool_hook_decision(verdict: &GateVerdict) -> HookOut {
    if verdict.bypass {
        return HookOut {
            decision: "defer",
            reason: "bypass".to_string(),
            exit_code: 0,
        };
    }
    let permission_honors = surface_honors(&verdict.permission);
    let calls_honors = surface_honors(&verdict.calls);
    let mode_active = verdict.mode.eq_ignore_ascii_case("active");
    if !mode_active && !permission_honors && !calls_honors {
        return HookOut {
            decision: "defer",
            reason: "shadow".to_string(),
            exit_code: 0,
        };
    }

    let mut choice = verdict.choice.clone().unwrap_or_else(|| "unclassified".to_string());
    let mut gate = verdict.gate.clone().unwrap_or_else(|| "hold".to_string());
    let loop_allows = allows_exec(&choice, &gate, verdict.blocked);
    let perm_blocks = permission_honors
        && verdict.permission.as_ref().is_some_and(|row| {
            row.blocked || row.choice == "stop" || row.choice == "escalate"
        });
    let calls_blocks = calls_honors
        && verdict.calls.as_ref().is_some_and(|row| row.blocked || row.choice == "stop" || row.choice == "escalate");

if perm_blocks || calls_blocks {
        let blocker = if perm_blocks {
            verdict.permission.as_ref()
        } else {
            verdict.calls.as_ref()
        };
        if let Some(row) = blocker {
            choice = if row.choice.is_empty() {
                "stop".to_string()
            } else {
                row.choice.clone()
            };
            gate = if row.gate.is_empty() {
                "hold".to_string()
            } else {
                row.gate.clone()
            };
        }
    } else if loop_allows && permission_honors {
        if let Some(row) = verdict.permission.as_ref() {
            if !row.choice.is_empty() {
                choice = row.choice.clone();
            }
            if !row.gate.is_empty() {
                gate = row.gate.clone();
            }
        }
    }
    let allows = !perm_blocks && !calls_blocks && loop_allows && allows_exec(&choice, &gate, false);

    if allows {
        return HookOut {
            decision: "defer",
            reason: "exec".to_string(),
            exit_code: 0,
        };
    }

    let policy_id = if !verdict.policy_id.is_empty() {
        verdict.policy_id.clone()
    } else if perm_blocks {
        verdict.permission.as_ref().map(|row| row.policy_id.clone()).unwrap_or_default()
    } else if calls_blocks {
        verdict.calls.as_ref().map(|row| row.policy_id.clone()).unwrap_or_default()
    } else {
        verdict.permission.as_ref().map(|row| row.policy_id.clone()).unwrap_or_default()
    };
    let policy = if policy_id.is_empty() {
        String::new()
    } else {
        format!(" policy={policy_id}")
    };
    HookOut {
        decision: "deny",
        reason: format!("jev choice={choice} gate={gate}{policy}"),
        exit_code: 2,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteDecision {
    pub outcome: String,
    pub reason: &'static str,
    pub selected: String,
    pub confidence: f64,
    pub selected_prob: f64,
    pub margin: f64,
}

pub fn apply_routing_thresholds(
    choice: &str,
    confidence: f64,
    probabilities: &[(&str, f64)],
    available: &[&str],
) -> RouteDecision {
    let selected = if choice.is_empty() { "cannot_tell" } else { choice };
    let selected_prob = probabilities
        .iter()
        .find(|(label, _)| *label == selected)
        .map(|(_, value)| *value)
        .unwrap_or(0.0);
    let runner_up = probabilities
        .iter()
        .filter(|(label, _)| *label != selected)
        .map(|(_, value)| *value)
        .fold(0.0_f64, f64::max);
    let margin = selected_prob - runner_up;
    let (outcome, reason) = if selected == "cannot_tell" {
        ("cannot_tell", "cannot_tell")
    } else if !available.iter().any(|name| *name == selected) {
        ("cannot_tell", "unavailable")
    } else if confidence < LOOP_STOP_MIN_CONFIDENCE
        || selected_prob < LOOP_STOP_MIN_PROBABILITY
        || margin < LOOP_STOP_MIN_MARGIN
    {
        ("cannot_tell", "model_uncertain")
    } else {
        (selected, "selected")
    };
    RouteDecision {
        outcome: outcome.to_string(),
        reason,
        selected: selected.to_string(),
        confidence,
        selected_prob,
        margin,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckBucket {
    pub bucket: &'static str,
    pub reason: &'static str,
    pub flag: Option<String>,
}

pub fn apply_check_thresholds(
    concern: f64,
    kind: &str,
    confidence: f64,
    probabilities: &[(&str, f64)],
) -> CheckBucket {
    let kind = if kind.is_empty() { "cannot_tell" } else { kind };
    let kind_prob = probabilities
        .iter()
        .find(|(label, _)| *label == kind)
        .map(|(_, value)| *value)
        .unwrap_or(0.0);
    if kind == "cannot_tell" || confidence < KIND_MIN_CONFIDENCE || kind_prob < KIND_MIN_PROBABILITY {
        return CheckBucket {
            bucket: "parked",
            reason: "model_uncertain",
            flag: None,
        };
    }
    if concern >= CONCERN_FINDING {
        let flag = if kind == "none" { "jev_concern" } else { kind };
        return CheckBucket {
            bucket: "finding",
            reason: "concern_above_threshold",
            flag: Some(flag.to_string()),
        };
    }
    if concern >= CONCERN_PARK {
        return CheckBucket {
            bucket: "parked",
            reason: "concern_in_band",
            flag: None,
        };
    }
    CheckBucket {
        bucket: "clear",
        reason: "below_park",
        flag: None,
    }
}

pub fn choice_family(class_id: &str) -> &'static [&'static str] {
    match class_id {
        "web" | "subagent" | "shell" => &["continue", "stop", "escalate"],
        "write" => &["none", "test_safety", "task_mismatch", "cannot_tell"],
        "mcp" => &["check", "review", "cannot_tell"],
        _ => &[],
    }
}
