//! Bridle core. The harness stays the harness. This crate decides.
//!
//! First slice: code-deny flags, the 0.4 write concern bar, and an empty
//! check that never approves. No network. No client.

mod check;
mod permission;

pub use check::{check_envelope, deterministic_flags, parse_unified_diff, CheckReport, Hunk};
pub use permission::{write_from_flags, WriteVerdict};

pub const AUTO_ALLOW: bool = false;
pub const CONCERN_PARK: f64 = 0.4;
pub const LOOP_STOP_MIN_CONFIDENCE: f64 = 0.6;
pub const LOOP_STOP_MIN_PROBABILITY: f64 = 0.55;
pub const LOOP_STOP_MIN_MARGIN: f64 = 0.15;

pub const WRITE_CODE_DENY_FLAGS: &[&str] = &[
    "secret_path",
    "skip_marker_added",
    "assertions_removed",
    "test_file_deleted",
];

#[cfg(test)]
mod tests {
    use super::*;

    const DELETED_TEST: &str = "\
diff --git a/src/math.test.js b/src/math.test.js
deleted file mode 100644
index 1111111..0000000
--- a/src/math.test.js
+++ /dev/null
@@ -1 +0,0 @@
-const value = 1;
";

    const DELETED_ASSERTION: &str = "\
diff --git a/src/math.test.js b/src/math.test.js
deleted file mode 100644
--- a/src/math.test.js
+++ /dev/null
@@ -1 +0,0 @@
-test('adds', () => { expect(1).toBe(1); });
";

    #[test]
    fn real_deletion_stops_as_test_file_deleted() {
        let hunks = parse_unified_diff(DELETED_TEST);
        assert_eq!(hunks.len(), 1);
        assert!(hunks[0].header.contains("+++ /dev/null"));
        assert!(hunks[0].header.contains("@@ -1 +0,0 @@"));
        assert!(hunks[0].text.contains("deleted file mode"));
        assert!(hunks[0].deleted);
        let flags = deterministic_flags(&hunks[0]);
        assert_eq!(flags, vec!["test_file_deleted".to_string()]);
        let verdict = write_from_flags(&flags, Some(0.2));
        assert_eq!(verdict.mapped, "stop");
        assert_eq!(verdict.reason, "test_file_deleted");
        assert!(verdict.code_deny);
        assert!(!verdict.auto_allow);
    }

    #[test]
    fn deleted_assertion_names_assertions_removed_first() {
        let hunks = parse_unified_diff(DELETED_ASSERTION);
        let flags = deterministic_flags(&hunks[0]);
        assert!(flags.contains(&"assertions_removed".to_string()));
        assert!(flags.contains(&"test_file_deleted".to_string()));
        let verdict = write_from_flags(&flags, Some(0.2));
        assert_eq!(verdict.reason, "assertions_removed");
        assert_eq!(verdict.mapped, "stop");
    }

    #[test]
    fn concern_under_park_continues_and_absent_does_not() {
        let go = write_from_flags(&[], Some(0.2));
        assert_eq!(go.mapped, "continue");
        assert_eq!(go.gate, "auto");
        assert_eq!(go.reason, "below_park");
        assert!(!go.auto_allow);
        let zero = write_from_flags(&[], Some(0.0));
        assert_eq!(zero.mapped, "continue");
        let at_bar = write_from_flags(&[], Some(CONCERN_PARK));
        assert_ne!(at_bar.mapped, "continue");
        assert_eq!(at_bar.reason, "empty_findings_not_approval");
        let absent = write_from_flags(&[], None);
        assert_ne!(absent.mapped, "continue");
        assert_eq!(absent.reason, "empty_findings_not_approval");
    }

    #[test]
    fn four_deny_flags_stop_by_name() {
        for flag in WRITE_CODE_DENY_FLAGS {
            let verdict = write_from_flags(&[flag.to_string()], Some(0.2));
            assert_eq!(verdict.reason, *flag);
            assert_eq!(verdict.mapped, "stop");
            assert!(verdict.code_deny);
        }
    }

    #[test]
    fn empty_check_is_not_approval() {
        let report = check_envelope("");
        assert_eq!(report.status, "no_diff");
        assert!(!report.approval);
        assert!(report.empty_findings_are_not_approval);
    }

    #[test]
    fn loop_stop_mins_stay_put() {
        assert_eq!(LOOP_STOP_MIN_CONFIDENCE, 0.6);
        assert_eq!(LOOP_STOP_MIN_PROBABILITY, 0.55);
        assert_eq!(LOOP_STOP_MIN_MARGIN, 0.15);
        assert!(!AUTO_ALLOW);
    }
}
