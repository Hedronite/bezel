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

pub use calls::tool_call;
pub use catalog::{schema_dump_json, tiny_catalog_json};
pub use check::{
    check_envelope, deterministic_flags, file_kind, offline_check_json, parse_unified_diff, run_offline_check,
    write_flags, CheckReport, Hunk, OfflineCheck,
};
pub use cli::{fixture_client, run as cli_run, LIVE_COMMAND, LIVE_ON};
pub use facts::{clip_to_max_hunk_chars, gather, gather_diff, write_tool_wire, Edit, GatherOpts, WriteBody};
pub use judge::{matches_recorded, recorded_answer, CHECK_JUDGE, MAIN_GATE, SHADOW_WORKFLOW};
pub use harness::{
    decide_hook, hook_command, hook_on_text, hook_stdout, modes_from_env, parse_hook_event, HookEvent, Modes,
};
pub use loop_stop::{apply_loop_stop_thresholds, LoopStopDecision};
pub use permission::{active_hook, write_from_flags, WriteVerdict};
pub use router::bash_no_key;
pub use policy::{
    hook_decision, match_pretool_class, pretool_hook_decision, pretool_stamp, GateVerdict, HookOut, AUTO_ALLOW,
    CHECK_POLICY_ID, CONCERN_PARK, MAX_HUNK_CHARS, LOOP_STOP_MIN_CONFIDENCE, LOOP_STOP_MIN_MARGIN, LOOP_STOP_MIN_PROBABILITY,
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
        let live = ["facet", "request", "run"].join(" ");
        assert!(LIVE_COMMAND.starts_with(&live));
        let client = fixture_client();
        for name in ["check-judge.json", "shadow-workflow.json", "main-gate.json"] {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/jev-router/testdata/system-one")
                .join(name);
            let recorded = std::fs::read(&path).expect(name);
            let replayed = client.system_one(name).expect(name);
            assert_eq!(replayed, recorded, "{name}");
            assert!(matches_recorded(&replayed, &recorded));
            let mut paraphrased = recorded.clone();
            paraphrased.pop();
            assert!(!matches_recorded(&paraphrased, &recorded), "{name}");
        }
        assert!(client.system_one("missing.json").is_err());

        let call = tool_call();
        assert_eq!(call.transport, "facet");
        assert!(!call.initiated);
        assert_eq!(call.op, "tool_call");
        assert_eq!(call.facet_version, "2.1.3");

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
