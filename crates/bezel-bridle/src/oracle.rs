//! Behaviors once asserted by the router package tests.
//! Each test calls a shipped function.

use super::*;
use super::{calls, cli, facts, loop_stop, permission, policy};
use std::path::PathBuf;

fn root(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn read_rel(rel: &str) -> String {
    std::fs::read_to_string(root(rel)).unwrap_or_else(|err| panic!("{rel}: {err}"))
}

fn permit<'a>(
    class_id: &'static str,
    requested_mode: &'static str,
    has_key: bool,
    content_present: bool,
    label: Option<&'static str>,
    confidence: f64,
    allow_p: f64,
    deny_p: f64,
    ask_p: f64,
    path: &'a str,
    diff: &'a str,
    concern: Option<f64>,
    tool_name: &'a str,
    effect: &'a str,
    routing_outcome: Option<&'a str>,
    typed: Option<TypedPermission<'a>>,
) -> PermissionInput<'a> {
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
        diff,
        concern,
        flags: None,
        tool_name,
        effect,
        routing_outcome,
        typed,
    }
}

fn set_env(key: &str, value: Option<&str>) {
    unsafe {
        match value {
            Some(text) => std::env::set_var(key, text),
            None => std::env::remove_var(key),
        }
    }
}

#[test]
fn routing_policy_versions_and_thresholds() {
    assert_eq!(ROUTING_POLICY_ID, "omapi-route-workflow-policy@1");
    assert_eq!(LOOP_STOP_MIN_CONFIDENCE, 0.6);
    assert_eq!(LOOP_STOP_MIN_PROBABILITY, 0.55);
    assert_eq!(LOOP_STOP_MIN_MARGIN, 0.15);
    assert!(!LIVE_ON);
}

#[test]
fn apply_routing_thresholds_selects_a_confident_available_pick() {
    let decision = policy::apply_routing_thresholds(
        "check",
        0.8,
        &[("check", 0.7), ("review", 0.2), ("cannot_tell", 0.1)],
        &["check", "review"],
    );
    assert_eq!(decision.outcome, "check");
    assert_eq!(decision.reason, "selected");
}

#[test]
fn apply_routing_thresholds_parks_uncertain_and_unavailable() {
    let uncertain = policy::apply_routing_thresholds(
        "check",
        0.4,
        &[("check", 0.4), ("review", 0.35), ("cannot_tell", 0.25)],
        &["check", "review"],
    );
    assert_eq!(uncertain.outcome, "cannot_tell");
    assert_eq!(uncertain.reason, "model_uncertain");
    let unavailable = policy::apply_routing_thresholds(
        "check",
        0.9,
        &[("check", 0.8), ("review", 0.1), ("cannot_tell", 0.1)],
        &[],
    );
    assert_eq!(unavailable.outcome, "cannot_tell");
    assert_eq!(unavailable.reason, "unavailable");
}

#[test]
fn available_gate_without_a_diff() {
    assert!(!facts::diff_present(""));
    assert!(!facts::diff_present("   \n"));
    let caps = facts::available_capabilities(false);
    assert!(!caps.check);
    assert!(!caps.review);
    assert!(caps.unavailable_check.contains("diff"));
}

#[test]
fn available_gate_with_a_diff() {
    let caps = facts::available_capabilities(true);
    assert!(caps.check);
    assert!(caps.review);
}

#[test]
fn gather_diff_from_file_and_facts() {
    let skip = root("../../packages/jev-router/testdata/skip-marker.diff");
    let empty = root("../../packages/jev-router/testdata/empty.diff");
    let diff = gather_diff(&GatherOpts {
        diff_file: Some(skip),
        ..GatherOpts::default()
    });
    assert!(diff.present);
    assert_eq!(diff.source, "file");
    let facts = gather(&GatherOpts {
        diff_file: Some(root("../../packages/jev-router/testdata/skip-marker.diff")),
        ..GatherOpts::default()
    });
    assert!(facts.diff.present);
    assert_eq!(facts.available, ["check", "review"]);
    let blank = gather(&GatherOpts {
        diff_file: Some(empty),
        ..GatherOpts::default()
    });
    assert!(!blank.diff.present);
    assert!(blank.available.is_empty());
}

#[test]
fn apply_check_thresholds_buckets_concern() {
    assert_eq!(CHECK_POLICY_ID, "omapi-check-policy@1");
    let finding = policy::apply_check_thresholds(
        0.85,
        "test_safety",
        0.8,
        &[
            ("test_safety", 0.7),
            ("none", 0.2),
            ("task_mismatch", 0.05),
            ("cannot_tell", 0.05),
        ],
    );
    assert_eq!(finding.bucket, "finding");
    assert_eq!(finding.flag.as_deref(), Some("test_safety"));
    let parked = policy::apply_check_thresholds(
        0.5,
        "task_mismatch",
        0.8,
        &[
            ("task_mismatch", 0.6),
            ("none", 0.2),
            ("test_safety", 0.1),
            ("cannot_tell", 0.1),
        ],
    );
    assert_eq!(parked.bucket, "parked");
    assert_eq!(parked.reason, "concern_in_band");
    let clear = policy::apply_check_thresholds(
        0.1,
        "none",
        0.8,
        &[("none", 0.8), ("test_safety", 0.1), ("task_mismatch", 0.05), ("cannot_tell", 0.05)],
    );
    assert_eq!(clear.bucket, "clear");
}

#[test]
fn run_check_skip_marker_is_not_approval() {
    let diff = std::fs::read_to_string(root("../../packages/jev-router/testdata/skip-marker.diff")).unwrap();
    let report = run_offline_check(&diff);
    let json = offline_check_json(&report);
    assert!(json.contains("\"approval\":false"));
    assert!(json.contains("\"emptyFindingsAreNotApproval\":true"));
    assert!(json.contains("\"workflow\":\"check\""));
    assert!(json.contains("\"flag\":\"skip_marker_added\""));
    assert!(json.contains("TYPESAFE_API_KEY unset"));
    assert!(!json.to_lowercase().contains("approved"));
}

#[test]
fn run_check_assertions_removed_is_not_approval() {
    let diff = std::fs::read_to_string(root("../../packages/jev-router/testdata/assertions-removed.diff")).unwrap();
    let json = offline_check_json(&run_offline_check(&diff));
    assert!(json.contains("\"flag\":\"assertions_removed\""));
    assert!(json.contains("\"approval\":false"));
}

#[test]
fn run_check_empty_diff_is_no_diff() {
    let json = offline_check_json(&run_offline_check(""));
    assert!(json.contains("\"status\":\"no_diff\""));
    assert!(json.contains("\"approval\":false"));
    assert!(json.contains("no git diff"));
    assert!(json.contains("\"findings\":[]"));
}

