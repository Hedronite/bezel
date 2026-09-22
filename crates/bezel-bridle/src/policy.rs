//! Thresholds and the hook map. Jev classifies. Code authorizes.
//! No client is constructed here.

pub const CHECK_POLICY_ID: &str = "omapi-check-policy@1";
pub const LOOP_STOP_POLICY_ID: &str = "omapi-loop-stop-policy@1";
pub const ROUTING_POLICY_ID: &str = "omapi-route-workflow-policy@1";
pub const CONCERN_PARK: f64 = 0.4;
pub const MAX_HUNK_CHARS: usize = 4000;
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
