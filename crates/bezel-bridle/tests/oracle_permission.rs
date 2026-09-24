//! Oracle: permission. Every behavior `packages/jev-router/test.mjs` asserts
//! for `decidePermission`, `applyPermissionVerdict`, `permissionFromHook`,
//! surfaces, and the pretool hook, asserted against the shipped crate.

use bezel_bridle::{
    active_hook, apply_permission_verdict, decide_permission, hook_stdout, pretool_hook_decision,
    write_from_flags, write_flags, AUTO_ALLOW, CHECK_POLICY_ID,
    CONCERN_PARK, GateVerdict, HookOut, LOOP_STOP_MIN_CONFIDENCE, LOOP_STOP_MIN_MARGIN,
    LOOP_STOP_MIN_PROBABILITY, LOOP_STOP_POLICY_ID, PermissionInput, PermissionParent,
    PermissionVerdict, ROUTING_POLICY_ID, SurfaceVerdict, TypedPermission,
};
use std::path::PathBuf;

fn permission_golden(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/jev-router/testdata/permission")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

fn input<'a>(
    class_id: &'static str,
    requested_mode: &'static str,
    has_key: bool,
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
    content_present: bool,
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
        routing_outcome: None,
        typed: None,
    }
}

/// Same as `input` but sets `routing_outcome` for the workflow branches.
fn workflow_input<'a>(
    class_id: &'static str,
    requested_mode: &'static str,
    has_key: bool,
    label: Option<&'static str>,
    routing_outcome: &'a str,
) -> PermissionInput<'a> {
    PermissionInput {
        routing_outcome: Some(routing_outcome),
        ..input(class_id, requested_mode, has_key, label, 0.0, 0.0, 0.0, 0.0, "", "", None, "", "", false)
    }
}

fn assert_never_allow(verdict: &PermissionVerdict, name: &str) {
    assert!(!verdict.auto_allow, "{name}");
    assert!(!verdict.json.contains("\"decision\":\"allow\""), "{name}");
    assert!(!verdict.json.contains("F454"), "{name}");
    assert_eq!(AUTO_ALLOW, false);
}

fn surface_of(verdict: &PermissionVerdict) -> SurfaceVerdict {
    SurfaceVerdict {
        honor: verdict.honor,
        mode: verdict.mode.to_string(),
        choice: verdict.choice.to_string(),
        gate: verdict.gate.to_string(),
        blocked: verdict.blocked,
        policy_id: verdict.policy_id.to_string(),
    }
}

fn hook_of(
    mode: &str,
    choice: &str,
    gate: &str,
    blocked: bool,
    verdict: &PermissionVerdict,
) -> HookOut {
    pretool_hook_decision(&GateVerdict {
        bypass: false,
        mode: mode.to_string(),
        choice: Some(choice.to_string()),
        gate: Some(gate.to_string()),
        blocked,
        policy_id: String::new(),
        permission: Some(surface_of(verdict)),
        calls: None,
    })
}

#[test]
fn key_absent_forces_shadow_even_when_active_was_requested() {
    let shell = decide_permission(&input(
        "shell", "active", false, Some("deny"), 0.99, 0.01, 0.98, 0.01, "", "", None, "", "", true,
    ))
    .unwrap();
    assert_eq!(shell.mode, "shadow");
    assert!(!shell.honor);
    assert!(!shell.blocked);
    assert_eq!(shell.choice, "unclassified");
    assert_eq!(shell.policy_id, LOOP_STOP_POLICY_ID);
    assert!(!shell.auto_allow);
    assert_eq!(shell.json, permission_golden("shell-missing-key.json"));

    let write = decide_permission(&input(
        "write", "active", false, Some("allow"), 0.0, 0.0, 0.0, 0.0, ".env", "", None, "", "", false,
    ))
    .unwrap();
    assert_eq!(write.mode, "shadow");
    assert!(!write.blocked);
    assert!(!write.honor);
    assert!(write.json.contains("\"codeDeny\":true"));
    assert!(write.json.contains("\"mapped\":\"stop\""));
    assert!(write.json.contains("\"reason\":\"secret_path\""));
    assert_eq!(write.policy_id, CHECK_POLICY_ID);
    assert_eq!(write.choice, "unclassified");
    assert_never_allow(&write, "write-missing-key");
    assert_eq!(write.json, permission_golden("write-missing-key-deny.json"));
}