#[test]
fn proposed_write_clips_and_is_the_permission_diff() {
    assert_eq!(MAX_HUNK_CHARS, 4000);
    assert_eq!(clip_to_max_hunk_chars("abc", Some(2)), "ab");
    let body = format!(
        "it.skip(\"keeps the marker\", () => {{\n  expect(1).toBe(1);\n}});\n{}",
        "Z".repeat(5000)
    );
    let wire = write_tool_wire(&WriteBody {
        file_path: Some("src/math.test.js".into()),
        old_string: Some("it(\"adds\", () => {\n  expect(1).toBe(1);\n});".into()),
        new_string: Some(body),
        ..WriteBody::default()
    });
    let (path, diff) = facts::parse_write_tool_wire(&wire);
    assert_eq!(path, "src/math.test.js");
    assert_eq!(diff.len(), 4000);
    assert!(diff.contains("+it.skip"));
    assert!(!diff.contains(&"Z".repeat(4000)));
    let flagged = decide_permission(&permit(
        "write", "active", true, false, None, 0.0, 0.0, 0.0, 0.0, &path, &diff, None, "", "", None, None,
    ))
    .unwrap();
    assert!(flagged.json.contains("\"reason\":\"skip_marker_added\""));
    assert!(flagged.json.contains("\"codeDeny\":true"));
    assert!(!flagged.auto_allow);

    let (created_path, created_diff) = facts::parse_write_tool_wire(&write_tool_wire(&WriteBody {
        path: "notes/a.md".into(),
        contents: Some("hello\n".into()),
        ..WriteBody::default()
    }));
    assert_eq!(created_path, "notes/a.md");
    assert!(created_diff.contains("+hello"));
    let (multi_path, multi_diff) = facts::parse_write_tool_wire(&write_tool_wire(&WriteBody {
        path: "src/a.test.js".into(),
        edits: Some(vec![Edit {
            old_string: "expect(1)".into(),
            new_string: "expect(2)".into(),
        }]),
        ..WriteBody::default()
    }));
    assert_eq!(multi_path, "src/a.test.js");
    assert!(multi_diff.contains("-expect(1)"));
    assert!(multi_diff.contains("+expect(2)"));

    let (only_path, only_diff) = facts::parse_write_tool_wire("src/app.js");
    assert_eq!(only_path, "src/app.js");
    assert!(only_diff.is_empty());
    let empty = decide_permission(&permit(
        "write", "active", true, false, None, 0.0, 0.0, 0.0, 0.0, &only_path, &only_diff, None, "", "", None, None,
    ))
    .unwrap();
    assert!(empty.json.contains("\"reason\":\"empty_findings_not_approval\""));
    assert!(empty.json.contains("\"codeDeny\":false"));

    let (secret_path, secret_diff) = facts::parse_write_tool_wire(".env");
    let denied = decide_permission(&permit(
        "write",
        "shadow",
        false,
        false,
        None,
        0.0,
        0.0,
        0.0,
        0.0,
        &secret_path,
        &secret_diff,
        None,
        "",
        "",
        None,
        None,
    ))
    .unwrap();
    assert!(denied.json.contains("\"reason\":\"secret_path\""));
    assert!(denied.json.contains("\"codeDeny\":true"));

    let over = facts::parse_write_tool_wire(&format!(
        "{{\"path\":\"a.md\",\"proposed\":\"{}\"}}",
        "Q".repeat(5000)
    ));
    assert_eq!(over.0, "a.md");
    assert_eq!(over.1, "Q".repeat(4000));
}

#[test]
fn write_hook_forwards_a_clipped_redacted_edit() {
    let secret = "typesafe-test-key";
    let new_string = format!("{secret}\n{}", "Z".repeat(5000));
    let scrubbed = facts::scrub_secret_text(&new_string, &[secret]);
    let wire = write_tool_wire(&WriteBody {
        file_path: Some("notes/a.md".into()),
        old_string: Some("old".into()),
        new_string: Some(scrubbed),
        ..WriteBody::default()
    });
    assert_ne!(wire, "notes/a.md");
    let (path, proposed) = facts::parse_write_tool_wire(&wire);
    assert_eq!(path, "notes/a.md");
    assert_eq!(proposed.len(), 4000);
    assert!(!proposed.contains(secret));
    assert!(proposed.contains("[redacted]"));
    assert!(proposed.contains('Z'));
    assert!(!wire.contains(secret));
    let hook = decide_hook(
        &HookEvent {
            tool_name: "search_replace".into(),
            command: String::new(),
        },
        &modes_from_env(&[("JEV_MODE", "shadow")]),
        Some(&GateVerdict::simple("shadow", "continue", "auto", false)),
    );
    assert_eq!(hook.decision, "defer");
    assert_ne!(hook.decision, "allow");
}

