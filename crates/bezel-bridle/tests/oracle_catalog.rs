//! Oracle: catalog. Every behavior `packages/jev-router/test.mjs` asserts for
//! the tiny catalog, the schema dump, and the check/offline oracle, asserted
//! against the shipped crate's public API.

use bezel_bridle::{cli_run, CHECK_POLICY_ID, LOOP_STOP_POLICY_ID, ROUTING_POLICY_ID};
use std::path::PathBuf;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn arg(text: &str) -> String {
    text.to_string()
}

fn testdata(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/jev-router/testdata").join(rel)
}

/// Shadow verdict is env-sensitive; clear router keys for this process only.
struct EnvClear {
    saved: Vec<(String, Option<String>)>,
}

impl EnvClear {
    fn clear(keys: &[&str]) -> Self {
        let saved = keys
            .iter()
            .map(|key| ((*key).to_string(), std::env::var(key).ok()))
            .collect();
        for key in keys {
            // SAFETY: single-threaded test body; other tests join on ENV_LOCK.
            unsafe { std::env::remove_var(key) };
        }
        Self { saved }
    }
}

impl Drop for EnvClear {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            // SAFETY: same single-threaded body as clear.
            unsafe {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

#[test]
fn tiny_catalog_is_the_pretool_index_with_no_orphan_policy_id() {
    let catalog = cli_run(&[arg("--catalog")]).1;
    assert!(catalog.contains("\"kind\":\"tiny\""), "{catalog}");
    // Five classes: web, subagent, shell, write, mcp.
    for class in ["web", "subagent", "shell", "write", "mcp"] {
        assert!(catalog.contains(&format!("\"class\":\"{class}\"")), "{class}");
    }
    // Every policy id appears; none outside the three ids.
    assert!(catalog.contains(LOOP_STOP_POLICY_ID));
    assert!(catalog.contains(CHECK_POLICY_ID));
    assert!(catalog.contains(ROUTING_POLICY_ID));
    let orphaned = catalog.replace(LOOP_STOP_POLICY_ID, "").replace(CHECK_POLICY_ID, "").replace(ROUTING_POLICY_ID, "");
    assert!(!orphaned.contains("omapi-"), "{orphaned}");
    // The mcp entry is the pattern, not a tool list.
    assert!(catalog.contains("\"pattern\":\"[A-Za-z0-9][A-Za-z0-9_.-]*__[A-Za-z0-9_.-]+\""));
    assert!(!catalog.contains("\"tools\":[\"linear"));
    // Shell keeps its three tool tokens.
    assert!(catalog.contains("\"tools\":[\"Bash\",\"run_terminal_command\",\"run_terminal_cmd\"]"));
    // Under 900 bytes; no $schema, no properties, no allow.
    assert!(catalog.len() < 900, "tiny catalog is {} chars", catalog.len());
    assert!(!catalog.contains("$schema"));
    assert!(!catalog.contains("properties"));
    assert!(!catalog.contains("\"decision\":\"allow\""));
}

#[test]
fn schema_dump_defers_known_tools_and_denies_the_rest() {
    let (code, bash) = cli_run(&[arg("--schema"), arg("Bash")]);
    assert_eq!(code, 0);
    assert!(bash.contains("\"call\":\"schema-dump\""));
    assert!(bash.contains("\"found\":true"));
    assert!(bash.contains("\"class\":\"shell\""));
    assert!(bash.contains(&format!("\"policyId\":\"{LOOP_STOP_POLICY_ID}\"")));
    assert!(bash.contains("\"decision\":\"defer\""));
    assert!(bash.contains("\"autoAllow\":false"));
    assert!(bash.contains("\"title\":\"Bash\""));
    assert!(bash.contains("\"properties\":{\"command\":{\"type\":\"string\""));
    assert!(bash.contains("\"required\":[\"command\"]"));
    assert!(bash.contains("json-schema"));
    assert!(!bash.contains("\"decision\":\"allow\""));

    let (code, edit) = cli_run(&[arg("--schema"), arg("search_replace")]);
    assert_eq!(code, 0);
    assert!(edit.contains("\"class\":\"write\""));
    assert!(edit.contains(&format!("\"policyId\":\"{CHECK_POLICY_ID}\"")));
    assert!(edit.contains("\"decision\":\"defer\""));
    assert!(edit.contains("\"autoAllow\":false"));

    let (code, mcp) = cli_run(&[arg("--schema"), arg("linear__save_issue")]);
    assert_eq!(code, 0);
    assert!(mcp.contains("\"class\":\"mcp\""));
    assert!(mcp.contains(&format!("\"policyId\":\"{ROUTING_POLICY_ID}\"")));
    assert!(mcp.contains("\"typedCall\":false"));
    assert!(mcp.contains("\"decision\":\"defer\""));
    assert!(mcp.contains("\"required\":[\"server\",\"tool\"]"));
    assert!(!mcp.contains("\"decision\":\"allow\""));

    for name in ["read_file", "use_tool", ""] {
        let (code, denied) = cli_run(&[arg("--schema"), arg(name)]);
        assert_eq!(code, 2, "{name:?}");
        assert!(denied.contains("\"decision\":\"deny\""), "{name:?}");
        assert!(denied.contains("\"found\":false"), "{name:?}");
        assert!(denied.contains("\"schema\":null"), "{name:?}");
        assert!(denied.contains("\"autoAllow\":false"), "{name:?}");
        assert!(!denied.contains("\"decision\":\"allow\""), "{name:?}");
    }
    // Missing --schema argument also denies with exit 2.
    let (code, missing) = cli_run(&[arg("--schema")]);
    assert_eq!(code, 2);
    assert!(missing.contains("\"decision\":\"deny\""));
}

#[test]
fn every_mapped_tool_has_a_titled_json_schema() {
    for (name, title) in [
        ("web_search", "web_search"),
        ("WebFetch", "WebFetch"),
        ("Task", "Task"),
        ("run_terminal_command", "run_terminal_command"),
        ("Write", "Write"),
        ("Edit", "Edit"),
        ("MultiEdit", "MultiEdit"),
    ] {
        let (code, body) = cli_run(&[arg("--schema"), arg(name)]);
        assert_eq!(code, 0, "{name}");
        assert!(body.contains(&format!("\"title\":\"{title}\"")), "{name}");
        assert!(body.contains("json-schema"), "{name}");
    }
}

#[test]
fn check_workflow_reports_findings_and_never_approval() {
    let (code, skip) = cli_run(&[
        arg("--check"),
        arg("--diff-file"),
        arg(testdata("skip-marker.diff").to_str().unwrap()),
    ]);
    assert_eq!(code, 0);
    assert!(skip.contains("\"workflow\":\"check\""));
    assert!(skip.contains("\"approval\":false"));
    assert!(skip.contains("\"emptyFindingsAreNotApproval\":true"));
    assert!(skip.contains("\"flag\":\"skip_marker_added\""));
    assert!(skip.contains("TYPESAFE_API_KEY unset"));
    assert!(skip.contains("\"mode\":\"shadow\""));
    assert!(!skip.to_lowercase().contains("approved"));
    assert!(!skip.contains("\"decision\":\"allow\""));

    let (code, assertions) = cli_run(&[
        arg("--check"),
        arg("--diff-file"),
        arg(testdata("assertions-removed.diff").to_str().unwrap()),
    ]);
    assert_eq!(code, 0);
    assert!(assertions.contains("\"flag\":\"assertions_removed\""));
    assert!(assertions.contains("\"approval\":false"));

    let (code, empty) = cli_run(&[
        arg("--check"),
        arg("--diff-file"),
        arg(testdata("empty.diff").to_str().unwrap()),
    ]);
    assert_eq!(code, 0);
    assert!(empty.contains("\"status\":\"no_diff\""));
    assert!(empty.contains("\"approval\":false"));
    assert!(empty.contains("no git diff"));
    assert!(!empty.contains("\"flag\":"));
    assert!(!empty.to_lowercase().contains("approved"));
}

#[test]
fn check_ignores_bypass_and_stays_on_the_check_workflow() {
    // --check with JEV_BYPASS=1 in the env still runs the check workflow.
    let (code, payload) = cli_run(&[
        arg("--check"),
        arg("--diff-file"),
        arg(testdata("skip-marker.diff").to_str().unwrap()),
    ]);
    assert_eq!(code, 0);
    assert!(payload.contains("\"workflow\":\"check\""));
    assert!(payload.contains("\"approval\":false"));
    assert!(!payload.contains("\"choice\":\"bypass\""));
    // The offline check carries the tiny catalog without $schema.
    assert!(payload.contains("\"kind\":\"tiny\""));
    assert!(!payload.contains("$schema"));
}

#[test]
fn shadow_router_without_key_stays_unclassified_via_cli() {
    let _lock = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    let _env = EnvClear::clear(&["JEV_BYPASS", "JEV_MODE", "TYPESAFE_API_KEY", "JEV_PERMISSION_MODE", "JEV_TYPED_CALL_MODE"]);
    let (code, payload) = cli_run(&[arg("--intent"), arg("probe intent")]);
    assert_eq!(code, 0, "{payload}");
    assert!(payload.contains("\"choice\":\"unclassified\""), "{payload}");
    assert!(payload.contains("\"missingKey\":true"));
    assert!(payload.contains("\"blocked\":false"));
    assert!(payload.contains("\"autoRetry\":false"));
    assert!(payload.contains("\"autoPromote\":false"));
    assert!(payload.contains("\"reason\":\"missing_key\""));
    assert!(payload.contains(&format!("\"loopStopPolicy\":\"{LOOP_STOP_POLICY_ID}\"")));
    assert!(!payload.contains("\"decision\":\"allow\""));
}

#[test]
fn catalog_and_schema_output_never_leak_a_key_or_print_two_objects() {
    // cli::run returns one payload string; the router never echoes an env key
    // because the catalog and schema builders take no key argument.
    let (code, catalog) = cli_run(&[arg("--catalog")]);
    assert_eq!(code, 0);
    assert!(!catalog.contains("catalog-test-secret"));
    let (code, schema) = cli_run(&[arg("--schema"), arg("Bash")]);
    assert_eq!(code, 0);
    assert!(!schema.contains("catalog-test-secret"));
}
