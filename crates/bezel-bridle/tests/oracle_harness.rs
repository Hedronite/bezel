//! Oracle: harness. Every behavior `packages/jev-router/test.mjs` asserts for
//! the PreToolUse adapter — modes, hook events, hook decisions, the bypass
//! predicate, and stdin/stdout behavior — against the shipped crate.

use bezel_bridle::{
    decide_hook, hook_command, hook_on_text, hook_stdout, is_jev_bypass, match_pretool_class,
    modes_from_env, parse_hook_event, pretool_hook_decision, pretool_stamp, CHECK_POLICY_ID,
    GateVerdict, HookEvent, HookOut, LOOP_STOP_POLICY_ID, Modes, ROUTING_POLICY_ID,
};

#[test]
fn jev_bypass_is_only_one_or_true() {
    assert!(is_jev_bypass("1"));
    assert!(is_jev_bypass("true"));
    for value in ["", "0", "false", "yes", "TRUE", " 1"] {
        assert!(!is_jev_bypass(value), "{value:?}");
    }
}

#[test]
fn modes_only_honor_with_active_requests() {
    // JEV_MODE=active honors; yes/true permission values never do.
    let modes = modes_from_env(&[
        ("JEV_MODE", "active"),
        ("JEV_PERMISSION_MODE", "yes"),
        ("JEV_TYPED_CALL_MODE", "true"),
    ]);
    assert_eq!(modes.jev_mode, "active");
    assert!(!modes.bypass);
    assert!(modes.honors);

    let with_key = modes_from_env(&[
        ("JEV_MODE", "active"),
        ("TYPESAFE_API_KEY", "present-key"),
    ]);
    assert!(with_key.honors);

    let permission_only = modes_from_env(&[
        ("JEV_MODE", "shadow"),
        ("JEV_PERMISSION_MODE", "ACTIVE"),
        ("TYPESAFE_API_KEY", "present-key"),
    ]);
    assert!(permission_only.honors);
    assert_eq!(permission_only.jev_mode, "shadow");

    let typed_only = modes_from_env(&[
        ("JEV_MODE", "shadow"),
        ("JEV_TYPED_CALL_MODE", "active"),
        ("TYPESAFE_API_KEY", "present-key"),
    ]);
    assert!(typed_only.honors);

    // Pure shadow without any active request does not honor.
    let quiet = modes_from_env(&[("JEV_MODE", "shadow"), ("TYPESAFE_API_KEY", "present-key")]);
    assert!(!quiet.honors);
}