#[test]
fn unknown_classes_are_null_like_permission_mjs() {
    assert!(decide_permission(&input(
        "read_file", "active", true, None, 0.0, 0.0, 0.0, 0.0, "", "", None, "", "", false,
    ))
    .is_none());
    assert!(decide_permission(&input(
        "web", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, "", "", None, "", "", false,
    ))
    .is_none());
    assert_eq!(permission_golden("unknown-read-file.json"), "null");
    assert_eq!(permission_golden("unknown-web.json"), "null");
}

#[test]
fn shadow_shell_logs_but_never_honors() {
    let allowed = decide_permission(&input(
        "shell", "shadow", true, Some("allow"), 0.9, 0.8, 0.1, 0.1, "", "", None, "", "", true,
    ))
    .unwrap();
    assert_eq!(allowed.mode, "shadow");
    assert!(!allowed.honor);
    assert!(!allowed.blocked);
    assert_eq!(allowed.choice, "unclassified");
    assert!(allowed.json.contains("\"mapped\":\"continue\""));
    assert!(allowed.json.contains("\"label\":\"allow\""));
    assert_eq!(allowed.policy_id, LOOP_STOP_POLICY_ID);
    assert!(!allowed.auto_allow);
    assert_eq!(allowed.json, permission_golden("shell-shadow-default.json"));

    // Active honors: choice continue, gate auto, exec allowed.
    let honored = decide_permission(&input(
        "shell", "active", true, Some("allow"), 0.9, 0.8, 0.1, 0.1, "", "", None, "", "", true,
    ))
    .unwrap();
    assert!(honored.honor);
    assert_eq!(honored.choice, "continue");
    assert_eq!(honored.gate, "auto");
    assert!(!honored.blocked);
    assert_eq!(honored.json, permission_golden("shell-active-continue.json"));
    assert_never_allow(&honored, "shell-active-continue");
}

#[test]
fn uncertain_and_ask_escalate_and_never_continue() {
    let uncertain = decide_permission(&input(
        "shell", "active", true, Some("allow"), 0.4, 0.4, 0.35, 0.25, "", "", None, "", "", true,
    ))
    .unwrap();
    assert_eq!(uncertain.choice, "escalate");
    assert!(uncertain.blocked);
    assert!(uncertain.json.contains("\"modelLabel\":\"allow\""));
    assert!(uncertain.json.contains("\"reason\":\"model_uncertain\""));
    assert_eq!(uncertain.json, permission_golden("shell-under-bars.json"));
    assert_never_allow(&uncertain, "shell-under-bars");

    let ask = decide_permission(&input(
        "shell", "active", true, Some("ask"), 0.2, 0.2, 0.2, 0.6, "", "", None, "", "", true,
    ))
    .unwrap();
    assert_eq!(ask.choice, "escalate");
    assert!(ask.hitl);
    assert_eq!(ask.json, permission_golden("shell-ask-under-bars.json"));
    assert_never_allow(&ask, "shell-ask-under-bars");

    let abstain = decide_permission(&input(
        "shell", "active", true, Some("cannot_tell"), 0.9, 0.1, 0.0, 0.0, "", "", None, "", "", true,
    ))
    .unwrap();
    assert_eq!(abstain.choice, "escalate");
    assert!(abstain.json.contains("\"reason\":\"cannot_tell\""));
    assert_eq!(abstain.json, permission_golden("shell-cannot-tell.json"));
}

