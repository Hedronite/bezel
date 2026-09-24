//! PreToolUse adapter. Stdout is one JSON line: `defer` or `deny`. Never `allow`.
//! No client is constructed here.

use crate::policy::{is_jev_bypass, pretool_hook_decision, pretool_stamp, GateVerdict, HookOut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modes {
    pub jev_mode: &'static str,
    pub bypass: bool,
    pub honors: bool,
}

pub fn modes_from_env(pairs: &[(&str, &str)]) -> Modes {
    let get = |key: &str| pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or("");
    let has_key = !get("TYPESAFE_API_KEY").is_empty();
    let active = |value: &str| has_key && value.eq_ignore_ascii_case("active");
    let jev_mode = if get("JEV_MODE").eq_ignore_ascii_case("active") {
        "active"
    } else {
        "shadow"
    };
    Modes {
        jev_mode,
        bypass: is_jev_bypass(get("JEV_BYPASS")),
        honors: jev_mode == "active" || active(get("JEV_PERMISSION_MODE")) || active(get("JEV_TYPED_CALL_MODE")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookEvent {
    pub tool_name: String,
    pub command: String,
}

pub fn parse_hook_event(text: &str) -> Result<HookEvent, &'static str> {
    let raw = text.trim();
    if raw.is_empty() {
        return Err("empty");
    }
    if !raw.starts_with('{') {
        return Err("malformed");
    }
    let tool_name = json_string(raw, "toolName")
        .or_else(|| json_string(raw, "tool_name"))
        .unwrap_or_default();
    let command = json_string(raw, "command").unwrap_or_default();
    Ok(HookEvent { tool_name, command })
}

fn json_string(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let idx = text.find(&needle)?;
    let rest = text[idx + needle.len()..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
            continue;
        }
        if ch == '"' {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

fn uncertain(modes: &Modes) -> HookOut {
    if modes.honors {
        HookOut {
            decision: "deny",
            reason: "jev uncertain".to_string(),
            exit_code: 2,
        }
    } else {
        HookOut {
            decision: "defer",
            reason: "shadow".to_string(),
            exit_code: 0,
        }
    }
}

/// Same map as the Node hook. A decision other than defer or deny becomes uncertain.
pub fn decide_hook(event: &HookEvent, modes: &Modes, verdict: Option<&GateVerdict>) -> HookOut {
    if modes.bypass {
        return HookOut {
            decision: "defer",
            reason: "bypass".to_string(),
            exit_code: 0,
        };
    }
    if event.tool_name.is_empty() {
        return uncertain(modes);
    }
    let stamp = pretool_stamp(&event.tool_name);
    if !stamp.as_ref().is_some_and(|row| row.matched) {
        return HookOut {
            decision: "defer",
            reason: "unmatched".to_string(),
            exit_code: 0,
        };
    }
    let Some(verdict) = verdict else {
        return uncertain(modes);
    };
    let mut verdict = verdict.clone();
    if verdict.policy_id.is_empty() {
        if let Some(row) = stamp.as_ref().and_then(|row| row.policy_id) {
            verdict.policy_id = row.to_string();
        }
    }
    let decision = pretool_hook_decision(&verdict);
    if decision.decision != "defer" && decision.decision != "deny" {
        return uncertain(modes);
    }
    decision
}

/// Same entry `bezel-bridle hook` uses. Stdin JSON in, one JSON line out, exit 2 on deny.
/// No API key uses the same missing-key router verdict as `jev-router`: active Bash
/// escalates on the loop-stop policy instead of an empty `jev uncertain`.
pub fn hook_command(stdin: &str, env: &[(&str, &str)]) -> (i32, String) {
    let modes = modes_from_env(env);
    let event = parse_hook_event(stdin).unwrap_or(HookEvent {
        tool_name: String::new(),
        command: String::new(),
    });
    let first = decide_hook(&event, &modes, None);
    let matched = !event.tool_name.is_empty()
        && pretool_stamp(&event.tool_name).as_ref().is_some_and(|row| row.matched);
    let has_key = env
        .iter()
        .any(|(key, value)| *key == "TYPESAFE_API_KEY" && !value.is_empty());
    // Malformed stdin, an empty tool, bypass, and an unmatched tool never see the router.
    if !matched || modes.bypass || first.reason == "bypass" || first.reason == "unmatched" {
        return (first.exit_code, hook_stdout(&first));
    }
    let verdict = if !has_key && modes.jev_mode == "active" {
        Some(missing_key_verdict())
    } else {
        None
    };
    let decision = annotate_missing_key(decide_hook(&event, &modes, verdict.as_ref()));
    (decision.exit_code, hook_stdout(&decision))
}

/// Hold suffix only on a router `jev choice=` line. Never rewrite `jev uncertain`.
fn annotate_missing_key(mut decision: HookOut) -> HookOut {
    if decision.reason.starts_with("jev choice=") && !decision.reason.contains("detail=") {
        decision.reason = format!(
            "{} detail=missing_key question=Is the judge available for this action?",
            decision.reason
        );
    }
    decision
}

fn missing_key_verdict() -> GateVerdict {
    GateVerdict {
        bypass: false,
        mode: "active".to_string(),
        choice: Some("escalate".to_string()),
        gate: Some("hold".to_string()),
        blocked: true,
        policy_id: String::new(),
        permission: None,
        calls: None,
    }
}

pub fn hook_on_text(text: &str, modes: &Modes, verdict: Option<&GateVerdict>) -> HookOut {
    let event = parse_hook_event(text).unwrap_or(HookEvent {
        tool_name: String::new(),
        command: String::new(),
    });
    decide_hook(&event, modes, verdict)
}

/// One JSON line. `decision` is `defer` or `deny`.
pub fn hook_stdout(decision: &HookOut) -> String {
    let decision_word = if decision.decision == "deny" { "deny" } else { "defer" };
    format!(
        "{{\"decision\":\"{decision_word}\",\"reason\":{}}}\n",
        json_escape(&decision.reason)
    )
}

fn json_escape(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}
