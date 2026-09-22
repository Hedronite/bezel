//! Thresholds and the hook map. Jev classifies. Code authorizes.
//! No client is constructed here.

pub const CHECK_POLICY_ID: &str = "omapi-check-policy@1";
pub const LOOP_STOP_POLICY_ID: &str = "omapi-loop-stop-policy@1";
pub const CONCERN_PARK: f64 = 0.4;
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