#[test]
fn shell_without_content_asks_instead_of_allowing() {
    let no_content = decide_permission(&input(
        "shell", "active", true, Some("allow"), 0.9, 0.8, 0.1, 0.1, "", "", None, "", "", false,
    ))
    .unwrap();
    assert!(no_content.json.contains("\"label\":\"ask\""));
    assert!(no_content.json.contains("\"mapped\":\"escalate\""));
    assert!(no_content.json.contains("\"reason\":\"no_content\""));
    assert_eq!(no_content.choice, "escalate");
    assert_eq!(no_content.json, permission_golden("shell-no-content.json"));
    let hook = hook_of("active", "continue", "auto", false, &no_content);
    assert_eq!(hook.decision, "deny");
    assert_eq!(
        format!(
            r#"{{"action":"deny","exitCode":2,"decision":"deny","reason":"{}"}}"#,
            hook.reason
        ),
        permission_golden("shell-no-content-hook.json")
    );
}

#[test]
fn write_deny_flags_stop_over_an_allow_label() {
    let skip = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/jev-router/testdata/skip-marker.diff");
    let skip_diff = std::fs::read_to_string(&skip).unwrap();
    let assertions = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/jev-router/testdata/assertions-removed.diff");
    let assertions_diff = std::fs::read_to_string(&assertions).unwrap();
    let docs = "diff --git a/notes/a.md b/notes/a.md\n--- a/notes/a.md\n+++ b/notes/a.md\n@@\n-old\n+new paragraph";
    let deleted = "diff --git a/src/math.test.js b/src/math.test.js\ndeleted file mode 100644\nindex 1111111..0000000\n--- a/src/math.test.js\n+++ /dev/null\n@@ -1 +0,0 @@\n-const value = 1;";

    let secret = decide_permission(&input(
        "write", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, ".env", docs, Some(0.2), "", "", false,
    ))
    .unwrap();
    assert!(secret.json.contains("\"reason\":\"secret_path\""));
    assert!(secret.json.contains("\"codeDeny\":true"));
    assert!(secret.json.contains("\"modelLabel\":\"allow\""));
    assert_eq!(secret.choice, "stop");
    assert_eq!(secret.json, permission_golden("write-secret-path.json"));
    assert_never_allow(&secret, "write-secret-path");

    let skip_in = decide_permission(&input(
        "write", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, "", &skip_diff, Some(0.2), "", "", false,
    ))
    .unwrap();
    assert_eq!(skip_in.json, permission_golden("write-skip-marker.json"));
    assert!(skip_in.json.contains("\"reason\":\"skip_marker_added\""));

    let assert_in = decide_permission(&input(
        "write", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, "", &assertions_diff, Some(0.2), "", "", false,
    ))
    .unwrap();
    assert_eq!(assert_in.json, permission_golden("write-assertions-removed.json"));

    let deleted_in = decide_permission(&input(
        "write", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, "", deleted, Some(0.2), "", "", false,
    ))
    .unwrap();
    assert_eq!(deleted_in.json, permission_golden("write-test-file-deleted.json"));
    assert!(deleted_in.json.contains("\"reason\":\"test_file_deleted\""));
}

