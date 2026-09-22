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
