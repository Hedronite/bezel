use crate::policy::{hook_decision, AUTO_ALLOW, CONCERN_PARK, WRITE_CODE_DENY_FLAGS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteVerdict {
    pub mapped: &'static str,
    pub gate: &'static str,
    pub reason: String,
    pub code_deny: bool,
    pub auto_allow: bool,
}

pub fn write_from_flags(flags: &[String], concern: Option<f64>) -> WriteVerdict {
    if let Some(flag) = WRITE_CODE_DENY_FLAGS.iter().find(|flag| flags.iter().any(|got| got == *flag))
    {
        return WriteVerdict {
            mapped: "stop",
            gate: "hold",
            reason: (*flag).to_string(),
            code_deny: true,
            auto_allow: AUTO_ALLOW,
        };
    }
    if concern_under_park(concern) {
        return WriteVerdict {
            mapped: "continue",
            gate: "auto",
            reason: "below_park".to_string(),
            code_deny: false,
            auto_allow: AUTO_ALLOW,
        };
    }
    let reason = if flags.is_empty() {
        "empty_findings_not_approval"
    } else {
        "writer_parent"
    };
    WriteVerdict {
        mapped: "writer",
        gate: "hold",
        reason: reason.to_string(),
        code_deny: false,
        auto_allow: AUTO_ALLOW,
    }
}

pub fn active_hook(verdict: &WriteVerdict) -> &'static str {
    let decision = hook_decision(verdict.mapped, verdict.gate);
    if decision == "allow" {
        "deny"
    } else {
        decision
    }
}

fn concern_under_park(concern: Option<f64>) -> bool {
    matches!(concern, Some(value) if value.is_finite() && value < CONCERN_PARK)
}

/// One permission input. Mode comes from `requested_mode` and `has_key`.
/// This surface does not read another mode.
#[derive(Clone, Copy)]
pub struct PermissionInput {
    pub class_id: &'static str,
    pub requested_mode: &'static str,
    pub has_key: bool,
    pub content_present: bool,
    pub label: Option<&'static str>,
    pub confidence: f64,
    pub allow_p: f64,
    pub deny_p: f64,
    pub ask_p: f64,
    pub path: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionVerdict {
    pub json: String,
    pub honor: bool,
    pub mode: &'static str,
    pub choice: &'static str,
    pub gate: &'static str,
    pub blocked: bool,
    pub hitl: bool,
    pub policy_id: &'static str,
    pub auto_allow: bool,
}

#[derive(Clone, Copy)]
pub struct PermissionParent {
    pub choice: &'static str,
    pub gate: &'static str,
    pub blocked: bool,
    pub exec: bool,
    pub hitl: Option<bool>,
    pub mode: &'static str,
}

struct Surface {
    id: &'static str,
    policy_id: &'static str,
    parent: &'static str,
    catalog: &'static str,
    new_catalog: bool,
}

/// `decidePermission` for the surface and mode step.
/// A class other than shell, write, or mcp is `None` (`null`).
/// Keyed write and keyed mcp routing stay on later steps.
pub fn decide_permission(input: &PermissionInput) -> Option<PermissionVerdict> {
    let surface = surface(input.class_id)?;
    let mode = effective_mode(input.requested_mode, input.has_key);
    let deny = write_deny_flag(surface.id, input.path);
    if !input.has_key {
        return Some(if let Some(flag) = deny {
            pack(
                surface,
                mode,
                Some("stop"),
                Some("deny"),
                input.label,
                flag,
                true,
                true,
            )
        } else {
            pack(surface, mode, None, None, input.label, "missing_key", false, true)
        });
    }
    if surface.id == "shell" {
        return Some(shell_from_label(surface, mode, input));
    }
    None
}

/// Shadow does not rewrite the parent. A continue does not clear a parent stop.
pub fn apply_permission_verdict(parent: PermissionParent, permission: &PermissionVerdict) -> String {
    let blocks = permission.honor
        && (permission.blocked || permission.choice == "stop" || permission.choice == "escalate");
    let choice = if blocks { permission.choice } else { parent.choice };
    let gate = if blocks { permission.gate } else { parent.gate };
    let blocked = if blocks { true } else { parent.blocked };
    let exec = if blocks { false } else { parent.exec };
    let hitl = if blocks { Some(permission.hitl) } else { parent.hitl };
    let hitl_json = match hitl {
        Some(value) => format!(",\"hitl\":{}", jbool(value)),
        None => String::new(),
    };
    format!(
        r#"{{"choice":"{choice}","gate":"{gate}","blocked":{blocked},"exec":{exec}{hitl_json},"mode":"{mode}","permission":{permission}}}"#,
        blocked = jbool(blocked),
        exec = jbool(exec),
        mode = parent.mode,
        permission = permission.json,
    )
}

fn effective_mode(requested: &str, has_key: bool) -> &'static str {
    if !has_key {
        return "shadow";
    }
    if requested.eq_ignore_ascii_case("active") {
        "active"
    } else {
        "shadow"
    }
}