#[test]
fn write_concern_below_park_continues_and_absent_concern_does_not() {
    let docs = "diff --git a/notes/a.md b/notes/a.md\n--- a/notes/a.md\n+++ b/notes/a.md\n@@\n-old\n+new paragraph";
    assert_eq!(CONCERN_PARK, 0.4);
    assert_eq!(LOOP_STOP_MIN_CONFIDENCE, 0.6);
    assert_eq!(LOOP_STOP_MIN_PROBABILITY, 0.55);
    assert_eq!(LOOP_STOP_MIN_MARGIN, 0.15);

    let continued = decide_permission(&input(
        "write", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, "notes/a.md", docs, Some(0.2), "", "", false,
    ))
    .unwrap();
    assert_eq!(continued.choice, "continue");
    assert_eq!(continued.gate, "auto");
    assert!(!continued.blocked);
    assert!(continued.json.contains("\"codeDeny\":false"));
    assert_eq!(continued.json, permission_golden("write-below-park.json"));
    assert_never_allow(&continued, "write-below-park");

    let zero = decide_permission(&input(
        "write", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, "notes/a.md", docs, Some(0.0), "", "", false,
    ))
    .unwrap();
    assert_eq!(zero.choice, "continue");
    assert_eq!(zero.json, permission_golden("write-below-park-zero.json"));

    // A continue must not clear a parent stop. lib.rs builds this from a
    // label-free input, so modelLabel is null in the golden.
    let below_plain = decide_permission(&input(
        "write", "active", true, None, 0.0, 0.0, 0.0, 0.0, "notes/a.md", docs, Some(0.2), "", "", false,
    ))
    .unwrap();
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
    assert_eq!(
        format!(
            r#"{{"action":"deny","exitCode":2,"decision":"deny","reason":"{}"}}"#,
            kept_hook.reason
        ),
        permission_golden("write-below-park-keeps-stop-hook.json")
    );

    // Absent, NaN, and at-bar concern never continue; label is ask.
    let absent = decide_permission(&input(
        "write", "active", true, None, 0.0, 0.0, 0.0, 0.0, "notes/a.md", docs, None, "", "", false,
    ))
    .unwrap();
    assert_ne!(absent.choice, "continue");
    assert!(absent.json.contains("\"label\":\"ask\""));
    assert!(absent.json.contains("\"reason\":\"empty_findings_not_approval\""));
    let nan = decide_permission(&input(
        "write", "active", true, None, 0.0, 0.0, 0.0, 0.0, "notes/a.md", docs, Some(f64::NAN), "", "", false,
    ))
    .unwrap();
    assert_ne!(nan.choice, "continue");
    let at_bar = decide_permission(&input(
        "write", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, "notes/a.md", docs, Some(CONCERN_PARK), "", "", false,
    ))
    .unwrap();
    assert_ne!(at_bar.choice, "continue");
    assert_eq!(at_bar.json, permission_golden("write-at-park.json"));
}

#[test]
fn write_empty_findings_are_writer_parent_not_approval() {
    let empty = decide_permission(&input(
        "write", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, "src/app.js", "", None, "", "", false,
    ))
    .unwrap();
    assert!(empty.json.contains("\"reason\":\"empty_findings_not_approval\""));
    assert!(empty.json.contains("\"mapped\":\"writer\""));
    assert_eq!(empty.choice, "escalate");
    assert_ne!(empty.choice, "continue");
    assert!(!empty.json.contains("\"codeDeny\":true"));
    assert!(!empty.auto_allow);
    assert_eq!(empty.json, permission_golden("write-empty.json"));
    let hook = hook_of("active", "continue", "auto", false, &empty);
    assert_eq!(hook.decision, "deny");
    assert_eq!(
        format!(
            r#"{{"action":"deny","exitCode":2,"decision":"deny","reason":"{}"}}"#,
            hook.reason
        ),
        permission_golden("write-empty-hook.json")
    );

    let shadow_empty = decide_permission(&input(
        "write", "shadow", true, None, 0.0, 0.0, 0.0, 0.0, "src/app.js", "", None, "", "", false,
    ))
    .unwrap();
    assert!(!shadow_empty.honor);
    assert!(!shadow_empty.blocked);
    assert_eq!(shadow_empty.choice, "unclassified");
    assert!(shadow_empty.json.contains("\"mapped\":\"writer\""));
    assert_eq!(shadow_empty.json, permission_golden("write-shadow-empty.json"));
}

#[test]
fn write_from_flags_matches_collect_write_flags_behavior() {
    let skip = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/jev-router/testdata/skip-marker.diff");
    let flags = write_flags(&std::fs::read_to_string(&skip).unwrap(), "src/math.test.js");
    assert!(flags.contains(&"skip_marker_added".to_string()));

    // Flags collected from the diff stop an active write even under label=allow.
    let flag_refs: Vec<&str> = flags.iter().map(|flag| flag.as_str()).collect();
    let flagged = decide_permission(&PermissionInput {
        flags: Some(&flag_refs),
        ..input(
            "write", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, "", "", None, "", "", false,
        )
    })
    .unwrap();
    assert_eq!(flagged.choice, "stop");
    assert!(flagged.json.contains("\"codeDeny\":true"));
    assert_never_allow(&flagged, "write-flagged");

    let verdict = write_from_flags(&flags, Some(0.2));
    assert_eq!(verdict.mapped, "stop");
    assert_eq!(verdict.gate, "hold");
    assert_eq!(verdict.reason, "skip_marker_added");
    assert!(verdict.code_deny);
    assert!(!verdict.auto_allow);
    assert_eq!(active_hook(&verdict), "deny");
    assert_ne!(active_hook(&verdict), "allow");
}