#[test]
fn check_stays_on_the_git_worktree_and_empty_findings_are_not_approval() {
    let dir = std::env::temp_dir().join(format!("bezel-oracle-wt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        let run = std::process::Command::new("git")
            .args(args)
            .current_dir(&dir)
            .env("HOME", &dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap();
        assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    };
    git(&["init"]);
    std::fs::write(dir.join("a.js"), "test('ok', () => { expect(1).toBe(1); });\n").unwrap();
    git(&["add", "a.js"]);
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
    std::fs::write(dir.join("a.js"), "test.skip('ok', () => { expect(1).toBe(1); });\n").unwrap();
    let worktree = gather_diff(&GatherOpts {
        repo: dir.clone(),
        ..GatherOpts::default()
    });
    assert_eq!(worktree.source, "git-worktree");
    assert!(worktree.text.contains("test.skip"));
    let checked = offline_check_json(&run_offline_check(&worktree.text));
    assert!(checked.contains("\"approval\":false"));
    assert!(checked.contains("\"flag\":\"skip_marker_added\""));
    let (path, diff) = facts::parse_write_tool_wire("a.js");
    assert!(diff.is_empty());
    let ignored = decide_permission(&permit(
        "write", "active", true, false, None, 0.0, 0.0, 0.0, 0.0, &path, &diff, None, "", "", None, None,
    ))
    .unwrap();
    assert!(ignored.json.contains("\"reason\":\"empty_findings_not_approval\""));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn fake_judge_parks_and_does_not_approve() {
    let parked = policy::apply_check_thresholds(
        0.5,
        "test_safety",
        0.8,
        &[("test_safety", 0.7), ("none", 0.1), ("task_mismatch", 0.1), ("cannot_tell", 0.1)],
    );
    assert_eq!(parked.bucket, "parked");
    let diff = std::fs::read_to_string(root("../../packages/jev-router/testdata/skip-marker.diff")).unwrap();
    let json = offline_check_json(&run_offline_check(&diff));
    assert!(json.contains("\"approval\":false"));
    assert!(!json.contains("\"approval\":true"));
}

#[test]
fn shadow_check_without_a_key_emits_the_envelope() {
    let _lock = cli::test_env_lock();
    set_env("JEV_MODE", Some("shadow"));
    set_env("TYPESAFE_API_KEY", None);
    let skip = root("../../packages/jev-router/testdata/skip-marker.diff");
    let (code, out) = cli::run(&[
        "--check".into(),
        "--intent".into(),
        "probe".into(),
        "--diff-file".into(),
        skip.to_str().unwrap().into(),
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"approval\":false"));
    assert!(out.contains("\"emptyFindingsAreNotApproval\":true"));
    assert!(out.contains("\"missingKey\":true"));
    assert!(out.contains("\"flag\":\"skip_marker_added\""));
    assert!(!out.to_lowercase().contains("approved"));
    set_env("JEV_MODE", None);
}

#[test]
fn shadow_router_without_a_key_continues_unclassified() {
    let _lock = cli::test_env_lock();
    set_env("JEV_MODE", Some("shadow"));
    set_env("JEV_BYPASS", None);
    set_env("TYPESAFE_API_KEY", None);
    let (code, out) = cli::run(&["probe intent".into()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"choice\":\"unclassified\""));
    assert!(out.contains("\"blocked\":false"));
    assert!(out.contains("\"missingKey\":true"));
    assert!(out.contains("\"shadow\":true"));
    assert!(out.contains("\"diffPresent\":"));
    assert!(out.contains("\"autoRetry\":false"));
    assert!(out.contains("\"autoPromote\":false"));
    assert!(out.contains("\"reason\":\"missing_key\""));
    assert!(out.contains("\"loopStopPolicy\":\"omapi-loop-stop-policy@1\""));
    set_env("JEV_MODE", None);
}

#[test]
fn loop_stop_versions_select_continue_stop_and_sticky_escalate() {
    assert_eq!(LOOP_STOP_POLICY_ID, "omapi-loop-stop-policy@1");
    assert_eq!(policy::MAX_DIGEST_CHARS, 4000);
    let cont = apply_loop_stop_thresholds("continue", 0.85, &[("continue", 0.7), ("stop", 0.2), ("escalate", 0.1)]);
    assert_eq!(cont.outcome, "continue");
    assert_eq!(cont.reason, "selected");
    let stop = apply_loop_stop_thresholds("stop", 0.85, &[("stop", 0.72), ("continue", 0.18), ("escalate", 0.1)]);
    assert_eq!(stop.outcome, "stop");
    assert_eq!(stop.reason, "selected");
    let sure = apply_loop_stop_thresholds("escalate", 0.9, &[("escalate", 0.8), ("continue", 0.1), ("stop", 0.1)]);
    assert_eq!(sure.outcome, "escalate");
    assert_eq!(sure.reason, "selected");
    let unsure = apply_loop_stop_thresholds("escalate", 0.3, &[("escalate", 0.4), ("continue", 0.35), ("stop", 0.25)]);
    assert_eq!(unsure.outcome, "escalate");
    assert_eq!(unsure.reason, "escalate_uncertain");
    let uncertain = apply_loop_stop_thresholds("continue", 0.4, &[("continue", 0.4), ("stop", 0.35), ("escalate", 0.25)]);
    assert_eq!(uncertain.outcome, "cannot_tell");
    assert_eq!(uncertain.reason, "model_uncertain");
}

#[test]
fn decide_loop_stop_maps_gate_hitl_and_shadow_exec() {
    let cont = loop_stop::decide_loop_stop(
        "continue",
        0.85,
        &[("continue", 0.7), ("stop", 0.2), ("escalate", 0.1)],
        "active",
        "ship it",
        "step 1 ok",
    );
    assert_eq!(cont.choice, "continue");
    assert_eq!(cont.gate, "auto");
    assert!(!cont.blocked);
    assert!(cont.exec);
    assert!(!cont.hitl);
    assert!(!cont.auto_retry);
    assert!(!cont.auto_promote);
    assert_eq!(cont.step_digest, "step 1 ok");

    let stop = loop_stop::decide_loop_stop(
        "stop",
        0.85,
        &[("stop", 0.72), ("continue", 0.18), ("escalate", 0.1)],
        "active",
        "done",
        "",
    );
    assert_eq!(stop.choice, "stop");
    assert_eq!(stop.gate, "hold");
    assert!(stop.blocked);
    assert!(!stop.exec);

    let esc = loop_stop::decide_loop_stop(
        "escalate",
        0.85,
        &[("escalate", 0.8), ("continue", 0.1), ("stop", 0.1)],
        "active",
        "need a human",
        "",
    );
    assert_eq!(esc.choice, "escalate");
    assert_eq!(esc.gate, "hold");
    assert!(esc.hitl);
    assert!(esc.loop_hitl);
    assert!(!esc.auto_retry);
    assert!(esc.blocked);
    assert!(!esc.exec);

    let answer = ("continue", 0.3, &[("continue", 0.4), ("stop", 0.3), ("escalate", 0.3)][..]);
    let shadow = loop_stop::decide_loop_stop(answer.0, answer.1, answer.2, "shadow", "maybe", "");
    assert_eq!(shadow.choice, "unclassified");
    assert_eq!(shadow.gate, "auto");
    assert!(!shadow.blocked);
    assert!(shadow.exec);
    assert!(!shadow.hitl);
    let active = loop_stop::decide_loop_stop(answer.0, answer.1, answer.2, "active", "maybe", "");
    assert_eq!(active.choice, "escalate");
    assert_eq!(active.gate, "hold");
    assert!(active.hitl);
    assert!(active.blocked);
    assert!(!active.exec);
    assert!(!active.auto_retry);

    let stop_ans = ("stop", 0.9, &[("stop", 0.8), ("continue", 0.1), ("escalate", 0.1)][..]);
    let shadow_stop = loop_stop::decide_loop_stop(stop_ans.0, stop_ans.1, stop_ans.2, "shadow", "", "");
    assert_eq!(shadow_stop.choice, "stop");
    assert!(shadow_stop.exec);
    assert!(!shadow_stop.blocked);
    let active_stop = loop_stop::decide_loop_stop(stop_ans.0, stop_ans.1, stop_ans.2, "active", "", "");
    assert!(!active_stop.exec);
    let esc_ans = ("escalate", 0.9, &[("escalate", 0.8), ("continue", 0.1), ("stop", 0.1)][..]);
    assert!(loop_stop::decide_loop_stop(esc_ans.0, esc_ans.1, esc_ans.2, "shadow", "", "").exec);
    assert!(!loop_stop::decide_loop_stop(esc_ans.0, esc_ans.1, esc_ans.2, "active", "", "").exec);
}

#[test]
fn should_exec_agent_shadow_always_and_active_continue_only() {
    assert!(policy::should_exec_agent("shadow", "stop", "hold", true));
    assert!(policy::should_exec_agent("shadow", "escalate", "hold", false));
    assert!(policy::should_exec_agent("active", "continue", "auto", false));
    assert!(policy::should_exec_agent("active", "unclassified", "auto", false));
    assert!(!policy::should_exec_agent("active", "stop", "hold", false));
    assert!(!policy::should_exec_agent("active", "escalate", "hold", false));
    assert!(!policy::should_exec_agent("active", "hold_for_human", "hold", false));
}

#[test]
fn clip_digest_and_missing_key_loop() {
    assert_eq!(clip_to_max_hunk_chars(&"x".repeat(5000), Some(4000)).len(), 4000);
    assert_eq!(clip_to_max_hunk_chars("short", Some(4000)), "short");
    let shadow = loop_stop::missing_key_loop("shadow", "probe", "digest");
    assert_eq!(shadow.choice, "unclassified");
    assert!(shadow.exec);
    assert_eq!(shadow.loop_reason, "missing_key");
    assert!(!shadow.auto_promote);
    let active = loop_stop::missing_key_loop("active", "probe", "");
    assert_eq!(active.choice, "escalate");
    assert_eq!(active.gate, "hold");
    assert!(active.hitl);
    assert!(!active.exec);
    assert!(!active.auto_retry);
}

#[test]
fn shadow_loop_stop_and_check_ignore_bypass() {
    let _lock = cli::test_env_lock();
    set_env("JEV_MODE", Some("shadow"));
    set_env("JEV_BYPASS", None);
    set_env("TYPESAFE_API_KEY", None);
    let (code, out) = cli::run(&[
        "--loop-stop".into(),
        "--intent".into(),
        "probe".into(),
        "--step-digest".into(),
        "step one failed".into(),
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("\"choice\":\"unclassified\""));
    assert!(out.contains("\"blocked\":false"));
    assert!(out.contains("\"exec\":true"));
    assert!(out.contains("\"stepDigest\":\"step one failed\""));
    assert!(out.contains("\"loopStopPolicy\":\"omapi-loop-stop-policy@1\""));
    let skip = root("../../packages/jev-router/testdata/skip-marker.diff");
    let (code, checked) = cli::run(&[
        "--check".into(),
        "--intent".into(),
        "probe".into(),
        "--diff-file".into(),
        skip.to_str().unwrap().into(),
        "--step-digest".into(),
        "ignored-for-check".into(),
    ]);
    assert_eq!(code, 0, "{checked}");
    assert!(checked.contains("\"workflow\":\"check\""));
    assert!(checked.contains("\"approval\":false"));
    assert!(checked.contains("\"flag\":\"skip_marker_added\""));
    set_env("JEV_MODE", None);
}

#[test]
fn jev_bypass_is_only_one_or_true_and_stamps_shell() {
    assert!(policy::is_jev_bypass("1"));
    assert!(policy::is_jev_bypass("true"));
    for value in ["", "0", "false", "yes", "TRUE", " 1"] {
        assert!(!policy::is_jev_bypass(value), "{value}");
    }
    let _lock = cli::test_env_lock();
    set_env("JEV_BYPASS", Some("true"));
    set_env("JEV_MODE", Some("shadow"));
    set_env("TYPESAFE_API_KEY", None);
    let (code, bypass) = cli::run(&[
        "--tool-name".into(),
        "run_terminal_command".into(),
        "probe intent".into(),
    ]);
    assert_eq!(code, 0, "{bypass}");
    assert!(bypass.contains("\"bypass\":true"));
    assert!(bypass.contains("\"choice\":\"bypass\""));
    assert!(bypass.contains("\"blocked\":false"));
    assert!(bypass.contains("\"class\":\"shell\""));
    assert!(bypass.contains("\"policyId\":\"omapi-loop-stop-policy@1\""));
    assert!(bypass.contains("\"matched\":true"));
    set_env("JEV_BYPASS", Some("yes"));
    let (code, not_bypass) = cli::run(&["probe intent".into()]);
    assert_eq!(code, 0, "{not_bypass}");
    assert!(not_bypass.contains("\"bypass\":false"));
    assert!(not_bypass.contains("\"missingKey\":true"));
    assert!(!not_bypass.contains("\"pretool\""));
    set_env("JEV_BYPASS", None);
    let (code, write) = cli::run(&["--tool-name".into(), "search_replace".into(), "probe intent".into()]);
    assert_eq!(code, 0, "{write}");
    assert!(write.contains("\"class\":\"write\""));
    assert!(write.contains("\"policyId\":\"omapi-check-policy@1\""));
    assert!(write.contains("\"bypass\":false"));
    set_env("JEV_BYPASS", Some("1"));
    let (code, mcp) = cli::run(&["--tool-name".into(), "linear__save_issue".into(), "probe intent".into()]);
    assert_eq!(code, 0, "{mcp}");
    assert!(mcp.contains("\"bypass\":true"));
    assert!(mcp.contains("\"class\":\"mcp\""));
    assert!(mcp.contains("\"policyId\":\"omapi-route-workflow-policy@1\""));
    let skip = root("../../packages/jev-router/testdata/skip-marker.diff");
    let (code, checked) = cli::run(&[
        "--check".into(),
        "--intent".into(),
        "probe".into(),
        "--diff-file".into(),
        skip.to_str().unwrap().into(),
    ]);
    assert_eq!(code, 0, "{checked}");
    assert!(checked.contains("\"workflow\":\"check\""));
    assert!(checked.contains("\"approval\":false"));
    assert!(!checked.contains("\"choice\":\"bypass\""));
    assert!(checked.contains("\"kind\":\"tiny\""));
    set_env("JEV_BYPASS", None);
    set_env("JEV_MODE", None);
}

#[test]
fn pretool_map_matcher_catalog_and_schema_dump() {
    let ids = [
        match_pretool_class("web_search").unwrap().1,
        match_pretool_class("Bash").unwrap().1,
        match_pretool_class("search_replace").unwrap().1,
        match_pretool_class("linear__save_issue").unwrap().1,
    ];
    assert!(ids.contains(&LOOP_STOP_POLICY_ID));
    assert!(ids.contains(&CHECK_POLICY_ID));
    assert!(ids.contains(&ROUTING_POLICY_ID));
    assert_eq!(policy::choice_family("shell"), &["continue", "stop", "escalate"]);
    assert_eq!(
        policy::choice_family("write"),
        &["none", "test_safety", "task_mismatch", "cannot_tell"]
    );
    assert_eq!(policy::choice_family("mcp"), &["check", "review", "cannot_tell"]);
    assert!(policy::PRETOOL_MATCHER.contains("Bash"));
    assert!(policy::PRETOOL_MATCHER.contains("search_replace"));
    assert!(policy::PRETOOL_MATCHER.contains("__"));
    let stamp = pretool_stamp("search_replace").unwrap();
    assert!(stamp.matched);
    assert_eq!(stamp.class_id, Some("write"));
    assert!(pretool_stamp("").is_none());
    assert!(!pretool_stamp("read_file").unwrap().matched);
    let catalog = tiny_catalog_json();
    assert!(catalog.contains("\"kind\":\"tiny\""));
    assert!(catalog.len() < 900);
    assert!(!catalog.contains("$schema"));
    assert!(!catalog.contains("properties"));
    let bash = schema_dump_json("Bash");
    assert!(bash.contains("\"decision\":\"defer\""));
    assert!(bash.contains("\"autoAllow\":false"));
    assert!(bash.contains("\"policyId\":\"omapi-loop-stop-policy@1\""));
    let denied = schema_dump_json("read_file");
    assert!(denied.contains("\"decision\":\"deny\""));
    assert!(denied.contains("\"schema\":null"));
    let (code, missing) = cli::run(&["--schema".into()]);
    assert_eq!(code, 2);
    assert!(missing.contains("\"decision\":\"deny\""));
}

#[test]
fn router_source_policy_map_wrap_and_one_json_line() {
    let doc = read_rel("../../docs/POLICY-MAP.md");
    let fence = doc.split("```pretool-matcher\n").nth(1).unwrap().split("\n```").next().unwrap();
    assert_eq!(fence, policy::PRETOOL_MATCHER);
    assert!(doc.contains("JEV_BYPASS"));
    assert!(doc.contains("allow→continue"));
    assert!(policy::is_jev_bypass("1"));
    let wrap = read_rel("../../packages/cursor-agent-jev.nix");
    assert!(!wrap.contains("JEV_PERMISSION_MODE"));
    assert!(wrap.contains("--schema"));
    assert!(wrap.contains("--catalog"));
    assert!(wrap.contains("stop/escalate do not exec"));
    let baked_space = ["--set", "TYPESAFE_API_KEY"].join(" ");
    let baked_eq = ["--set=", "TYPESAFE_API_KEY"].join("");
    assert!(!wrap.contains(&baked_space) && !wrap.contains(&baked_eq));
    let shim = read_rel("../../packages/bezel-planes-shim.sh");
    assert!(!shim.contains("PLANES filter on"));
    let _lock = cli::test_env_lock();
    set_env("JEV_MODE", Some("shadow"));
    set_env("JEV_BYPASS", None);
    set_env("TYPESAFE_API_KEY", None);
    let (code, out) = cli::run(&["probe intent".into()]);
    assert_eq!(code, 0, "{out}");
    assert!(out.ends_with('\n'));
    assert!(!out.trim_end_matches('\n').contains('\n'));
    assert!(!out.contains("[jev-router]"));
    assert!(out.contains("\"missingKey\":true"));
    assert!(out.contains("\"blocked\":false"));
    assert!(out.contains("\"kind\":\"tiny\""));
    let (code, catalog) = cli::run(&["--catalog".into()]);
    assert_eq!(code, 0);
    assert_eq!(catalog.matches('\n').count(), 1);
    assert!(catalog.contains("\"kind\":\"tiny\""));
    set_env("JEV_MODE", None);
}

#[test]
fn permission_labels_key_absent_shell_and_write() {
    assert_eq!(permission::map_permission_label("allow", "loop-stop"), Some("continue"));
    assert_eq!(permission::map_permission_label("deny", "loop-stop"), Some("stop"));
    assert_eq!(permission::map_permission_label("ask", "loop-stop"), Some("escalate"));
    assert_eq!(permission::map_permission_label("ask", "writer"), Some("writer"));
    assert_eq!(permission::resolve_permission_mode("yes"), "shadow");
    assert_eq!(permission::resolve_permission_mode("true"), "shadow");
    assert_eq!(permission::resolve_permission_mode("ACTIVE"), "active");
    assert!(permission::permission_surface("web").is_none());
    assert_eq!(permission::permission_surface("shell").unwrap().0, "loop-stop");
    assert_eq!(permission::permission_surface("write").unwrap().0, "writer");
    assert_eq!(permission::permission_surface("mcp").unwrap().1, "route-workflow");
    assert!(decide_permission(&permit(
        "web", "active", true, false, Some("allow"), 0.0, 0.0, 0.0, 0.0, "", "", None, "", "", None, None,
    ))
    .is_none());

    let shell = decide_permission(&permit(
        "shell", "active", false, true, Some("deny"), 0.99, 0.01, 0.98, 0.01, "", "", None, "Bash", "", None, None,
    ))
    .unwrap();
    assert_eq!(shell.mode, "shadow");
    assert!(!shell.honor);
    assert!(!shell.blocked);
    assert!(shell.json.contains("\"missingKey\":true"));
    assert!(shell.json.contains("\"reason\":\"missing_key\""));
    assert!(shell.json.contains("\"choice\":\"unclassified\""));
    assert!(!shell.auto_allow);

    let write = decide_permission(&permit(
        "write", "active", false, false, Some("allow"), 0.0, 0.0, 0.0, 0.0, ".env", "", None, "Write", "", None, None,
    ))
    .unwrap();
    assert_eq!(write.mode, "shadow");
    assert!(!write.blocked);
    assert!(write.json.contains("\"codeDeny\":true"));
    assert!(write.json.contains("\"mapped\":\"stop\""));
    assert!(write.json.contains("\"missingKey\":true"));

    let no_content = decide_permission(&permit(
        "shell", "active", true, false, Some("allow"), 0.9, 0.8, 0.1, 0.1, "", "", None, "Bash", "", None, None,
    ))
    .unwrap();
    assert!(no_content.json.contains("\"reason\":\"no_content\""));
    assert_ne!(no_content.choice, "continue");

    let confident = decide_permission(&permit(
        "shell", "active", true, true, Some("allow"), 0.9, 0.8, 0.1, 0.1, "", "", None, "Bash", "", None, None,
    ))
    .unwrap();
    assert_eq!(confident.choice, "continue");
    assert!(!confident.auto_allow);

    let empty = decide_permission(&permit(
        "write", "active", true, false, Some("allow"), 0.9, 0.0, 0.0, 0.0, "src/app.js", "", None, "Write", "", None, None,
    ))
    .unwrap();
    assert!(empty.json.contains("\"reason\":\"empty_findings_not_approval\""));
    assert_ne!(empty.choice, "continue");
    assert!(empty.json.contains("detail=empty_findings_not_approval"));
    assert!(empty.json.contains("question="));

    let below = decide_permission(&permit(
        "write",
        "active",
        true,
        false,
        Some("allow"),
        0.0,
        0.0,
        0.0,
        0.0,
        "src/app.js",
        "",
        Some(0.2),
        "Write",
        "",
        None,
        None,
    ))
    .unwrap();
    assert_eq!(below.choice, "continue");
    assert!(!below.json.contains("\"hold\""));
}

#[test]
fn permission_does_not_auto_allow_over_a_parent_stop() {
    let continued = decide_permission(&permit(
        "shell", "active", true, true, Some("allow"), 0.9, 0.8, 0.1, 0.1, "", "", None, "Bash", "", None, None,
    ))
    .unwrap();
    let merged = apply_permission_verdict(
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
    assert!(merged.contains("\"choice\":\"stop\""));
    assert!(merged.contains("\"blocked\":true"));
    assert!(merged.contains("\"mapped\":\"continue\""));
    assert!(merged.contains("\"autoAllow\":false"));
    let shadow_stop = decide_permission(&permit(
        "shell", "shadow", true, true, Some("deny"), 0.9, 0.1, 0.8, 0.1, "", "", None, "Bash", "", None, None,
    ))
    .unwrap();
    let logged = apply_permission_verdict(
        PermissionParent {
            choice: "continue",
            gate: "auto",
            blocked: false,
            exec: true,
            hitl: Some(false),
            mode: "shadow",
        },
        &shadow_stop,
    );
    assert!(logged.contains("\"choice\":\"continue\""));
    assert!(logged.contains("\"blocked\":false"));
    assert!(!shadow_stop.honor);
}

#[test]
fn pretool_hook_honors_permission_and_never_allows() {
    let mut stop = GateVerdict::simple("active", "continue", "auto", false);
    stop.permission = Some(SurfaceVerdict {
        honor: true,
        mode: "active".into(),
        choice: "stop".into(),
        gate: "hold".into(),
        blocked: true,
        policy_id: LOOP_STOP_POLICY_ID.into(),
    });
    let denied = pretool_hook_decision(&stop);
    assert_eq!(denied.decision, "deny");
    assert_ne!(denied.decision, "allow");
    assert_eq!(denied.exit_code, 2);
    assert!(denied.reason.contains("choice=stop"));
    assert!(denied.reason.contains("policy=omapi-loop-stop-policy@1"));
    let deferred = pretool_hook_decision(&GateVerdict::simple("active", "continue", "auto", false));
    assert_eq!(deferred.decision, "defer");
    assert_eq!(deferred.reason, "exec");
    let shadow = pretool_hook_decision(&GateVerdict::simple("shadow", "stop", "hold", true));
    assert_eq!(shadow.decision, "defer");
    assert_eq!(shadow.reason, "shadow");
}

#[test]
fn shadow_permission_spawn_stays_unblocked() {
    let _lock = cli::test_env_lock();
    set_env("JEV_MODE", Some("shadow"));
    set_env("JEV_PERMISSION_MODE", Some("active"));
    set_env("JEV_BYPASS", None);
    set_env("TYPESAFE_API_KEY", None);
    let (code, shell) = cli::run(&[
        "--tool-name".into(),
        "Bash".into(),
        "--tool-input".into(),
        "npm test".into(),
        "probe intent".into(),
    ]);
    assert_eq!(code, 0, "{shell}");
    assert!(shell.contains("\"missingKey\":true"));
    assert!(shell.contains("\"blocked\":false"));
    assert!(shell.contains("\"honor\":false"));
    assert!(shell.contains("\"reason\":\"missing_key\""));
    assert!(shell.contains("\"autoAllow\":false"));
    assert!(shell.contains("\"policyId\":\"omapi-loop-stop-policy@1\""));
    let (code, write) = cli::run(&[
        "--tool-name".into(),
        "Write".into(),
        "--tool-input".into(),
        ".env".into(),
        "probe intent".into(),
    ]);
    assert_eq!(code, 0, "{write}");
    assert!(write.contains("\"blocked\":false"));
    assert!(write.contains("\"codeDeny\":true"));
    assert!(write.contains("\"mapped\":\"stop\""));
    assert!(write.contains("\"choice\":\"unclassified\""));
    assert!(write.contains("\"policyId\":\"omapi-check-policy@1\""));
    set_env("JEV_PERMISSION_MODE", None);
    set_env("JEV_MODE", None);
}

#[test]
fn typed_calls_rank_guard_ask_and_missing_key() {
    let ranked = calls::rank_names(
        &[
            ("linear__list_issues", 0.8),
            ("linear__save_issue", 0.12),
            ("notes__add", 0.05),
        ],
        2,
    );
    assert_eq!(ranked, ["linear__list_issues", "linear__save_issue"]);
    assert!(calls::stated_keeps_optional_arg(0.92));
    let team = policy::apply_routing_thresholds("eng", 0.88, &[("eng", 0.84), ("ops", 0.16)], &["eng", "ops"]);
    assert_eq!(team.outcome, "eng");
    assert_eq!(team.reason, "selected");
    let call = tool_call();
    assert_eq!(call.transport, "facet");
    assert!(!call.initiated);
    assert_eq!(call.op, "tool_call");
    for effect in ["read", "write", "filesystem", "network", "external"] {
        let guard = calls::guard_effect(effect);
        assert!(guard.allow, "{effect}");
        assert!(guard.code.is_none(), "{effect}");
        assert!(!obvious_effect_json(effect, "active").contains("F454"));
    }
    let payment = calls::guard_effect("payment");
    assert!(!payment.allow);
    assert_eq!(payment.code, Some("F454"));
    assert_eq!(calls::guard_effect("").code, Some("F456"));
    let body = hard_stop_json(
        "pay__now",
        &[
            HardStopTool {
                name: "pay__now",
                description: "Pay",
                effect: "payment",
            },
            HardStopTool {
                name: "linear__list_issues",
                description: "List issues",
                effect: "read",
            },
        ],
        &[("pay__now", 0.8), ("linear__list_issues", 0.2)],
    );
    assert!(body.contains("\"best\":null"));
    assert!(body.contains("\"autoPromote\":false"));
    assert!(body.contains("\"mapped\":\"stop\""));
    let ask = model_uncertain_json();
    assert!(ask.contains("\"reason\":\"model_uncertain\""));
    assert!(ask.contains("\"mapped\":\"ask\""));
    assert!(ask.contains("confidence 0.6"));
    assert!(ask.contains("probability 0.55"));
    assert!(ask.contains("margin 0.15"));
    assert!(!ask.contains("F454"));
    assert!(ask.contains("\"best\":null"));
    let human = cannot_tell_json();
    assert!(human.contains("\"mapped\":\"escalate\""));
    assert!(human.contains("detail="));
    assert!(human.contains("question="));
    let missing = missing_key_envelope();
    assert!(missing.contains("\"mode\":\"shadow\""));
    assert!(missing.contains("\"honor\":false"));
    assert!(missing.contains("\"missingKey\":true"));
    assert!(missing.contains("\"blocked\":false"));
    assert!(missing.contains("\"best\":null"));
    assert!(missing.contains("\"initiated\":false"));
    assert!(missing.contains("\"label\":\"ask\""));
    assert!(!missing.contains("\"mapped\":\"continue\""));
    let secret = "supersecretvalue";
    let scrubbed = facts::scrub_secret_text(&format!("List {secret}"), &[secret]);
    assert!(!scrubbed.contains(secret));
    assert!(scrubbed.contains("[redacted]"));
    assert!(facts::secret_object_key("TYPESAFE_API_KEY"));
    assert!(facts::secret_object_key("api_key"));
    assert!(!facts::secret_object_key("description"));
}

#[test]
fn typed_call_does_not_auto_allow_and_mcp_read_follows_loop_stop() {
    let read = decide_permission(&permit(
        "mcp",
        "active",
        true,
        true,
        Some("allow"),
        0.9,
        0.8,
        0.1,
        0.1,
        "",
        "",
        None,
        "lapis__search",
        "read",
        Some("check"),
        None,
    ))
    .unwrap();
    assert_eq!(read.choice, "continue");
    assert!(read.json.contains("\"reason\":\"selected\""));
    assert!(!read.json.contains("workflow_is_not_permission"));
    assert!(!read.auto_allow);
    let dropped = decide_permission(&permit(
        "mcp",
        "active",
        true,
        true,
        Some("allow"),
        0.9,
        0.8,
        0.1,
        0.1,
        "",
        "",
        None,
        "",
        "",
        Some("check"),
        None,
    ))
    .unwrap();
    assert!(dropped.json.contains("\"reason\":\"workflow_is_not_permission\""));
    assert_ne!(dropped.choice, "continue");
    let hook = pretool_hook_decision(&GateVerdict {
        permission: Some(SurfaceVerdict {
            honor: true,
            mode: "active".into(),
            choice: read.choice.into(),
            gate: read.gate.into(),
            blocked: read.blocked,
            policy_id: ROUTING_POLICY_ID.into(),
        }),
        ..GateVerdict::simple("active", "continue", "auto", false)
    });
    assert_eq!(hook.decision, "defer");
    assert_ne!(hook.decision, "allow");
    let kept = apply_permission_verdict(
        PermissionParent {
            choice: "stop",
            gate: "hold",
            blocked: true,
            exec: false,
            hitl: Some(false),
            mode: "active",
        },
        &read,
    );
    assert!(kept.contains("\"choice\":\"stop\""));
    assert!(kept.contains("\"blocked\":true"));
    assert!(kept.contains("\"autoAllow\":false"));
}

#[test]
fn typed_call_cli_stays_shadow_without_a_key() {
    let _lock = cli::test_env_lock();
    set_env("JEV_MODE", Some("shadow"));
    set_env("JEV_BYPASS", None);
    set_env("TYPESAFE_API_KEY", None);
    set_env("JEV_PERMISSION_MODE", Some("active"));
    let path = std::env::temp_dir().join(format!("oracle-calls-{}.json", std::process::id()));
    let secret = "supersecretvalue";
    std::fs::write(
        &path,
        format!(
            r#"{{"interface":"Mcp","TYPESAFE_API_KEY":"{secret}","tools":[{{"name":"linear__list_issues","description":"List issues","effect":"read","args":[]}}]}}"#
        ),
    )
    .unwrap();
    let (code, out) = cli::run(&[
        "--calls".into(),
        "--tools-file".into(),
        path.to_str().unwrap().into(),
        "--tool-name".into(),
        "linear__list_issues".into(),
        "--top".into(),
        "2".into(),
        "list the issues".into(),
    ]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains(secret));
    assert!(out.contains("\"blocked\":false"));
    assert!(out.contains("\"class\":\"mcp\""));
    assert!(out.contains("\"transport\":\"facet\""));
    assert!(out.contains("\"mode\":\"shadow\""));
    assert!(out.contains("\"honor\":false"));
    assert!(out.contains("\"missingKey\":true"));
    assert!(out.contains("\"initiated\":false"));
    assert!(out.contains("\"best\":null"));
    assert!(out.contains("\"reason\":\"missing_key\""));
    assert!(out.contains("\"op\":\"tool_expose\""));
    assert!(out.contains("\"policyId\":\"omapi-route-workflow-policy@1\""));
    assert!(!out.contains("jsonrpc"));
    assert!(!out.contains("tools/call"));
    set_env("JEV_PERMISSION_MODE", None);
    set_env("JEV_MODE", None);
}

#[test]
fn harness_uses_the_map_and_smoke_pieces_never_allow() {
    let modes = modes_from_env(&[
        ("JEV_MODE", "active"),
        ("JEV_PERMISSION_MODE", "yes"),
        ("JEV_TYPED_CALL_MODE", "true"),
    ]);
    assert_eq!(modes.jev_mode, "active");
    assert!(!modes.bypass);
    assert_eq!(permission::resolve_permission_mode("yes"), "shadow");
    assert_eq!(permission::resolve_permission_mode("true"), "shadow");
    let event = parse_hook_event(r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#).unwrap();
    assert_eq!(event.tool_name, "Bash");
    assert_eq!(event.command, "npm test");
    let stamp = pretool_stamp("Bash").unwrap();
    assert!(stamp.matched);
    assert_eq!(stamp.class_id, Some("shell"));
    assert_eq!(stamp.policy_id, Some(LOOP_STOP_POLICY_ID));
    let catalog = tiny_catalog_json();
    assert!(catalog.contains("\"kind\":\"tiny\""));
    assert!(catalog.len() < 900);
    assert!(!catalog.contains("$schema"));
    let dumped = schema_dump_json("Bash");
    assert!(dumped.contains("\"decision\":\"defer\""));
    assert!(dumped.contains("\"autoAllow\":false"));
    let call = tool_call();
    assert_eq!(call.transport, "facet");
    assert_eq!(call.op, "tool_call");
    assert!(!call.initiated);
    let (code, line) = hook_command(
        r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#,
        &[("JEV_MODE", "active")],
    );
    assert_eq!(code, 2, "{line}");
    assert!(line.contains("\"decision\":\"deny\""));
    assert!(line.contains("policy=omapi-loop-stop-policy@1"));
    assert!(!line.contains("\"decision\":\"allow\""));
    let (code, bypass) = hook_command(
        r#"{"tool_name":"Bash"}"#,
        &[("JEV_MODE", "active"), ("JEV_BYPASS", "1")],
    );
    assert_eq!(code, 0);
    assert!(bypass.contains("\"reason\":\"bypass\""));
    let (code, yes) = hook_command(
        r#"{"tool_name":"Bash"}"#,
        &[("JEV_MODE", "active"), ("JEV_BYPASS", "yes")],
    );
    assert_eq!(code, 2);
    assert!(yes.contains("\"decision\":\"deny\""));
    assert_ne!(hook_decision("continue", "auto"), "allow");
    assert!(!AUTO_ALLOW);
}

#[test]
fn smoke_and_config_match_the_harness_check() {
    let report = smoke_report();
    let catalog = tiny_catalog_json();
    assert!(catalog.len() < 900, "catalog bytes {}", catalog.len());
    for needle in [
        r#""ok":true"#,
        r#""pretool":{"toolName":"Bash","class":"shell","policyId":"omapi-loop-stop-policy@1","matched":true}"#,
        r#""classes":[{"toolName":"Bash","class":"shell","policyId":"omapi-loop-stop-policy@1"},{"toolName":"search_replace","class":"write","policyId":"omapi-check-policy@1"},{"toolName":"linear__list_issues","class":"mcp","policyId":"omapi-route-workflow-policy@1"}]"#,
        r#""kind":"tiny""#,
        r#""schemaInline":false"#,
        r#""schemaDump":{"tool":"linear__list_issues","decision":"defer","typedCall":false,"autoAllow":false}"#,
        r#""calls":{"transport":"facet","op":"tool_call","initiated":false,"policyId":"omapi-route-workflow-policy@1","autoAllow":false}"#,
        r#""shadow":{"decision":"defer","honor":false,"blocked":false}"#,
        r#""active":{"decision":"deny","honor":true,"blocked":true}"#,
        r#""uncertain":{"decision":"deny"}"#,
        r#""emptyFindings":{"reason":"empty_findings_not_approval","decision":"deny","approval":false}"#,
        r#""keyAbsent":{"mode":"shadow","honor":false,"blocked":false}"#,
        r#""bypass":{"decision":"defer","reason":"bypass"}"#,
        r#""notBypass":{"decision":"deny"}"#,
        r#""irreversible":{"decision":"defer","code":null,"initiated":false,"autoPromote":false}"#,
        r#""uncertainActive":{"decision":"deny"}"#,
        r#""uncertainShadow":{"decision":"defer"}"#,
        r#""readStaysDefer":{"decision":"defer"}"#,
        r#""loopStopWins":{"decision":"deny"}"#,
        r#""unmapped":{"decision":"defer","reason":"unmatched"}"#,
    ] {
        assert!(report.contains(needle), "missing {needle} in {report}");
    }
    assert!(report.contains(&format!(r#""bytes":{}"#, catalog.len())), "{report}");
    assert!(!report.contains("\"decision\":\"allow\""));
    assert!(!report.contains("F454"));
    let (code, out) = cli::run(&["smoke".into()]);
    assert_eq!(code, 0);
    assert_eq!(out.trim(), report);
    let cfg = harness_config();
    assert!(cfg.contains("\"command\":\"grok-build-jev hook\""));
    assert!(!cfg.contains("TYPESAFE_API_KEY"));
    assert!(!cfg.contains("JEV_MODE"));
    assert!(!cfg.contains("JEV_PERMISSION_MODE"));
    assert!(!cfg.contains("JEV_TYPED_CALL_MODE"));
    let (code, printed) = cli::run(&["config".into()]);
    assert_eq!(code, 0);
    assert_eq!(printed.trim(), cfg);
}

#[test]
fn route_prompt_names_the_calling_harness() {
    let text = route_writer_prompt();
    assert!(text.contains("cursor_default"));
    assert!(text.contains("Default writer for the calling harness (host Cursor auth)"));
    assert!(!text.contains("Cursor/omp writer"));
}

#[test]
fn build_gate_state_strips_the_secret_and_keeps_the_tiny_catalog() {
    let secret = ["catalog", "test", "secret"].join("-");
    let state = build_gate_state(&GateInput {
        intent: &format!("ship {secret} please"),
        typesafe_api_key: Some(&secret),
        api_key: Some(&secret),
        schema: Some(r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object"}"#),
        loop_stop_policy_version: LOOP_STOP_POLICY_ID,
        secret: &secret,
    });
    assert!(!state.contains(&secret));
    assert!(state.contains("[redacted]"));
    assert!(state.contains("\"kind\":\"tiny\""));
    assert!(!state.contains("$schema"));
    assert!(!state.contains("TYPESAFE_API_KEY"));
    assert!(!state.contains("apiKey"));
    assert!(state.contains(&format!("\"version\":\"{LOOP_STOP_POLICY_ID}\"")));
}

#[test]
fn smoke_decision_log_is_one_temp_line() {
    let dir = std::env::temp_dir().join(format!("bezel-decision-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let log = smoke_decision_log(&dir).unwrap();
    assert_eq!(log.lines, 1);
    assert!(log.path.starts_with(&dir));
    assert!(!log.path.to_string_lossy().contains(".grok/logs/jev"));
    let body = std::fs::read_to_string(&log.path).unwrap();
    assert_eq!(body.trim().lines().count(), 1);
    assert!(body.contains("detail="));
    assert!(body.contains("question="));
    assert!(!body.contains("TYPESAFE_API_KEY"));
    let _ = std::fs::remove_dir_all(&dir);
}