#[test]
fn parse_hook_event_reads_snake_and_camel_tool_names() {
    let snake = parse_hook_event(r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#).unwrap();
    assert_eq!(snake.tool_name, "Bash");

    let camel = parse_hook_event(r#"{"toolName":"search_replace"}"#).unwrap();
    assert_eq!(camel.tool_name, "search_replace");

    assert!(parse_hook_event("").is_err());
    assert!(parse_hook_event("not-json").is_err());
    let no_tool = parse_hook_event(r#"{"other":1}"#).unwrap();
    assert_eq!(no_tool.tool_name, "");
}

#[test]
fn pretool_stamp_maps_classes_and_policy_ids() {
    let shell = pretool_stamp("Bash").unwrap();
    assert!(shell.matched);
    assert_eq!(shell.class_id, Some("shell"));
    assert_eq!(shell.policy_id, Some(LOOP_STOP_POLICY_ID));

    let write = pretool_stamp("search_replace").unwrap();
    assert_eq!(write.class_id, Some("write"));
    assert_eq!(write.policy_id, Some(CHECK_POLICY_ID));

    let mcp = pretool_stamp("linear__save_issue").unwrap();
    assert_eq!(mcp.class_id, Some("mcp"));
    assert_eq!(mcp.policy_id, Some(ROUTING_POLICY_ID));

    assert!(pretool_stamp("").is_none());
    let unmatched = pretool_stamp("read_file").unwrap();
    assert!(!unmatched.matched);
    assert_eq!(unmatched.class_id, None);

    // match_pretool_class holds for every mapped token in test.mjs.
    for name in [
        "web_search", "WebSearch", "web_fetch", "WebFetch",
        "spawn_subagent", "Task", "Bash", "run_terminal_command", "run_terminal_cmd",
        "Write", "Edit", "MultiEdit", "search_replace",
        "linear__save_issue", "mcp__filesystem__read_file",
    ] {
        assert!(match_pretool_class(name).is_some(), "{name}");
    }
    for name in ["read_file", "grep", "use_tool", "search_tool", "todo_write"] {
        assert_eq!(match_pretool_class(name), None, "{name}");
    }
    assert_eq!(match_pretool_class("web_search").map(|row| row.0), Some("web"));
    assert_eq!(match_pretool_class("Task").map(|row| row.0), Some("subagent"));
    assert_eq!(match_pretool_class("run_terminal_cmd").map(|row| row.0), Some("shell"));
}

#[test]
fn hook_decisions_defer_or_deny_and_never_allow() {
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
        if decision == "deny" && choice == "stop" {
            verdict.policy_id = LOOP_STOP_POLICY_ID.to_string();
        }
        let hook = pretool_hook_decision(&verdict);
        assert_eq!(hook.decision, decision, "{reason}");
        assert_ne!(hook.decision, "allow");
        assert_eq!(hook.reason, reason);
        assert_eq!(hook.exit_code, if decision == "deny" { 2 } else { 0 });
    }
}

#[test]
fn decide_hook_bypass_unmatched_and_uncertain_paths() {
    let bash = HookEvent {
        tool_name: "Bash".to_string(),
        command: String::new(),
    };
    let bypass = decide_hook(
        &bash,
        &modes_from_env(&[("JEV_BYPASS", "1")]),
        Some(&GateVerdict::simple("active", "stop", "hold", true)),
    );
    assert_eq!(bypass.decision, "defer");
    assert_eq!(bypass.reason, "bypass");

    // "yes" is not a bypass value, so the verdict still denies.
    let not_bypass = decide_hook(
        &bash,
        &modes_from_env(&[("JEV_BYPASS", "yes"), ("JEV_MODE", "active")]),
        Some(&GateVerdict {
            policy_id: LOOP_STOP_POLICY_ID.to_string(),
            ..GateVerdict::simple("active", "stop", "hold", true)
        }),
    );
    assert_eq!(not_bypass.decision, "deny");

    // An unmatched tool defers without consulting the router.
    let unmapped = decide_hook(
        &HookEvent {
            tool_name: "read_file".to_string(),
            command: String::new(),
        },
        &modes_from_env(&[("JEV_MODE", "active")]),
        Some(&GateVerdict::simple("active", "continue", "auto", false)),
    );
    assert_eq!(unmapped.decision, "defer");
    assert_eq!(unmapped.reason, "unmatched");

    // No verdict: active denies as jev uncertain, shadow defers.
    let active = modes_from_env(&[("JEV_MODE", "active")]);
    let shadow = modes_from_env(&[("JEV_MODE", "shadow")]);
    let uncertain = hook_on_text("not-json", &active, None);
    assert_eq!(uncertain.decision, "deny");
    assert_eq!(uncertain.reason, "jev uncertain");
    let shadow_empty = hook_on_text("not-json", &shadow, None);
    assert_eq!(shadow_empty.decision, "defer");
    assert_eq!(shadow_empty.reason, "shadow");

    // Shadow defers a real verdict; an active stop denies with the stamp policy.
    let deferred = decide_hook(
        &bash,
        &shadow,
        Some(&GateVerdict::simple("shadow", "continue", "auto", false)),
    );
    assert_eq!(deferred.decision, "defer");
    let denied = decide_hook(
        &bash,
        &active,
        Some(&GateVerdict {
            policy_id: LOOP_STOP_POLICY_ID.to_string(),
            ..GateVerdict::simple("active", "stop", "hold", true)
        }),
    );
    assert_eq!(denied.decision, "deny");
    assert!(denied.reason.contains(&format!("policy={LOOP_STOP_POLICY_ID}")));
}

#[test]
fn hook_command_matches_node_stdin_to_stdout_and_exit() {
    let bash = r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#;
    let rows = [
        (
            bash,
            &[("JEV_MODE", "active")][..],
            2,
            "{\"decision\":\"deny\",\"reason\":\"jev choice=escalate gate=hold policy=omapi-loop-stop-policy@1 detail=missing_key question=Is the judge available for this action?\"}\n",
        ),
        (
            bash,
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
        assert!(!got.contains("\"decision\":\"allow\""));
    }
}

#[test]
fn hook_stdout_is_one_json_line_of_defer_or_deny() {
    let defer = hook_stdout(&HookOut {
        decision: "defer",
        reason: "exec".to_string(),
        exit_code: 0,
    });
    assert_eq!(defer, "{\"decision\":\"defer\",\"reason\":\"exec\"}\n");
    let deny = hook_stdout(&HookOut {
        decision: "deny",
        reason: "jev choice=stop gate=\"hold\"".to_string(),
        exit_code: 2,
    });
    assert_eq!(deny, "{\"decision\":\"deny\",\"reason\":\"jev choice=stop gate=\\\"hold\\\"\"}\n");
}

#[test]
fn modes_struct_is_opaque_and_event_is_typed() {
    // Modes and HookEvent are plain data the harness layer reads.
    let modes = Modes {
        jev_mode: "shadow",
        bypass: false,
        honors: false,
    };
    assert_eq!(modes.jev_mode, "shadow");
    let event = parse_hook_event(r#"{"tool_name":"Write"}"#).unwrap();
    assert_eq!(event.tool_name, "Write");
}