#[test]
fn mcp_workflow_is_not_permission_and_mutating_mcp_escalates() {
    let workflow = decide_permission(&workflow_input(
        "mcp", "active", true, Some("allow"), "check",
    ))
    .unwrap();
    assert!(workflow.json.contains("\"reason\":\"workflow_is_not_permission\""));
    assert_eq!(workflow.choice, "escalate");
    assert!(workflow.blocked);
    assert!(!workflow.auto_allow);
    assert_eq!(workflow.json, permission_golden("mcp-workflow.json"));
    let hook = hook_of("active", "continue", "auto", false, &workflow);
    assert_eq!(hook.decision, "deny");
    assert_eq!(
        format!(
            r#"{{"action":"deny","exitCode":2,"decision":"deny","reason":"{}"}}"#,
            hook.reason
        ),
        permission_golden("mcp-workflow-hook.json")
    );

    let mutate = decide_permission(&input(
        "mcp", "active", true, Some("allow"), 0.9, 0.8, 0.1, 0.1, "", "", None, "linear__save_issue", "write", false,
    ))
    .unwrap();
    assert!(mutate.json.contains("\"reason\":\"mutating_mcp\""));
    assert_eq!(mutate.choice, "escalate");
    assert_ne!(mutate.choice, "continue");
    assert!(!mutate.auto_allow);
    assert_eq!(mutate.json, permission_golden("mcp-mutate-write.json"));
}

#[test]
fn mcp_read_follows_loop_stop_and_typing_is_adopted_only_when_honoring() {
    let read = decide_permission(&input(
        "mcp", "active", true, Some("allow"), 0.9, 0.8, 0.1, 0.1, "", "", None, "linear__list_issues", "read", false,
    ))
    .unwrap();
    assert_eq!(read.choice, "continue");
    assert_eq!(read.gate, "auto");
    assert_eq!(read.policy_id, ROUTING_POLICY_ID);
    assert!(read.json.contains("\"reason\":\"selected\""));
    assert!(!read.json.contains("workflow_is_not_permission"));
    assert!(!read.auto_allow);
    assert_eq!(read.json, permission_golden("mcp-read-list.json"));
    let hook = hook_of("shadow", "continue", "auto", false, &read);
    assert_eq!(hook.decision, "defer");
    assert_eq!(hook.reason, "exec");
    assert_eq!(
        format!(
            r#"{{"action":"defer","exitCode":0,"decision":"defer","reason":"{}"}}"#,
            hook.reason
        ),
        permission_golden("mcp-read-list-hook.json")
    );

    let typed_honoring = TypedPermission {
        honor: true,
        policy_id: ROUTING_POLICY_ID,
        transport: "facet",
        mapped: "continue",
        reason: "selected",
        code_deny: false,
        name: "linear__list_issues",
        effect: "read",
    };
    let adopted = decide_permission(&PermissionInput {
        routing_outcome: Some("check"),
        typed: Some(typed_honoring),
        ..input(
            "mcp", "active", true, Some("allow"), 0.0, 0.0, 0.0, 0.0, "", "", None, "linear__list_issues", "read", false,
        )
    })
    .unwrap();
    assert_eq!(adopted.policy_id, ROUTING_POLICY_ID);
    assert_eq!(adopted.choice, "continue");
    assert!(!adopted.auto_allow);
    assert_eq!(adopted.json, permission_golden("mcp-typed-facet.json"));

    let typed_shadow = TypedPermission {
        honor: false,
        ..typed_honoring
    };
    let still_ask = decide_permission(&PermissionInput {
        routing_outcome: Some("check"),
        typed: Some(typed_shadow),
        ..input(
            "mcp", "active", true, None, 0.0, 0.0, 0.0, 0.0, "", "", None, "linear__list_issues", "read", false,
        )
    })
    .unwrap();
    assert_ne!(still_ask.choice, "continue");
    assert!(still_ask.json.contains("\"label\":\"ask\""));
    assert!(!still_ask.json.contains("workflow_is_not_permission"));
    assert_eq!(still_ask.json, permission_golden("mcp-typed-not-honor.json"));
}