fn surface(class_id: &str) -> Option<Surface> {
    match class_id {
        "shell" => Some(Surface {
            id: "shell",
            policy_id: crate::policy::LOOP_STOP_POLICY_ID,
            parent: "loop-stop",
            catalog: "permission",
            new_catalog: true,
        }),
        "write" => Some(Surface {
            id: "write",
            policy_id: crate::policy::CHECK_POLICY_ID,
            parent: "writer",
            catalog: "writer",
            new_catalog: false,
        }),
        "mcp" => Some(Surface {
            id: "mcp",
            policy_id: crate::policy::ROUTING_POLICY_ID,
            parent: "route-workflow",
            catalog: "route-workflow",
            new_catalog: false,
        }),
        _ => None,
    }
}

fn write_deny_flag(class_id: &str, path: &str) -> Option<&'static str> {
    if class_id == "write" && crate::check::file_kind(path) == "secret" {
        Some("secret_path")
    } else {
        None
    }
}

fn shell_from_label(surface: Surface, mode: &'static str, input: &PermissionInput) -> PermissionVerdict {
    if !input.content_present {
        return pack(
            surface,
            mode,
            Some("escalate"),
            Some("ask"),
            input.label,
            "no_content",
            false,
            false,
        );
    }
    let parent = match input.label {
        Some("allow") => "continue",
        Some("deny") => "stop",
        Some("ask") => "escalate",
        _ => {
            return uncertain(surface, mode, input.label, "cannot_tell");
        }
    };
    let decision = crate::loop_stop::apply_loop_stop_thresholds(
        parent,
        input.confidence,
        &[
            ("continue", input.allow_p),
            ("stop", input.deny_p),
            ("escalate", input.ask_p),
        ],
    );
    if matches!(decision.outcome, "continue" | "stop" | "escalate") {
        let label = match decision.outcome {
            "continue" => Some("allow"),
            "stop" => Some("deny"),
            _ => Some("ask"),
        };
        return pack(
            surface,
            mode,
            Some(decision.outcome),
            label,
            input.label,
            decision.reason,
            false,
            false,
        );
    }
    let reason = if decision.reason == "cannot_tell" {
        "cannot_tell"
    } else {
        "model_uncertain"
    };
    uncertain(surface, mode, input.label, reason)
}

fn uncertain(
    surface: Surface,
    mode: &'static str,
    model_label: Option<&'static str>,
    reason: &'static str,
) -> PermissionVerdict {
    let active = mode == "active";
    pack(
        surface,
        mode,
        if active { Some("escalate") } else { None },
        if active { Some("ask") } else { None },
        model_label,
        reason,
        false,
        false,
    )
}

fn pack(
    surface: Surface,
    mode: &'static str,
    mut mapped: Option<&'static str>,
    mut label: Option<&'static str>,
    model_label: Option<&'static str>,
    reason: &'static str,
    code_deny: bool,
    missing_key: bool,
) -> PermissionVerdict {
    let honor = mode == "active";
    let (choice, gate, blocked, exec, hitl) = if !honor {
        ("unclassified", "auto", false, true, false)
    } else if mapped == Some("continue") {
        ("continue", "auto", false, true, false)
    } else if mapped == Some("stop") {
        ("stop", "hold", true, false, false)
    } else {
        if mapped != Some("writer") {
            mapped = Some("escalate");
        }
        label = label.or(Some("ask"));
        ("escalate", "hold", true, false, true)
    };
    let json = format!(
        r#"{{"surface":"{id}","policyId":"{policy}","parent":"{parent}","catalog":"{catalog}","newCatalog":{new_catalog},"mode":"{mode}","honor":{honor},"shadow":{shadow},"blocked":{blocked},"exec":{exec},"hitl":{hitl},"choice":"{choice}","gate":"{gate}","label":{label},"modelLabel":{model_label},"mapped":{mapped},"reason":"{reason}","codeDeny":{code_deny},"skipped":false,"missingKey":{missing_key},"autoAllow":false}}"#,
        id = surface.id,
        policy = surface.policy_id,
        parent = surface.parent,
        catalog = surface.catalog,
        new_catalog = jbool(surface.new_catalog),
        honor = jbool(honor),
        shadow = jbool(!honor),
        blocked = jbool(blocked),
        exec = jbool(exec),
        hitl = jbool(hitl),
        label = json_opt(label),
        model_label = json_opt(model_label),
        mapped = json_opt(mapped),
        code_deny = jbool(code_deny),
        missing_key = jbool(missing_key),
    );
    PermissionVerdict {
        json,
        honor,
        mode,
        choice,
        gate,
        blocked,
        hitl,
        policy_id: surface.policy_id,
        auto_allow: false,
    }
}

fn json_opt(value: Option<&str>) -> String {
    match value {
        Some(text) => format!("\"{text}\""),
        None => "null".to_string(),
    }
}

fn jbool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
