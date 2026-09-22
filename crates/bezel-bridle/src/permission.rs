use crate::{CONCERN_PARK, WRITE_CODE_DENY_FLAGS};

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
            auto_allow: false,
        };
    }
    if concern_under_park(concern) {
        return WriteVerdict {
            mapped: "continue",
            gate: "auto",
            reason: "below_park".to_string(),
            code_deny: false,
            auto_allow: false,
        };
    }
    WriteVerdict {
        mapped: "writer",
        gate: "hold",
        reason: "empty_findings_not_approval".to_string(),
        code_deny: false,
        auto_allow: false,
    }
}

fn concern_under_park(concern: Option<f64>) -> bool {
    matches!(concern, Some(value) if value.is_finite() && value < CONCERN_PARK)
}