#[test]
fn permission_continue_does_not_clear_a_parent_stop_and_deny_blocks() {
    let permission = decide_permission(&input(
        "shell", "active", true, Some("allow"), 0.9, 0.8, 0.1, 0.1, "", "", None, "", "", true,
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
        &permission,
    );
    assert_eq!(merged, permission_golden("continue-keeps-parent-stop.json"));
    assert!(merged.contains("\"choice\":\"stop\""));
    assert!(merged.contains("\"autoAllow\":false"));
    let kept_hook = pretool_hook_decision(&GateVerdict {
        bypass: false,
        mode: "active".to_string(),
        choice: Some("stop".to_string()),
        gate: Some("hold".to_string()),
        blocked: true,
        policy_id: String::new(),
        permission: Some(surface_of(&permission)),
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

    let deny = decide_permission(&input(
        "shell", "active", true, Some("deny"), 0.9, 0.05, 0.9, 0.05, "", "", None, "", "", true,
    ))
    .unwrap();
    assert_eq!(deny.choice, "stop");
    let stopped = apply_permission_verdict(
        PermissionParent {
            choice: "continue",
            gate: "auto",
            blocked: false,
            exec: true,
            hitl: None,
            mode: "shadow",
        },
        &deny,
    );
    assert!(stopped.contains("\"choice\":\"stop\""));
    assert!(stopped.contains("\"blocked\":true"));
    assert!(stopped.contains("\"exec\":false"));

    let shadow_deny = decide_permission(&input(
        "shell", "shadow", true, Some("deny"), 0.9, 0.05, 0.9, 0.05, "", "", None, "", "", true,
    ))
    .unwrap();
    assert_eq!(shadow_deny.json, permission_golden("shell-shadow-deny.json"));
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
        permission: Some(surface_of(&shadow_deny)),
        calls: None,
    });
    assert_eq!(logged_hook.decision, "defer");
    assert_eq!(logged_hook.reason, "shadow");
    assert_ne!(logged_hook.decision, "allow");
}

#[test]
fn pretool_hook_never_allows_and_the_matcher_table_holds() {
    // Bypass and shadow defer; active stop/escalate/unclassified-hold deny.
    let samples = [
        (true, "active", "stop", "hold", true, "defer", "bypass"),
        (false, "shadow", "stop", "hold", false, "defer", "shadow"),
        (false, "active", "continue", "auto", false, "defer", "exec"),
        (false, "active", "unclassified", "auto", false, "defer", "exec"),
        (
            false,
            "active",
            "stop",
            "hold",
            true,
            "deny",
            "jev choice=stop gate=hold policy=omapi-loop-stop-policy@1",
        ),
        (false, "active", "escalate", "hold", true, "deny", "jev choice=escalate gate=hold"),
        (
            false,
            "active",
            "unclassified",
            "hold",
            false,
            "deny",
            "jev choice=unclassified gate=hold",
        ),
    ];
    for (bypass, mode, choice, gate, blocked, decision, reason) in samples {
        let mut verdict = GateVerdict::simple(mode, choice, gate, blocked);
        verdict.bypass = bypass;
        if decision == "deny" && choice == "stop" && blocked {
            verdict.policy_id = LOOP_STOP_POLICY_ID.to_string();
        }
        let hook = pretool_hook_decision(&verdict);
        assert_eq!(hook.decision, decision, "{reason}");
        assert_ne!(hook.decision, "allow");
        assert_eq!(hook.reason, reason);
        assert_eq!(hook.exit_code, if decision == "deny" { 2 } else { 0 });
        let line = hook_stdout(&hook);
        assert!(line.ends_with('\n'));
        assert!(!line.contains("\"decision\":\"allow\""));
    }
}
