//! Bridle core. The harness stays the harness. This crate decides.
//!
//! Pure core: `policy`, `check`, `permission`, `loop_stop`. No network. No client.

mod check;
mod harness;
mod loop_stop;
mod permission;
mod policy;

pub use check::{check_envelope, deterministic_flags, file_kind, parse_unified_diff, write_flags, CheckReport, Hunk};
pub use harness::{decide_hook, hook_on_text, hook_stdout, modes_from_env, parse_hook_event, HookEvent, Modes};
pub use loop_stop::{apply_loop_stop_thresholds, LoopStopDecision};
pub use permission::{active_hook, write_from_flags, WriteVerdict};
pub use policy::{
    hook_decision, match_pretool_class, pretool_hook_decision, pretool_stamp, GateVerdict, HookOut, AUTO_ALLOW,
    CHECK_POLICY_ID, CONCERN_PARK, LOOP_STOP_MIN_CONFIDENCE, LOOP_STOP_MIN_MARGIN, LOOP_STOP_MIN_PROBABILITY,
    LOOP_STOP_POLICY_ID, ROUTING_POLICY_ID, WRITE_CODE_DENY_FLAGS,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    fn testdata(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/jev-router/testdata")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
    }

    fn laws_sentence() -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills-stub/laws/LAWS.bend");
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
    }

    /// F4 is a git-produced deletion, not a hand-written diff.
    fn real_deletion_diff() -> String {
        let dir = std::env::temp_dir().join(format!("bezel-bridle-f4-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).expect("temp src");
        let git = |args: &[&str]| {
            let run = Command::new("git")
                .args(args)
                .current_dir(&dir)
                .env("HOME", &dir)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .env("GIT_AUTHOR_NAME", "jev")
                .env("GIT_AUTHOR_EMAIL", "jev@example.com")
                .env("GIT_COMMITTER_NAME", "jev")
                .env("GIT_COMMITTER_EMAIL", "jev@example.com")
                .output()
                .unwrap_or_else(|err| panic!("git {} failed to spawn: {err}", args.join(" ")));
            assert!(
                run.status.success(),
                "git {}\nstatus={:?}\nstderr={}\nstdout={}",
                args.join(" "),
                run.status.code(),
                String::from_utf8_lossy(&run.stderr),
                String::from_utf8_lossy(&run.stdout),
            );
            run
        };
        git(&["init"]);
        std::fs::write(dir.join("src/math.test.js"), "const value = 1;\n").expect("fixture file");
        git(&[
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=jev",
            "-c",
            "user.email=jev@example.com",
            "add",
            "src/math.test.js",
        ]);
        git(&[
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=jev",
            "-c",
            "user.email=jev@example.com",
            "commit",
            "-m",
            "init",
        ]);
        git(&["rm", "src/math.test.js"]);
        let diff = git(&["diff", "--cached", "--no-color", "--no-ext-diff"]);
        let text = String::from_utf8(diff.stdout).expect("diff utf8");
        let _ = std::fs::remove_dir_all(&dir);
        text
    }

    #[test]
    fn f1_empty_check_does_not_approve() {
        let diff = testdata("empty.diff");
        let report = check_envelope(&diff);
        assert_eq!(report.status, "no_diff");
        assert!(!report.approval);
        assert!(report.empty_findings_are_not_approval);
        let law = laws_sentence();
        assert!(law.contains("Empty findings are not approval."));
    }

    #[test]
    fn f4_real_deletion_stops_as_test_file_deleted() {
        let diff = real_deletion_diff();
        assert!(diff.contains("deleted file mode"), "{diff}");
        assert!(diff.contains("+++ /dev/null"), "{diff}");
        let hunks = parse_unified_diff(&diff);
        assert_eq!(hunks.len(), 1);
        assert!(hunks[0].header.contains("+++ /dev/null"));
        assert!(hunks[0].text.contains("deleted file mode"));
        assert!(hunks[0].deleted);
        let flags = deterministic_flags(&hunks[0]);
        assert_eq!(flags, vec!["test_file_deleted".to_string()]);
        let verdict = write_from_flags(&flags, Some(0.2));
        assert_eq!(verdict.mapped, "stop");
        assert_eq!(verdict.reason, "test_file_deleted");
        assert!(verdict.code_deny);
        assert!(!verdict.auto_allow);
        assert_ne!(verdict.mapped, "continue");
    }

    #[test]
    fn f6_concern_under_0_4_continues_absent_does_not() {
        let go = write_from_flags(&[], Some(0.2));
        assert_eq!(go.mapped, "continue");
        assert_eq!(go.gate, "auto");
        assert_eq!(go.reason, "below_park");
        assert!(!go.auto_allow);
        assert_eq!(active_hook(&go), "defer");
        assert_ne!(active_hook(&go), "allow");
        assert_ne!(hook_decision(go.mapped, go.gate), "allow");

        let zero = write_from_flags(&[], Some(0.0));
        assert_eq!(zero.mapped, "continue");

        let at_bar = write_from_flags(&[], Some(CONCERN_PARK));
        assert_ne!(at_bar.mapped, "continue");
        assert_eq!(at_bar.reason, "empty_findings_not_approval");

        let absent = write_from_flags(&[], None);
        assert_ne!(absent.mapped, "continue");
        assert_eq!(absent.mapped, "writer");
        assert_eq!(absent.reason, "empty_findings_not_approval");

        let nan = write_from_flags(&[], Some(f64::NAN));
        assert_ne!(nan.mapped, "continue");
    }

    #[test]
    fn deny_flag_secret_path_stops_by_name() {
        let flags = write_flags("", ".env");
        let verdict = write_from_flags(&flags, Some(0.2));
        assert_eq!(verdict.reason, "secret_path");
        assert_eq!(verdict.mapped, "stop");
        assert!(verdict.code_deny);
        assert!(!verdict.auto_allow);
    }

    #[test]
    fn deny_flag_skip_marker_added_stops_by_name() {
        let diff = testdata("skip-marker.diff");
        let flags = write_flags(&diff, "src/math.test.js");
        assert!(flags.contains(&"skip_marker_added".to_string()));
        let verdict = write_from_flags(&flags, Some(0.2));
        assert_eq!(verdict.reason, "skip_marker_added");
        assert_eq!(verdict.mapped, "stop");
        assert!(verdict.code_deny);
    }

    #[test]
    fn deny_flag_assertions_removed_stops_by_name() {
        let diff = testdata("assertions-removed.diff");
        let flags = write_flags(&diff, "src/math.test.js");
        assert!(flags.contains(&"assertions_removed".to_string()));
        let verdict = write_from_flags(&flags, Some(0.2));
        assert_eq!(verdict.reason, "assertions_removed");
        assert_eq!(verdict.mapped, "stop");
        assert!(verdict.code_deny);
    }

    #[test]
    fn deny_flag_test_file_deleted_stops_by_name() {
        let flags = vec!["test_file_deleted".to_string()];
        let verdict = write_from_flags(&flags, Some(0.2));
        assert_eq!(verdict.reason, "test_file_deleted");
        assert_eq!(verdict.mapped, "stop");
        assert!(verdict.code_deny);
        assert_ne!(verdict.mapped, "continue");
    }

    #[test]
    fn loop_stop_subthreshold_does_not_continue() {
        assert_eq!(LOOP_STOP_MIN_CONFIDENCE, 0.6);
        assert_eq!(LOOP_STOP_MIN_PROBABILITY, 0.55);
        assert_eq!(LOOP_STOP_MIN_MARGIN, 0.15);
        assert!(!AUTO_ALLOW);
        assert_eq!(CHECK_POLICY_ID, "omapi-check-policy@1");
        assert_eq!(LOOP_STOP_POLICY_ID, "omapi-loop-stop-policy@1");
        let shaky = apply_loop_stop_thresholds(
            "continue",
            0.4,
            &[("continue", 0.4), ("stop", 0.35), ("escalate", 0.25)],
        );
        assert_eq!(shaky.outcome, "cannot_tell");
        assert_eq!(shaky.reason, "model_uncertain");
        assert_ne!(shaky.outcome, "continue");
    }

    fn assert_hook(decision: &HookOut) {
        assert!(decision.decision == "defer" || decision.decision == "deny");
        assert_ne!(decision.decision, "allow");
        let line = hook_stdout(decision);
        assert!(line.ends_with('\n'));
        assert!(!line.contains("\"decision\":\"allow\""));
        assert!(line.contains("\"decision\":\"defer\"") || line.contains("\"decision\":\"deny\""));
    }

    #[test]
    fn g2_bash_fixture_matches_node_hook_decisions() {
        let raw = testdata("grok-pretool-bash.json");
        let event = parse_hook_event(&raw).expect("bash fixture");
        assert_eq!(event.tool_name, "Bash");
        let stamp = pretool_stamp(&event.tool_name).expect("stamp");
        assert!(stamp.matched);
        assert_eq!(stamp.class_id, Some("shell"));
        assert_eq!(stamp.policy_id, Some(LOOP_STOP_POLICY_ID));

        let shadow = modes_from_env(&[("JEV_MODE", "shadow")]);
        let deferred = decide_hook(
            &event,
            &shadow,
            Some(&GateVerdict::simple("shadow", "continue", "auto", false)),
        );
        assert_eq!(deferred.decision, "defer");
        assert_hook(&deferred);

        let active = modes_from_env(&[("JEV_MODE", "active")]);
        let denied = decide_hook(
            &event,
            &active,
            Some(&GateVerdict {
                policy_id: LOOP_STOP_POLICY_ID.to_string(),
                ..GateVerdict::simple("active", "stop", "hold", true)
            }),
        );
        assert_eq!(denied.decision, "deny");
        assert_hook(&denied);

        let uncertain = hook_on_text(&raw, &active, None);
        assert_eq!(uncertain.decision, "deny");
        assert_eq!(uncertain.reason, "jev uncertain");
        assert_hook(&uncertain);
        let shadow_empty = hook_on_text(&raw, &shadow, None);
        assert_eq!(shadow_empty.decision, "defer");
        assert_eq!(shadow_empty.reason, "shadow");
        assert_hook(&shadow_empty);
    }

    #[test]
    fn g2_pretool_table_defers_or_denies_and_never_allows() {
        let samples = [
            (true, "active", "stop", "hold", true, "", "defer", "bypass"),
            (false, "shadow", "stop", "hold", false, "", "defer", "shadow"),
            (false, "active", "continue", "auto", false, "", "defer", "exec"),
            (false, "active", "unclassified", "auto", false, "", "defer", "exec"),
            (
                false,
                "active",
                "stop",
                "hold",
                true,
                LOOP_STOP_POLICY_ID,
                "deny",
                "jev choice=stop gate=hold policy=omapi-loop-stop-policy@1",
            ),
            (false, "active", "escalate", "hold", true, "", "deny", "jev choice=escalate gate=hold"),
            (
                false,
                "active",
                "unclassified",
                "hold",
                false,
                "",
                "deny",
                "jev choice=unclassified gate=hold",
            ),
        ];
        for (bypass, mode, choice, gate, blocked, policy_id, decision, reason) in samples {
            let mut verdict = GateVerdict::simple(mode, choice, gate, blocked);
            verdict.bypass = bypass;
            verdict.policy_id = policy_id.to_string();
            let hook = pretool_hook_decision(&verdict);
            assert_eq!(hook.decision, decision);
            assert_eq!(hook.reason, reason);
            assert_ne!(hook.decision, "allow");
            assert_eq!(hook.exit_code, if decision == "deny" { 2 } else { 0 });
            assert_hook(&hook);
        }
    }

    #[test]
    fn g2_unmatched_bypass_and_malformed_stay_defer_or_deny() {
        let bash = HookEvent {
            tool_name: "Bash".to_string(),
        };
        let bypass = decide_hook(
            &bash,
            &modes_from_env(&[("JEV_BYPASS", "1")]),
            Some(&GateVerdict::simple("active", "stop", "hold", true)),
        );
        assert_eq!(bypass.decision, "defer");
        assert_eq!(bypass.reason, "bypass");
        assert_hook(&bypass);

        let not_bypass = decide_hook(
            &bash,
            &modes_from_env(&[("JEV_BYPASS", "yes"), ("JEV_MODE", "active")]),
            Some(&GateVerdict {
                policy_id: LOOP_STOP_POLICY_ID.to_string(),
                ..GateVerdict::simple("active", "stop", "hold", true)
            }),
        );
        assert_eq!(not_bypass.decision, "deny");
        assert_hook(&not_bypass);

        let unmapped = decide_hook(
            &HookEvent {
                tool_name: "read_file".to_string(),
            },
            &modes_from_env(&[("JEV_MODE", "active")]),
            Some(&GateVerdict::simple("active", "continue", "auto", false)),
        );
        assert_eq!(unmapped.decision, "defer");
        assert_eq!(unmapped.reason, "unmatched");
        assert_hook(&unmapped);

        let malformed = hook_on_text("not-json", &modes_from_env(&[("JEV_MODE", "active")]), None);
        assert_eq!(malformed.decision, "deny");
        assert_hook(&malformed);
        let empty = hook_on_text("", &modes_from_env(&[]), None);
        assert_eq!(empty.decision, "defer");
        assert_hook(&empty);

        let catalog = testdata("mcp-tools.json");
        for name in ["linear__list_issues", "linear__save_issue"] {
            assert!(catalog.contains(name));
            assert_eq!(match_pretool_class(name).map(|row| row.0), Some("mcp"));
            let event = HookEvent {
                tool_name: name.to_string(),
            };
            let continued = decide_hook(
                &event,
                &modes_from_env(&[("JEV_MODE", "active")]),
                Some(&GateVerdict::simple("active", "continue", "auto", false)),
            );
            assert_eq!(continued.decision, "defer");
            assert_hook(&continued);
        }
    }
}
