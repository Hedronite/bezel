//! Bridle core. The harness stays the harness. This crate decides.
//!
//! Only `cli` may construct a client. This gate replays recorded `systemOne` bytes.

mod calls;
mod catalog;
mod check;
mod cli;
mod facts;
mod harness;
mod judge;
mod loop_stop;
mod permission;
mod policy;
mod router;
mod transport;

#[cfg(test)]
mod oracle;

pub use calls::{
    cannot_tell_json, envelope_matches, guard_effect, hard_stop_json, missing_key_envelope, model_uncertain_json,
    obvious_effect_json, rank_names, stated_keeps_optional_arg, tool_call, EffectGuard, HardStopTool,
};
pub use catalog::{schema_dump_json, tiny_catalog_json};
pub use check::{
    check_envelope, deterministic_flags, file_kind, offline_check_json, parse_unified_diff, run_offline_check,
    write_flags, CheckReport, Hunk, OfflineCheck,
};
pub use cli::{fixture_client, live_transport_argv, run as cli_run, LIVE_COMMAND, LIVE_ON};
pub use facts::{
    available_capabilities, clip_to_max_hunk_chars, diff_present, gather, gather_diff, parse_write_tool_wire,
    scrub_secret_text, secret_object_key, write_tool_wire, Capabilities, Edit, GatherOpts, WriteBody,
};
pub use judge::{matches_recorded, recorded_answer, CHECK_JUDGE, MAIN_GATE, SHADOW_WORKFLOW};
pub use harness::{
    decide_hook, hook_command, hook_on_text, hook_stdout, modes_from_env, parse_hook_event, HookEvent, Modes,
};
pub use loop_stop::{apply_loop_stop_thresholds, decide_loop_stop, missing_key_loop, LoopEnvelope, LoopStopDecision};
pub use permission::{
    active_hook, apply_permission_verdict, decide_permission, map_permission_label, permission_surface,
    resolve_permission_mode, write_from_flags, PermissionInput, PermissionParent, PermissionVerdict,
    TypedPermission, WriteVerdict,
};
pub use router::bash_no_key;
pub use policy::{
    apply_check_thresholds, apply_routing_thresholds, choice_family, hook_decision, is_jev_bypass,
    match_pretool_class, pretool_hook_decision, pretool_stamp, should_exec_agent, CheckBucket, GateVerdict,
    HookOut, RouteDecision, SurfaceVerdict, AUTO_ALLOW, CHECK_POLICY_ID, CONCERN_FINDING, CONCERN_PARK,
    KIND_MIN_CONFIDENCE, KIND_MIN_PROBABILITY, MAX_DIGEST_CHARS, MAX_HUNK_CHARS, LOOP_STOP_MIN_CONFIDENCE,
    LOOP_STOP_MIN_MARGIN, LOOP_STOP_MIN_PROBABILITY, LOOP_STOP_POLICY_ID, PRETOOL_MATCHER, ROUTING_POLICY_ID,
    WRITE_CODE_DENY_FLAGS,
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
        assert!(law.contains("law empty_findings_are_not_approval:"));
        assert!(law.contains("{approval(Empty{}) == No{} : Approval}"));
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

    fn permission_golden(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/jev-router/testdata/permission")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
    }

    fn permission_input(
        class_id: &'static str,
        requested_mode: &'static str,
        has_key: bool,
        content_present: bool,
        label: Option<&'static str>,
        confidence: f64,
        allow_p: f64,
        deny_p: f64,
        ask_p: f64,
        path: &'static str,
    ) -> PermissionInput<'static> {
        PermissionInput {
            class_id,
            requested_mode,
            has_key,
            content_present,
            label,
            confidence,
            allow_p,
            deny_p,
            ask_p,
            path,
            diff: "",
            concern: None,
            flags: None,
            tool_name: "",
            effect: "",
            routing_outcome: None,
            typed: None,
        }
    }

    fn assert_permission(name: &str, verdict: &PermissionVerdict) {
        assert_eq!(verdict.json, permission_golden(name), "{name}");
        assert!(!verdict.auto_allow, "{name}");
        assert!(!verdict.json.contains("F454"), "{name}");
        assert!(!verdict.json.contains("\"decision\":\"allow\""), "{name}");
    }

    #[test]
    fn g1_permission_surface_matches_permission_mjs() {
        let src = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/permission.rs"))
            .unwrap();
        assert!(!src.contains("JEV_MODE"));
        assert!(!src.contains("F454"));

        assert!(decide_permission(&permission_input(
            "web", "active", true, false, Some("allow"), 0.0, 0.0, 0.0, 0.0, ""
        ))
        .is_none());
        assert_eq!(permission_golden("unknown-web.json"), "null");
        assert!(decide_permission(&permission_input(
            "read_file", "active", true, false, None, 0.0, 0.0, 0.0, 0.0, ""
        ))
        .is_none());
        assert_eq!(permission_golden("unknown-read-file.json"), "null");

        let missing = decide_permission(&permission_input(
            "shell",
            "active",
            false,
            true,
            Some("deny"),
            0.99,
            0.01,
            0.98,
            0.01,
            "",
        ))
        .unwrap();
        assert_permission("shell-missing-key.json", &missing);
        assert_eq!(missing.mode, "shadow");
        assert!(!missing.honor);
        assert_eq!(missing.choice, "unclassified");

        let denied = decide_permission(&permission_input(
            "write",
            "active",
            false,
            false,
            Some("allow"),
            0.0,
            0.0,
            0.0,
            0.0,
            ".env",
        ))
        .unwrap();
        assert_permission("write-missing-key-deny.json", &denied);
        assert_eq!(denied.mode, "shadow");
        assert!(denied.json.contains("\"codeDeny\":true"));
        assert!(denied.json.contains("\"mapped\":\"stop\""));
        assert!(denied.json.contains("\"reason\":\"secret_path\""));
        assert_eq!(denied.choice, "unclassified");
        assert!(!denied.blocked);

        let mcp = decide_permission(&permission_input(
            "mcp", "active", false, false, None, 0.0, 0.0, 0.0, 0.0, ""
        ))
        .unwrap();
        assert_permission("mcp-missing-key.json", &mcp);
        assert_eq!(mcp.mode, "shadow");
        assert_eq!(mcp.policy_id, ROUTING_POLICY_ID);

        let shadow = decide_permission(&permission_input(
            "shell", "yes", true, true, Some("allow"), 0.9, 0.8, 0.1, 0.1, ""
        ))
        .unwrap();
        assert_permission("shell-shadow-default.json", &shadow);
        assert_eq!(shadow.mode, "shadow");
        assert!(!shadow.honor);
        assert!(shadow.json.contains("\"mapped\":\"continue\""));

        let continued = decide_permission(&permission_input(
            "shell", "active", true, true, Some("allow"), 0.9, 0.8, 0.1, 0.1, ""
        ))
        .unwrap();
        assert_permission("shell-active-continue.json", &continued);
        let kept = apply_permission_verdict(
            PermissionParent {
                choice: "stop",
                gate: "hold",
                blocked: true,
                exec: false,
                hitl: Some(false),
                mode: "active",
            },
            &continued,
        );
        assert_eq!(kept, permission_golden("continue-keeps-parent-stop.json"));
        assert!(kept.contains("\"choice\":\"stop\""));
        assert!(kept.contains("\"autoAllow\":false"));
        let kept_hook = pretool_hook_decision(&GateVerdict {
            bypass: false,
            mode: "active".to_string(),
            choice: Some("stop".to_string()),
            gate: Some("hold".to_string()),
            blocked: true,
            policy_id: String::new(),
            permission: Some(SurfaceVerdict {
                honor: continued.honor,
                mode: continued.mode.to_string(),
                choice: continued.choice.to_string(),
                gate: continued.gate.to_string(),
                blocked: continued.blocked,
                policy_id: continued.policy_id.to_string(),
            }),
            calls: None,
        });
        assert_eq!(kept_hook.decision, "deny");
        assert_ne!(kept_hook.decision, "allow");
        assert_eq!(
            format!(
                r#"{{"action":"deny","exitCode":2,"decision":"deny","reason":"{}"}}"#,
                kept_hook.reason
            ),
            permission_golden("continue-keeps-parent-stop-hook.json")
        );

        let shadow_deny = decide_permission(&permission_input(
            "shell", "shadow", true, true, Some("deny"), 0.9, 0.05, 0.9, 0.05, ""
        ))
        .unwrap();
        assert_permission("shell-shadow-deny.json", &shadow_deny);
        let logged = apply_permission_verdict(
            PermissionParent {
                choice: "continue",
                gate: "auto",
                blocked: false,
                exec: true,
                hitl: None,
                mode: "shadow",
            },
            &shadow_deny,
        );
        assert_eq!(logged, permission_golden("shadow-keeps-parent.json"));
        assert!(logged.contains("\"choice\":\"continue\""));
        assert!(!logged.contains("\"choice\":\"stop\""));
        let logged_hook = pretool_hook_decision(&GateVerdict {
            bypass: false,
            mode: "shadow".to_string(),
            choice: Some("continue".to_string()),
            gate: Some("auto".to_string()),
            blocked: false,
            policy_id: String::new(),
            permission: Some(SurfaceVerdict {
                honor: shadow_deny.honor,
                mode: shadow_deny.mode.to_string(),
                choice: shadow_deny.choice.to_string(),
                gate: shadow_deny.gate.to_string(),
                blocked: shadow_deny.blocked,
                policy_id: shadow_deny.policy_id.to_string(),
            }),
            calls: None,
        });
        assert_eq!(logged_hook.decision, "defer");
        assert_ne!(logged_hook.decision, "allow");
        assert_eq!(logged_hook.reason, "shadow");

        let honoring = pretool_hook_decision(&GateVerdict {
            bypass: false,
            mode: "shadow".to_string(),
            choice: Some("continue".to_string()),
            gate: Some("auto".to_string()),
            blocked: false,
            policy_id: String::new(),
            permission: Some(SurfaceVerdict {
                honor: continued.honor,
                mode: continued.mode.to_string(),
                choice: continued.choice.to_string(),
                gate: continued.gate.to_string(),
                blocked: continued.blocked,
                policy_id: continued.policy_id.to_string(),
            }),
            calls: None,
        });
        assert_eq!(honoring.decision, "defer");
        assert_eq!(honoring.reason, "exec");
        assert_ne!(honoring.decision, "allow");
    }

    fn write_input<'a>(
        requested_mode: &'static str,
        label: Option<&'static str>,
        path: &'a str,
        diff: &'a str,
        concern: Option<f64>,
    ) -> PermissionInput<'a> {
        PermissionInput {
            class_id: "write",
            requested_mode,
            has_key: true,
            content_present: false,
            label,
            confidence: 0.0,
            allow_p: 0.0,
            deny_p: 0.0,
            ask_p: 0.0,
            path,
            diff,
            concern,
            flags: None,
            tool_name: "",
            effect: "",
            routing_outcome: None,
            typed: None,
        }
    }

    fn hook_of(mode: &str, choice: &str, gate: &str, blocked: bool, verdict: &PermissionVerdict) -> crate::policy::HookOut {
        pretool_hook_decision(&GateVerdict {
            bypass: false,
            mode: mode.to_string(),
            choice: Some(choice.to_string()),
            gate: Some(gate.to_string()),
            blocked,
            policy_id: String::new(),
            permission: Some(SurfaceVerdict {
                honor: verdict.honor,
                mode: verdict.mode.to_string(),
                choice: verdict.choice.to_string(),
                gate: verdict.gate.to_string(),
                blocked: verdict.blocked,
                policy_id: verdict.policy_id.to_string(),
            }),
            calls: None,
        })
    }

    #[test]
    fn g2_write_flags_beat_allow_and_empty_findings_are_not_approval() {
        let docs = "diff --git a/notes/a.md b/notes/a.md\n--- a/notes/a.md\n+++ b/notes/a.md\n@@\n-old\n+new paragraph";
        let deleted = "diff --git a/src/math.test.js b/src/math.test.js\ndeleted file mode 100644\nindex 1111111..0000000\n--- a/src/math.test.js\n+++ /dev/null\n@@ -1 +0,0 @@\n-const value = 1;";
        let skip = testdata("skip-marker.diff");
        let assertions = testdata("assertions-removed.diff");

        let secret = write_input("active", Some("allow"), ".env", docs, Some(0.2));
        let stopped = decide_permission(&secret).unwrap();
        assert_permission("write-secret-path.json", &stopped);
        assert_eq!(stopped.choice, "stop");
        assert!(stopped.json.contains("\"modelLabel\":\"allow\""));
        assert!(stopped.json.contains("\"reason\":\"secret_path\""));
        assert_ne!(hook_of("active", "continue", "auto", false, &stopped).decision, "allow");

        let skip_in = write_input("active", Some("allow"), "", &skip, Some(0.2));
        assert_permission("write-skip-marker.json", &decide_permission(&skip_in).unwrap());
        let assert_in = write_input("active", Some("allow"), "", &assertions, Some(0.2));
        assert_permission("write-assertions-removed.json", &decide_permission(&assert_in).unwrap());
        let deleted_in = write_input("active", Some("allow"), "", deleted, Some(0.2));
        let deleted_verdict = decide_permission(&deleted_in).unwrap();
        assert_permission("write-test-file-deleted.json", &deleted_verdict);
        assert!(deleted_verdict.json.contains("\"codeDeny\":true"));
        assert!(!deleted_verdict.json.contains("F454"));

        let below = decide_permission(&write_input("active", Some("allow"), "notes/a.md", docs, Some(0.2))).unwrap();
        assert_permission("write-below-park.json", &below);
        assert_eq!(below.choice, "continue");
        assert!(below.json.contains("\"reason\":\"below_park\""));
        let zero = decide_permission(&write_input("active", Some("allow"), "notes/a.md", docs, Some(0.0))).unwrap();
        assert_permission("write-below-park-zero.json", &zero);
        let below_plain = decide_permission(&write_input("active", None, "notes/a.md", docs, Some(0.2))).unwrap();
        assert_eq!(below_plain.choice, "continue");
        assert!(below_plain.json.contains("\"modelLabel\":null"));
        let kept = apply_permission_verdict(
            PermissionParent {
                choice: "stop",
                gate: "hold",
                blocked: true,
                exec: false,
                hitl: Some(false),
                mode: "active",
            },
            &below_plain,
        );
        assert_eq!(kept, permission_golden("write-below-park-keeps-stop.json"));
        assert!(kept.contains("\"choice\":\"stop\""));
        let kept_hook = hook_of("active", "stop", "hold", true, &below_plain);
        assert_eq!(kept_hook.decision, "deny");
        assert_ne!(kept_hook.decision, "allow");
        assert_eq!(
            format!(
                r#"{{"action":"deny","exitCode":2,"decision":"deny","reason":"{}"}}"#,
                kept_hook.reason
            ),
            permission_golden("write-below-park-keeps-stop-hook.json")
        );

        let at_bar = decide_permission(&write_input("active", Some("allow"), "notes/a.md", docs, Some(0.4))).unwrap();
        assert_permission("write-at-park.json", &at_bar);
        assert_ne!(at_bar.choice, "continue");
        let absent = decide_permission(&write_input("active", Some("allow"), "notes/a.md", docs, None)).unwrap();
        assert_ne!(absent.choice, "continue");
        assert!(absent.json.contains("\"reason\":\"empty_findings_not_approval\""));
        let nan = decide_permission(&write_input("active", Some("allow"), "src/app.js", "", Some(f64::NAN))).unwrap();
        assert_ne!(nan.choice, "continue");

        let empty = decide_permission(&write_input("active", Some("allow"), "src/app.js", "", None)).unwrap();
        assert_permission("write-empty.json", &empty);
        assert!(empty.json.contains("\"reason\":\"empty_findings_not_approval\""));
        assert!(empty.json.contains("\"mapped\":\"writer\""));
        assert_ne!(empty.choice, "continue");
        let empty_hook = hook_of("active", "continue", "auto", false, &empty);
        assert_eq!(empty_hook.decision, "deny");
        assert_ne!(empty_hook.decision, "allow");
        assert_eq!(
            format!(
                r#"{{"action":"deny","exitCode":2,"decision":"deny","reason":"{}"}}"#,
                empty_hook.reason
            ),
            permission_golden("write-empty-hook.json")
        );
        assert!(!empty.json.contains("\"approval\":true"));

        let shadow_empty = decide_permission(&write_input("shadow", None, "src/app.js", "", None)).unwrap();
        assert_permission("write-shadow-empty.json", &shadow_empty);
        assert!(!shadow_empty.honor);
        assert!(!shadow_empty.blocked);
        assert_eq!(shadow_empty.choice, "unclassified");
    }

    fn shell_hook(mode: &str, verdict: &PermissionVerdict) -> String {
        let hook = hook_of(mode, "continue", "auto", false, verdict);
        assert_ne!(hook.decision, "allow");
        format!(
            r#"{{"action":"{}","exitCode":{},"decision":"{}","reason":"{}"}}"#,
            if hook.decision == "deny" { "deny" } else { "defer" },
            hook.exit_code,
            hook.decision,
            hook.reason,
        )
    }

    #[test]
    fn g3_shell_asks_without_content_and_under_the_loop_stop_bars() {
        assert_eq!(LOOP_STOP_MIN_CONFIDENCE, 0.6);
        assert_eq!(LOOP_STOP_MIN_PROBABILITY, 0.55);
        assert_eq!(LOOP_STOP_MIN_MARGIN, 0.15);

        let no_content = decide_permission(&permission_input(
            "shell", "active", true, false, Some("allow"), 0.9, 0.8, 0.1, 0.1, "",
        ))
        .unwrap();
        assert_permission("shell-no-content.json", &no_content);
        assert_eq!(no_content.choice, "escalate");
        assert_ne!(no_content.choice, "continue");
        assert!(no_content.json.contains("\"reason\":\"no_content\""));
        assert_eq!(shell_hook("active", &no_content), permission_golden("shell-no-content-hook.json"));

        let at_bars = decide_permission(&permission_input(
            "shell", "active", true, true, Some("allow"), 0.6, 0.55, 0.4, 0.05, "",
        ))
        .unwrap();
        assert_permission("shell-at-bars.json", &at_bars);
        assert_eq!(at_bars.choice, "continue");
        assert_eq!(at_bars.json.contains("\"reason\":\"selected\""), true);
        assert_eq!(shell_hook("shadow", &at_bars), permission_golden("shell-at-bars-hook.json"));

        let under = decide_permission(&permission_input(
            "shell", "active", true, true, Some("allow"), 0.4, 0.4, 0.35, 0.25, "",
        ))
        .unwrap();
        assert_permission("shell-under-bars.json", &under);
        assert_eq!(under.choice, "escalate");
        assert_ne!(under.choice, "continue");
        assert!(under.json.contains("\"reason\":\"model_uncertain\""));
        assert_eq!(shell_hook("active", &under), permission_golden("shell-under-bars-hook.json"));

        let abstain = decide_permission(&permission_input(
            "shell", "active", true, true, Some("cannot_tell"), 0.9, 0.1, 0.0, 0.0, "",
        ))
        .unwrap();
        assert_permission("shell-cannot-tell.json", &abstain);
        assert_eq!(abstain.choice, "escalate");
        assert_ne!(abstain.choice, "continue");
        assert!(abstain.json.contains("\"reason\":\"cannot_tell\""));
        assert_eq!(shell_hook("active", &abstain), permission_golden("shell-cannot-tell-hook.json"));

        let ask = decide_permission(&permission_input(
            "shell", "active", true, true, Some("ask"), 0.2, 0.2, 0.2, 0.6, "",
        ))
        .unwrap();
        assert_permission("shell-ask-under-bars.json", &ask);
        assert_eq!(ask.choice, "escalate");
        assert!(ask.hitl);
        assert_ne!(ask.choice, "continue");
        assert!(!ask.json.contains("\"decision\":\"allow\""));
    }

    fn mcp_input<'a>(
        label: Option<&'static str>,
        confidence: f64,
        allow_p: f64,
        deny_p: f64,
        ask_p: f64,
        tool_name: &'a str,
        effect: &'a str,
        routing_outcome: Option<&'a str>,
        typed: Option<TypedPermission<'a>>,
    ) -> PermissionInput<'a> {
        PermissionInput {
            class_id: "mcp",
            requested_mode: "active",
            has_key: true,
            content_present: false,
            label,
            confidence,
            allow_p,
            deny_p,
            ask_p,
            path: "",
            diff: "",
            concern: None,
            flags: None,
            tool_name,
            effect,
            routing_outcome,
            typed,
        }
    }

    fn facet_typed(honor: bool) -> TypedPermission<'static> {
        TypedPermission {
            honor,
            policy_id: ROUTING_POLICY_ID,
            transport: "facet",
            mapped: "continue",
            reason: "selected",
            code_deny: false,
            name: "",
            effect: "",
        }
    }

    #[test]
    fn g4_mcp_read_follows_loop_stop_and_workflow_does_not_continue() {
        let mutate = decide_permission(&mcp_input(
            Some("allow"),
            0.9,
            0.8,
            0.1,
            0.1,
            "linear__save_issue",
            "write",
            None,
            None,
        ))
        .unwrap();
        assert_permission("mcp-mutate-write.json", &mutate);
        assert_eq!(mutate.choice, "escalate");
        assert!(mutate.json.contains("\"reason\":\"mutating_mcp\""));
        assert_ne!(mutate.choice, "continue");
        assert_eq!(shell_hook("active", &mutate), permission_golden("mcp-mutate-write-hook.json"));

        let named = decide_permission(&mcp_input(
            Some("allow"),
            0.9,
            0.8,
            0.1,
            0.1,
            "notes__add",
            "",
            None,
            None,
        ))
        .unwrap();
        assert_permission("mcp-mutate-name.json", &named);
        assert!(named.json.contains("\"reason\":\"mutating_mcp\""));
        let effect_wins = decide_permission(&mcp_input(
            Some("allow"),
            0.9,
            0.8,
            0.1,
            0.1,
            "lapis__search",
            "write",
            None,
            None,
        ))
        .unwrap();
        assert_permission("mcp-effect-beats-name.json", &effect_wins);
        assert_eq!(effect_wins.choice, "escalate");

        let read = decide_permission(&mcp_input(
            Some("allow"),
            0.9,
            0.8,
            0.1,
            0.1,
            "linear__list_issues",
            "read",
            None,
            None,
        ))
        .unwrap();
        assert_permission("mcp-read-list.json", &read);
        assert_eq!(read.choice, "continue");
        assert_eq!(read.policy_id, ROUTING_POLICY_ID);
        assert!(read.json.contains("\"reason\":\"selected\""));
        assert!(!read.json.contains("workflow_is_not_permission"));
        assert_eq!(shell_hook("shadow", &read), permission_golden("mcp-read-list-hook.json"));
        assert_ne!(shell_hook("shadow", &read).contains("\"decision\":\"allow\""), true);

        let search = decide_permission(&mcp_input(
            Some("allow"),
            0.9,
            0.8,
            0.1,
            0.1,
            "lapis__search",
            "",
            None,
            Some(TypedPermission {
                honor: false,
                policy_id: "",
                transport: "",
                mapped: "escalate",
                reason: "no_catalog",
                code_deny: false,
                name: "",
                effect: "",
            }),
        ))
        .unwrap();
        assert_permission("mcp-lapis-search.json", &search);
        assert_eq!(search.choice, "continue");
        assert!(!search.json.contains("no_catalog"));
        assert!(!search.json.contains("workflow_is_not_permission"));

        let bare = decide_permission(&mcp_input(None, 0.0, 0.0, 0.0, 0.0, "lapis__search", "", None, None)).unwrap();
        assert_permission("mcp-lapis-bare.json", &bare);
        assert_ne!(bare.choice, "continue");
        assert_eq!(bare.json.contains("\"label\":\"ask\""), true);
        assert!(!bare.json.contains("workflow_is_not_permission"));

        let workflow = decide_permission(&mcp_input(Some("allow"), 0.0, 0.0, 0.0, 0.0, "", "", Some("check"), None)).unwrap();
        assert_permission("mcp-workflow.json", &workflow);
        assert_eq!(workflow.choice, "escalate");
        assert!(workflow.json.contains("\"reason\":\"workflow_is_not_permission\""));
        assert_ne!(workflow.choice, "continue");
        assert_eq!(shell_hook("active", &workflow), permission_golden("mcp-workflow-hook.json"));

        let adopted = decide_permission(&mcp_input(
            Some("allow"),
            0.0,
            0.0,
            0.0,
            0.0,
            "linear__list_issues",
            "read",
            Some("check"),
            Some(facet_typed(true)),
        ))
        .unwrap();
        assert_permission("mcp-typed-facet.json", &adopted);
        assert_eq!(adopted.choice, "continue");
        assert_eq!(adopted.policy_id, ROUTING_POLICY_ID);
        assert!(adopted.json.contains("\"reason\":\"selected\""));
        assert!(!adopted.json.contains("workflow_is_not_permission"));
        assert!(!adopted.auto_allow);

        let still_ask = decide_permission(&mcp_input(
            None,
            0.0,
            0.0,
            0.0,
            0.0,
            "linear__list_issues",
            "read",
            Some("check"),
            Some(facet_typed(false)),
        ))
        .unwrap();
        assert_permission("mcp-typed-not-honor.json", &still_ask);
        assert_ne!(still_ask.choice, "continue");
        assert!(still_ask.json.contains("\"reason\":\"cannot_tell\""));
        assert!(!still_ask.json.contains("workflow_is_not_permission"));
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
            command: String::new(),
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
                command: String::new(),
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
                command: String::new(),
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

    fn arg(text: &str) -> String {
        text.to_string()
    }

    #[test]
    fn g3_catalog_schema_and_check_match_the_offline_oracle() {
        let (code, catalog) = cli_run(&[arg("--catalog")]);
        assert_eq!(code, 0);
        assert!(catalog.contains("\"kind\":\"tiny\""));
        assert!(catalog.contains("\"tools\":[\"Bash\",\"run_terminal_command\",\"run_terminal_cmd\"]"));
        assert!(catalog.contains("\"pattern\":\"[A-Za-z0-9][A-Za-z0-9_.-]*__[A-Za-z0-9_.-]+\""));
        assert!(!catalog.contains("$schema"));
        assert!(!catalog.contains("properties"));
        assert!(!catalog.contains("\"decision\":\"allow\""));
        assert!(catalog.len() < 900);

        let (code, bash) = cli_run(&[arg("--schema"), arg("Bash")]);
        assert_eq!(code, 0);
        assert!(bash.contains("\"call\":\"schema-dump\""));
        assert!(bash.contains("\"decision\":\"defer\""));
        assert!(bash.contains("\"title\":\"Bash\""));
        assert!(bash.contains("\"autoAllow\":false"));
        assert!(bash.contains("\"class\":\"shell\""));
        assert!(bash.contains("\"policyId\":\"omapi-loop-stop-policy@1\""));
        assert!(bash.contains("\"required\":[\"command\"]"));
        assert!(bash.contains("\"command\":{\"type\":\"string\""));
        assert!(!bash.contains("\"decision\":\"allow\""));

        let (code, edit) = cli_run(&[arg("--schema"), arg("search_replace")]);
        assert_eq!(code, 0);
        assert!(edit.contains("\"class\":\"write\""));
        assert!(edit.contains("\"policyId\":\"omapi-check-policy@1\""));
        assert!(edit.contains("\"decision\":\"defer\""));
        assert!(edit.contains("\"autoAllow\":false"));

        let (code, mcp) = cli_run(&[arg("--schema"), arg("linear__save_issue")]);
        assert_eq!(code, 0);
        assert!(mcp.contains("\"class\":\"mcp\""));
        assert!(mcp.contains("\"policyId\":\"omapi-route-workflow-policy@1\""));
        assert!(mcp.contains("\"typedCall\":false"));
        assert!(mcp.contains("\"decision\":\"defer\""));
        assert!(mcp.contains("\"required\":[\"server\",\"tool\"]"));

        for name in ["read_file", "use_tool"] {
            let (code, denied) = cli_run(&[arg("--schema"), arg(name)]);
            assert_eq!(code, 2);
            assert!(denied.contains("\"decision\":\"deny\""));
            assert!(denied.contains("\"found\":false"));
            assert!(denied.contains("\"schema\":null"));
            assert!(denied.contains("\"autoAllow\":false"));
            assert!(!denied.contains("\"decision\":\"allow\""));
        }
        let (code, missing) = cli_run(&[arg("--schema")]);
        assert_eq!(code, 2);
        assert!(missing.contains("\"tool\":null"));
        assert!(missing.contains("\"decision\":\"deny\""));

        let empty_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/jev-router/testdata/empty.diff");
        let (code, empty) = cli_run(&[arg("--check"), arg("--diff-file"), arg(empty_path.to_str().unwrap())]);
        assert_eq!(code, 0);
        assert!(empty.contains("\"status\":\"no_diff\""));
        assert!(empty.contains("\"approval\":false"));
        assert!(empty.contains("\"emptyFindingsAreNotApproval\":true"));
        assert!(empty.contains("no git diff"));
        assert!(!empty.contains("\"approval\":true"));
        assert!(!empty.to_lowercase().contains("approved"));

        let skip_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/jev-router/testdata/skip-marker.diff");
        let (code, skip) = cli_run(&[arg("--check"), arg("--diff-file"), arg(skip_path.to_str().unwrap())]);
        assert_eq!(code, 0);
        assert!(skip.contains("\"approval\":false"));
        assert!(skip.contains("\"emptyFindingsAreNotApproval\":true"));
        assert!(skip.contains("\"flag\":\"skip_marker_added\""));
        assert!(skip.contains("\"id\":\"h1\""));
        assert!(skip.contains("\"source\":\"deterministic\""));
        assert!(skip.contains("\"severity\":\"warn\""));
        assert!(skip.contains("\"path\":\"src/math.test.js\""));
        assert!(skip.contains("\"mode\":\"shadow\""));
        assert!(skip.contains("\"kind\":\"tiny\""));
        assert!(skip.contains("\"hunkCount\":1"));
        assert!(skip.contains("\"parked\":[]"));
        assert!(skip.contains("TYPESAFE_API_KEY unset — unjudged (shadow continues)"));
        assert!(skip.contains("\"workflow\":\"check\""));
        assert!(!skip.contains("\"decision\":\"allow\""));
        assert!(!skip.contains("\"approval\":true"));
    }

    #[test]
    fn hook_command_table_matches_node_stdin() {
        let bash = testdata("grok-pretool-bash.json");
        let rows = [
            (
                bash.as_str(),
                &[("JEV_MODE", "active")][..],
                2,
                "{\"decision\":\"deny\",\"reason\":\"jev choice=escalate gate=hold policy=omapi-loop-stop-policy@1 detail=missing_key question=Is the judge available for this action?\"}\n",
            ),
            (
                bash.as_str(),
                &[("JEV_MODE", "shadow")][..],
                0,
                "{\"decision\":\"defer\",\"reason\":\"shadow\"}\n",
            ),
            (
                "not-json",
                &[("JEV_MODE", "active")][..],
                2,
                "{\"decision\":\"deny\",\"reason\":\"jev uncertain\"}\n",
            ),
            (
                "",
                &[("JEV_MODE", "active")][..],
                2,
                "{\"decision\":\"deny\",\"reason\":\"jev uncertain\"}\n",
            ),
            (
                "",
                &[("JEV_MODE", "shadow")][..],
                0,
                "{\"decision\":\"defer\",\"reason\":\"shadow\"}\n",
            ),
        ];
        for (stdin, env, code, stdout) in rows {
            let (got_code, got) = hook_command(stdin, env);
            assert_eq!(got_code, code, "{stdin:?} {env:?}");
            assert_eq!(got, stdout, "{stdin:?} {env:?}");
        }
    }

    #[test]
    fn g4_fixture_replays_system_one_bytes_and_only_cli_builds_a_client() {
        assert!(!LIVE_ON);
        assert_eq!(
            live_transport_argv(),
            ["facet", "request", "run", "--environment", "typesafe", "--no-record"]
        );
        assert_eq!(live_transport_argv().join(" "), LIVE_COMMAND);
        let client = fixture_client();
        for name in ["check-judge.json", "shadow-workflow.json", "main-gate.json"] {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/jev-router/testdata/system-one")
                .join(name);
            let recorded = std::fs::read(&path).expect(name);
            let replayed = client.system_one(name).expect(name);
            assert_eq!(replayed, recorded, "{name}");
            assert!(matches_recorded(&replayed, &recorded));
            let mut changed = recorded.clone();
            let index = changed.len() / 2;
            changed[index] ^= 0x01;
            assert_ne!(changed, recorded, "{name}");
            assert!(!matches_recorded(&changed, &recorded), "{name}");
        }
        assert!(client.system_one("missing.json").is_err());

        let call = tool_call();
        assert_eq!(call.transport, "facet");
        assert!(!call.initiated);
        assert_eq!(call.op, "tool_call");
        assert_eq!(call.facet_version, "2.1.3");
        let missing = missing_key_envelope();
        assert!(missing.contains("\"reason\":\"missing_key\""));
        assert!(missing.contains("\"initiated\":false"));
        assert!(!missing.contains("\"initiated\":true"));

        let live = live_transport_argv().join(" ");
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(&src).expect("src") {
            let path = entry.expect("entry").path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("read");
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let typesafe = ["TypeSafe", "Client"].join("");
            let sdk = ["@typesafe-ai", "sdk"].join("/");
            let ctor = ["construct", "client"].join("_");
            assert!(!text.contains(&typesafe), "{name}");
            assert!(!text.contains(&sdk), "{name}");
            if name != "cli.rs" {
                assert!(!text.contains(&ctor), "{name}");
                assert!(!text.contains(&live), "{name}");
            }
        }
    }

    #[test]
    fn g2_three_system_one_sites_byte_match_the_node_answer() {
        assert!(!LIVE_ON);
        let client = fixture_client();
        let sites = [
            (CHECK_JUDGE, "check-judge.json"),
            (SHADOW_WORKFLOW, "shadow-workflow.json"),
            (MAIN_GATE, "main-gate.json"),
        ];
        for (site, name) in sites {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/jev-router/testdata/system-one")
                .join(name);
            let recorded = std::fs::read(&path).expect(site);
            let replayed = recorded_answer(&client, site).expect(site);
            assert!(matches_recorded(&replayed, &recorded), "{site}");
            let text = String::from_utf8(recorded.clone()).expect(site);
            assert!(text.contains("\"model\":\"jev-latest\""), "{site}");
            assert!(text.contains("\"type\":"), "{site}");
            let paraphrase = text.replace("\"type\":\"choice\"", "\"kind\":\"choice\"");
            assert!(!matches_recorded(paraphrase.as_bytes(), &recorded), "{site}");
        }
        let call = tool_call();
        assert_eq!(call.transport, "facet");
        assert!(!call.initiated);
    }

    #[test]
    fn g1_missing_key_envelope_matches_calls_mjs() {
        let body = missing_key_envelope();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/jev-router/testdata/calls/missing-key.json");
        let recorded = std::fs::read(&path).unwrap();
        assert_eq!(body.as_bytes(), recorded.as_slice());
        let recorded = std::str::from_utf8(&recorded).unwrap();
        assert!(envelope_matches(&body, recorded));
        assert!(body.contains("\"facetVersion\":\"2.1.3\""));
        assert!(body.contains("\"transport\":\"facet\""));
        assert!(body.contains("\"initiated\":false"));
        assert!(body.contains("\"autoPromote\":false"));
        assert!(body.contains("\"autoAllow\":false"));
        assert!(body.contains("\"mode\":\"shadow\""));
        assert!(body.contains("\"honor\":false"));
        assert!(body.contains("\"reason\":\"missing_key\""));
        assert!(!body.contains("\"label\":\"allow\""));
        assert!(!body.contains("\"choice\":\"allow\""));
        assert!(!body.contains("F454"));
        let mut paraphrased = body.clone().into_bytes();
        let index = paraphrased.iter().position(|byte| *byte == b'f').unwrap();
        paraphrased[index] = b'g';
        let paraphrased = String::from_utf8(paraphrased).unwrap();
        assert!(!envelope_matches(&paraphrased, recorded));
    }

    fn calls_golden(name: &str) -> Vec<u8> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/jev-router/testdata/calls")
            .join(name);
        std::fs::read(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
    }

    #[test]
    fn g2_hard_stops_match_calls_mjs_and_are_not_promoted() {
        let next = HardStopTool {
            name: "linear__list_issues",
            description: "List issues",
            effect: "read",
        };
        let cases = [
            (
                "invalid-fn.json",
                "linear.save",
                HardStopTool {
                    name: "linear.save",
                    description: "Bad name",
                    effect: "read",
                },
                "F452",
            ),
            (
                "missing-effect.json",
                "bare__tool",
                HardStopTool {
                    name: "bare__tool",
                    description: "No effect",
                    effect: "",
                },
                "F456",
            ),
            (
                "invalid-effect.json",
                "odd__tool",
                HardStopTool {
                    name: "odd__tool",
                    description: "Odd",
                    effect: "not-an-effect",
                },
                "F456",
            ),
            (
                "payment.json",
                "pay__now",
                HardStopTool {
                    name: "pay__now",
                    description: "Pay",
                    effect: "payment",
                },
                "F454",
            ),
        ];
        for (file, winner, tool, code) in cases {
            let body = hard_stop_json(
                winner,
                &[tool, next],
                &[(winner, 0.8), ("linear__list_issues", 0.2)],
            );
            let recorded = calls_golden(file);
            assert_eq!(body.as_bytes(), recorded.as_slice(), "{file}");
            assert!(body.contains(&format!("\"code\":\"{code}\"")), "{file}");
            assert!(body.contains("\"best\":null"), "{file}");
            assert!(body.contains("\"autoPromote\":false"), "{file}");
            assert!(body.contains("\"mapped\":\"stop\""), "{file}");
            assert!(body.contains("\"choice\":\"stop\""), "{file}");
            assert!(!body.contains("\"name\":\"linear__list_issues\",\"outcome\""));
            let mut paraphrased = body.into_bytes();
            paraphrased[0] = b' ';
            assert_ne!(paraphrased, recorded, "{file}");
        }
    }

    #[test]
    fn g3_obvious_effects_continue_twice_and_uncertain_asks_before_a_human() {
        let effects = ["read", "write", "filesystem", "network", "external"];
        for _ in 0..2 {
            for effect in effects {
                let body = obvious_effect_json(effect, "active");
                let recorded = calls_golden(&format!("{effect}.json"));
                assert_calls_bytes(effect, &body, &recorded);
                assert!(body.contains("\"mapped\":\"continue\""), "{effect}");
                assert!(body.contains("\"choice\":\"continue\""), "{effect}");
                assert!(body.contains("\"blocked\":false"), "{effect}");
                assert!(body.contains("\"initiated\":false"), "{effect}");
                assert!(body.contains("\"autoPromote\":false"), "{effect}");
                assert!(body.contains("\"autoAllow\":false"), "{effect}");
                assert!(body.contains(&format!("\"effect\":\"{effect}\"")), "{effect}");
                assert!(!body.contains("F454"), "{effect}");
            }
        }
        let shadow = obvious_effect_json("write", "shadow");
        assert_calls_bytes("write-shadow", &shadow, &calls_golden("write-shadow.json"));
        assert!(shadow.contains("\"honor\":false"));
        assert!(shadow.contains("\"blocked\":false"));
        assert!(shadow.contains("\"choice\":\"unclassified\""));
        assert!(shadow.contains("\"mapped\":\"continue\""));
        assert!(!shadow.contains("F454"));

        let ask = model_uncertain_json();
        assert_calls_bytes("model-uncertain", &ask, &calls_golden("model-uncertain.json"));
        assert!(ask.contains("\"mapped\":\"ask\""));
        assert!(ask.contains("\"reason\":\"model_uncertain\""));
        assert!(ask.contains("confidence 0.6"));
        assert!(ask.contains("probability 0.55"));
        assert!(ask.contains("margin 0.15"));
        assert!(ask.contains("\"hitl\":false"));
        assert!(ask.contains("\"blocked\":false"));
        assert!(!ask.contains("F454"));

        let human = cannot_tell_json();
        assert_calls_bytes("cannot-tell", &human, &calls_golden("cannot-tell.json"));
        assert!(human.contains("\"mapped\":\"escalate\""));
        assert!(human.contains("detail="));
        assert!(human.contains("question="));
        assert!(human.contains("\"hitl\":true"));
        assert!(!human.contains("F454"));
    }

    fn assert_calls_bytes(label: &str, body: &str, recorded: &[u8]) {
        if body.as_bytes() == recorded {
            return;
        }
        let n = body.len().min(recorded.len());
        let mut at = 0;
        while at < n && body.as_bytes()[at] == recorded[at] {
            at += 1;
        }
        let start = at.saturating_sub(70);
        let rust_end = (at + 70).min(body.len());
        let node_end = (at + 70).min(recorded.len());
        panic!(
            "{label} differ at {at} ({} vs {})\nRUST {}\nNODE {}",
            body.len(),
            recorded.len(),
            &body[start..rust_end],
            String::from_utf8_lossy(&recorded[start..node_end]),
        );
    }

    fn router_golden(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/jev-router/testdata/router")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
    }

    fn clean_repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bezel-router-g3-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            let run = std::process::Command::new("git")
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
                .unwrap_or_else(|err| panic!("git {}: {err}", args.join(" ")));
            assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
        };
        git(&["init"]);
        git(&[
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=jev",
            "-c",
            "user.email=jev@example.com",
            "commit",
            "--allow-empty",
            "-m",
            "init",
        ]);
        dir
    }

    #[test]
    fn g3_bash_no_key_matches_node_active_and_shadow() {
        let repo = clean_repo();
        let active = bash_no_key("active", &repo);
        let shadow = bash_no_key("shadow", &repo);
        assert_eq!(active, router_golden("bash-active.json"));
        assert_eq!(shadow, router_golden("bash-shadow.json"));
        assert!(active.contains("\"missingKey\":true"));
        assert!(active.contains("\"choice\":\"escalate\""));
        assert!(active.contains("\"gate\":\"hold\""));
        assert!(active.contains("\"policy\":\"omapi-loop-stop-policy@1\""));
        assert!(!active.contains("jev uncertain"));
        assert!(shadow.contains("\"blocked\":false"));
        assert!(shadow.contains("\"choice\":\"unclassified\""));
        assert!(!shadow.contains("\"blocked\":true"));
        assert!(!shadow.contains("jev uncertain"));
        let _ = std::fs::remove_dir_all(&repo);

        let dirty = dirty_repo();
        let dirty_shadow = bash_no_key("shadow", &dirty);
        let dirty_active = bash_no_key("active", &dirty);
        assert_eq!(dirty_shadow, router_golden("bash-shadow-dirty.json"));
        assert_eq!(dirty_active, router_golden("bash-active-dirty.json"));
        assert!(dirty_shadow.contains("\"available\":[\"check\",\"review\"]"));
        assert!(dirty_shadow.contains("\"check\":true"));
        assert!(dirty_shadow.contains("\"review\":true"));
        assert!(dirty_shadow.contains("\"blocked\":false"));
        assert!(!dirty_shadow.contains("jev uncertain"));
        assert!(dirty_active.contains("\"choice\":\"escalate\""));
        assert!(dirty_active.contains("\"gate\":\"hold\""));
        assert!(dirty_active.contains("\"missingKey\":true"));
        assert!(!dirty_active.contains("jev uncertain"));
        let _ = std::fs::remove_dir_all(&dirty);
    }

    fn dirty_repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bezel-router-g3-dirty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            let run = std::process::Command::new("git")
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
                .unwrap_or_else(|err| panic!("git {}: {err}", args.join(" ")));
            assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
        };
        git(&["init"]);
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        git(&[
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=jev",
            "-c",
            "user.email=jev@example.com",
            "add",
            "a.txt",
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
        std::fs::write(dir.join("a.txt"), "two\n").unwrap();
        dir
    }

    fn facts_golden(name: &str) -> String {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/jev-router/testdata/facts")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
    }

    #[test]
    fn g1_gather_matches_facts_mjs_on_the_same_fixtures() {
        assert_eq!(clip_to_max_hunk_chars("abc", Some(2)), "ab");
        assert_eq!(MAX_HUNK_CHARS, 4000);

        let contents = WriteBody {
            path: "notes/a.md".to_string(),
            contents: Some("hello\n".to_string()),
            ..WriteBody::default()
        };
        assert_eq!(write_tool_wire(&contents), facts_golden("wire-contents.txt"));

        let edits = WriteBody {
            path: "src/a.test.js".to_string(),
            edits: Some(vec![Edit {
                old_string: "expect(1)".to_string(),
                new_string: "expect(2)".to_string(),
            }]),
            ..WriteBody::default()
        };
        assert_eq!(write_tool_wire(&edits), facts_golden("wire-edits.txt"));

        let long = WriteBody {
            file_path: Some("notes/a.md".to_string()),
            old_string: Some("old".to_string()),
            new_string: Some("Z".repeat(5000)),
            ..WriteBody::default()
        };
        let wire = write_tool_wire(&long);
        assert_eq!(wire, facts_golden("wire-long.txt"));
        assert!(wire.contains("\"proposed\":"));
        assert!(!wire.ends_with("src/app.js"));
        assert_eq!(write_tool_wire(&WriteBody {
            file_path: Some("src/app.js".to_string()),
            ..WriteBody::default()
        }), facts_golden("wire-path.txt"));

        let skip = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/jev-router/testdata/skip-marker.diff");
        let gathered = gather(&GatherOpts {
            diff_file: Some(skip.clone()),
            write: Some(long),
            ..GatherOpts::default()
        });
        assert_eq!(gathered.proposed, facts_golden("wire-long.txt"));
        assert_eq!(gathered.diff.text, std::fs::read_to_string(&skip).unwrap());
        assert_eq!(gathered.diff.source, "file");
        assert!(gathered.diff.present);
        assert_eq!(gathered.available, vec!["check".to_string(), "review".to_string()]);
        assert!(gathered.proposed.len() > "notes/a.md".len());

        let empty = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/jev-router/testdata/empty.diff");
        let empty_got = gather(&GatherOpts {
            diff_file: Some(empty),
            ..GatherOpts::default()
        });
        assert!(!empty_got.diff.present);
        assert!(empty_got.available.is_empty());
        assert_eq!(empty_got.diff.source, "file");

        let dir = std::env::temp_dir().join(format!("bezel-router-g1-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            let run = std::process::Command::new("git")
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
                .unwrap_or_else(|err| panic!("git {}: {err}", args.join(" ")));
            assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
        };
        git(&["init"]);
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        git(&["-c", "commit.gpgsign=false", "-c", "user.name=jev", "-c", "user.email=jev@example.com", "add", "a.txt"]);
        git(&["-c", "commit.gpgsign=false", "-c", "user.name=jev", "-c", "user.email=jev@example.com", "commit", "-m", "init"]);
        std::fs::write(dir.join("a.txt"), "two\n").unwrap();
        let worktree = gather(&GatherOpts {
            repo: dir.clone(),
            ..GatherOpts::default()
        });
        let git_text = std::process::Command::new("git")
            .args(["diff", "--no-color", "--no-ext-diff"])
            .current_dir(&dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap();
        assert!(git_text.status.success());
        assert_eq!(worktree.diff.source, "git-worktree");
        assert_eq!(worktree.diff.text, String::from_utf8(git_text.stdout).unwrap());
        assert!(worktree.diff.present);
        assert!(worktree.diff.text.contains("two"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
