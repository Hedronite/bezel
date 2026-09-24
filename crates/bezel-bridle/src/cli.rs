//! Offline argv. This is the only module that may construct a client.
//! The only live transport command is built here.

use crate::catalog::{schema_dump_json, tiny_catalog_json};
use crate::check::{offline_check_json, run_offline_check};
use crate::facts::{gather, GatherOpts};
use crate::transport::{recorded_dir, FixtureTransport};
use std::path::{Path, PathBuf};

pub const LIVE_COMMAND: &str = "facet request run --environment typesafe --no-record";
pub const LIVE_ON: bool = false;

/// The only live transport. No other module builds this command.
pub fn live_transport_argv() -> [&'static str; 6] {
    ["facet", "request", "run", "--environment", "typesafe", "--no-record"]
}

pub struct Client {
    fixture: FixtureTransport,
    live: [&'static str; 6],
}

/// Fixture client, or the Facet command when a key is present and live is on.
/// Does not link a TypeSafe SDK.
pub fn construct_client(dir: impl Into<PathBuf>) -> Client {
    let live = live_transport_argv();
    debug_assert_eq!(live.join(" "), LIVE_COMMAND);
    Client {
        fixture: FixtureTransport::open(dir),
        live,
    }
}

impl Client {
    pub fn system_one(&self, name: &str) -> Result<Vec<u8>, String> {
        if live_enabled() {
            return run_live(&self.live, name);
        }
        self.fixture.system_one(name)
    }
}

fn live_enabled() -> bool {
    if !LIVE_ON {
        return false;
    }
    std::env::var("TYPESAFE_API_KEY")
        .map(|value| !value.is_empty())
        .unwrap_or(false)
}

fn run_live(argv: &[&str], name: &str) -> Result<Vec<u8>, String> {
    let mut command = std::process::Command::new(argv[0]);
    command.args(&argv[1..]);
    let _ = (command, name);
    Err(format!("live transport refused for a recorded body; command is {LIVE_COMMAND}"))
}

pub fn fixture_client() -> Client {
    construct_client(recorded_dir())
}

#[cfg(test)]
pub(crate) fn test_env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
}

pub fn run(args: &[String]) -> (i32, String) {
    if args.first().map(String::as_str) == Some("--version") && args.len() == 1 {
        return (0, format!("bezel-bridle {}\n", env!("CARGO_PKG_VERSION")));
    }
    if args.first().map(String::as_str) == Some("hook") {
        let mut stdin = String::new();
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut stdin);
        return hook_cli(&stdin);
    }
    if args.first().map(String::as_str) == Some("smoke") {
        return (0, format!("{}\n", crate::harness::smoke_report()));
    }
    if args.first().map(String::as_str) == Some("config") {
        return (0, format!("{}\n", crate::harness::harness_config()));
    }
    if args.first().map(String::as_str) == Some("catalog") || args.iter().any(|arg| arg == "--catalog") {
        return (0, format!("{}\n", tiny_catalog_json()));
    }
    if let Some(index) = args.iter().position(|arg| arg == "--schema" || arg == "--schema-dump") {
        let name = args.get(index + 1).map(String::as_str).unwrap_or("");
        if name.is_empty() || name.starts_with('-') {
            return (2, format!("{}\n", schema_dump_json("")));
        }
        let dumped = schema_dump_json(name);
        let code = if dumped.contains("\"decision\":\"deny\"") { 2 } else { 0 };
        return (code, format!("{dumped}\n"));
    }
    if args.iter().any(|arg| arg == "--check") {
        let diff = diff_argument(args);
        let report = run_offline_check(&diff);
        return (0, format!("{}\n", offline_check_json(&report)));
    }
    router_verdict(args)
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn hook_cli_matches_hook_command() {
        let _lock = env_lock();
        let _env = EnvSet::apply(&[("JEV_ROUTER", None)]);
        let stdin = r#"{"tool_name":"Bash","tool_input":{"command":"npm test"}}"#;
        let (code, out) = super::hook_cli(stdin);
        let owned: Vec<(String, String)> = std::env::vars().collect();
        let pairs: Vec<(&str, &str)> = owned.iter().map(|(key, value)| (key.as_str(), value.as_str())).collect();
        let (expect_code, expect) = crate::harness::hook_command(stdin, &pairs);
        assert_eq!(code, expect_code);
        assert_eq!(out, expect);
        assert!(out.contains("\"decision\":\"defer\"") || out.contains("\"decision\":\"deny\""));
        assert!(!out.contains("\"decision\":\"allow\""));
    }

    /// grok-build-harness spawns JEV_ROUTER for a matched hook and skips it on bypass.
    #[test]
    fn hook_cli_spawns_router_stop_and_skips_bypass() {
        let _lock = env_lock();
        let dir = std::env::temp_dir().join(format!("bezel-hook-spawn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let router = dir.join("router");
        let garbage = dir.join("garbage");
        let argv = dir.join("argv");
        let called = dir.join("called");
        std::fs::write(
            &router,
            "#!/bin/sh\nif [ \"${HARNESS_MARK:-}\" = \"poison\" ]; then\n  : > \"${HARNESS_CALLED_FILE:?}\"\n  exit 99\nfi\nprintf '%s\\n' \"$*\" > \"${HARNESS_ARGV_LOG:-/dev/null}\"\nprintf '%s\\n' '{\"ok\":true,\"mode\":\"active\",\"choice\":\"stop\",\"gate\":\"hold\",\"blocked\":true,\"exec\":false,\"bypass\":false}'\n",
        )
        .unwrap();
        std::fs::write(&garbage, "#!/bin/sh\nprintf '%s\\n' 'not-json'\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&router, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::set_permissions(&garbage, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let event = r#"{"hookEventName":"PreToolUse","toolName":"Bash","toolInput":{"command":"npm test"}}"#;
        let _env = EnvSet::apply(&[
            ("JEV_ROUTER", Some(router.to_str().unwrap())),
            ("JEV_BYPASS", Some("")),
            ("JEV_MODE", Some("active")),
            ("HARNESS_ARGV_LOG", Some(argv.to_str().unwrap())),
            ("HARNESS_MARK", None),
            ("HARNESS_CALLED_FILE", None),
            ("TYPESAFE_API_KEY", None),
        ]);
        std::fs::write(&argv, "").unwrap();
        let (code, out) = super::hook_cli(event);
        assert_eq!(code, 2, "{out}");
        assert!(out.contains("\"decision\":\"deny\""), "{out}");
        assert!(out.contains("policy=omapi-loop-stop-policy@1"), "{out}");
        let logged = std::fs::read_to_string(&argv).unwrap();
        assert!(logged.contains("--tool-name"), "{logged}");
        assert!(logged.contains("Bash"), "{logged}");
        assert!(logged.contains("--tool-input"), "{logged}");
        assert!(logged.contains("npm test"), "{logged}");

        set_env("JEV_BYPASS", Some("1"));
        set_env("HARNESS_MARK", Some("poison"));
        set_env("HARNESS_CALLED_FILE", Some(called.to_str().unwrap()));
        let (code, out) = super::hook_cli(event);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"decision\":\"defer\"") && out.contains("\"reason\":\"bypass\""), "{out}");
        assert!(!called.exists(), "bypass must not call the router");

        set_env("JEV_BYPASS", Some(""));
        set_env("HARNESS_MARK", None);
        set_env("JEV_MODE", Some("active"));
        set_env("JEV_ROUTER", Some(garbage.to_str().unwrap()));
        let (code, out) = super::hook_cli(event);
        assert_eq!(code, 2, "{out}");
        assert!(out.contains("\"reason\":\"jev uncertain\""), "{out}");

        set_env("JEV_MODE", Some("shadow"));
        let (code, out) = super::hook_cli(event);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"decision\":\"defer\"") && out.contains("\"reason\":\"shadow\""), "{out}");

        set_env("JEV_ROUTER", None);
        set_env("JEV_MODE", Some("shadow"));
        set_env("JEV_BYPASS", Some(""));
        let (code, out) = super::hook_cli("");
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"decision\":\"defer\""), "{out}");
        assert!(!out.contains("oma on") && !out.contains("omapi-mark") && !out.contains("bezel-mark"));

        let (code, out) = run(&["catalog".into()]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"kind\":\"tiny\""), "{out}");
        assert!(!out.contains("oma on") && !out.contains("omapi-mark") && !out.contains("bezel-mark"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_matches_package() {
        let (code, out) = run(&["--version".into()]);
        assert_eq!(code, 0);
        assert_eq!(out, format!("bezel-bridle {}\n", env!("CARGO_PKG_VERSION")));
    }

    /// Flake `jev-bypass` and `jev-check-shadow` read this JSON from `run`.
    #[test]
    fn probe_intent_run_prints_router_json() {
        let _lock = env_lock();
        let _env = EnvSet::apply(&[
            ("JEV_BYPASS", None),
            ("JEV_MODE", None),
            ("TYPESAFE_API_KEY", None),
            ("JEV_PERMISSION_MODE", None),
        ]);

        set_env("JEV_BYPASS", Some("1"));
        let (code, out) = run(&["probe intent".into()]);
        assert_eq!(code, 0, "{out}");
        assert!(out.ends_with('\n') && !out.trim_end_matches('\n').contains('\n'));
        assert!(out.contains("\"bypass\":true"));
        assert!(out.contains("\"gate\":\"auto\""));
        assert!(out.contains("\"blocked\":false"));
        assert!(!out.contains("\"blocked\":true"));

        set_env("JEV_BYPASS", Some("true"));
        let (code, out) = run(&[
            "--tool-name".into(),
            "run_terminal_command".into(),
            "probe intent".into(),
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"bypass\":true"));
        assert!(out.contains(
            "\"pretool\":{\"matched\":true,\"class\":\"shell\",\"policyId\":\"omapi-loop-stop-policy@1\""
        ));

        set_env("JEV_BYPASS", Some("yes"));
        set_env("JEV_MODE", None);
        let (code, out) = run(&["probe intent".into()]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"bypass\":false"));
        assert!(out.contains("\"missingKey\":true"));
        assert!(out.contains("\"blocked\":false"));
        assert!(!out.contains("\"pretool\""));
        assert!(!out.contains("\"blocked\":true"));

        set_env("JEV_BYPASS", None);
        set_env("JEV_MODE", Some("shadow"));
        let (code, out) = run(&["probe intent".into()]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"missingKey\":true"));
        assert!(out.contains("\"blocked\":false"));
        assert!(out.contains("\"workflow\":{\"choice\":\"unclassified\",\"outcome\":\"cannot_tell\",\"reason\":\"missing_key\",\"shadow\":true,\"blocked\":false}"));
        assert!(out.contains("\"capabilities\":{"));
        assert!(out.contains("\"autoRetry\":false"));
        assert!(out.contains("\"autoPromote\":false"));
        assert!(out.contains("\"reason\":\"missing_key\""));
        assert!(out.contains("\"loopStopPolicy\":\"omapi-loop-stop-policy@1\""));
        assert!(out.contains("\"choice\":\"unclassified\""));
        assert!(out.contains("\"kind\":\"tiny\""));
        assert!(!out.contains("\"blocked\":true"));

        let (code, out) = run(&[
            "--loop-stop".into(),
            "--intent".into(),
            "probe".into(),
            "--step-digest".into(),
            "step one failed".into(),
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"stepDigest\":\"step one failed\""));
        assert!(out.contains("\"exec\":true"));
        assert!(out.contains("\"choice\":\"unclassified\""));

        let (code, out) = run(&[
            "--tool-name".into(),
            "search_replace".into(),
            "probe intent".into(),
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"bypass\":false"));
        assert!(out.contains(
            "\"pretool\":{\"matched\":true,\"class\":\"write\",\"policyId\":\"omapi-check-policy@1\""
        ));

        set_env("JEV_BYPASS", Some("1"));
        let (code, out) = run(&[
            "--tool-name".into(),
            "linear__save_issue".into(),
            "probe intent".into(),
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"bypass\":true"));
        assert!(out.contains(
            "\"pretool\":{\"matched\":true,\"class\":\"mcp\",\"policyId\":\"omapi-route-workflow-policy@1\""
        ));

        set_env("JEV_BYPASS", None);
        set_env("JEV_MODE", Some("shadow"));
        set_env("JEV_PERMISSION_MODE", Some("active"));
        let (code, out) = run(&[
            "--tool-name".into(),
            "Bash".into(),
            "--tool-input".into(),
            "npm test".into(),
            "probe intent".into(),
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"missingKey\":true"));
        assert!(out.contains("\"blocked\":false"));
        assert!(out.contains("\"honor\":false"));
        assert!(out.contains("\"reason\":\"missing_key\""));
        assert!(out.contains("\"autoAllow\":false"));
        assert!(out.contains("\"policyId\":\"omapi-loop-stop-policy@1\""));

        let (code, out) = run(&[
            "--tool-name".into(),
            "Write".into(),
            "--tool-input".into(),
            ".env".into(),
            "probe intent".into(),
        ]);
        assert_eq!(code, 0, "{out}");
        assert!(out.contains("\"blocked\":false"));
        assert!(out.contains("\"choice\":\"unclassified\""));
        assert!(out.contains("\"codeDeny\":true"));
        assert!(out.contains("\"mapped\":\"stop\""));
        assert!(out.contains("\"policyId\":\"omapi-check-policy@1\""));
        assert!(!out.contains("\"blocked\":true"));

        let path = std::env::temp_dir().join(format!("jev-calls-{}.json", std::process::id()));
        std::fs::write(
            &path,
            r#"{"interface":"Mcp","TYPESAFE_API_KEY":"supersecretvalue","tools":[{"name":"linear__list_issues","description":"List issues","effect":"read","args":[]}]}"#,
        )
        .unwrap();
        let (code, out) = run(&[
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
        assert!(!out.contains("supersecretvalue"));
        assert!(out.contains("\"transport\":\"facet\""));
        assert!(out.contains("\"initiated\":false"));
        assert!(out.contains("\"best\":null"));
        assert!(out.contains("\"reason\":\"missing_key\""));
        assert!(out.contains("\"op\":\"tool_expose\""));
        assert!(out.contains("\"name\":\"Mcp.linear__list_issues\""));
        assert!(out.contains("\"class\":\"mcp\""));
        assert!(!out.contains("jsonrpc"));
        assert!(!out.contains("tools/call"));
    }

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        super::test_env_lock()
    }

    fn set_env(key: &str, value: Option<&str>) {
        // Serial with env_lock. Restored when EnvSet drops.
        unsafe {
            match value {
                Some(text) => std::env::set_var(key, text),
                None => std::env::remove_var(key),
            }
        }
    }

    struct EnvSet {
        saved: Vec<(String, Option<String>)>,
    }

    impl EnvSet {
        fn apply(pairs: &[(&str, Option<&str>)]) -> Self {
            let saved = pairs
                .iter()
                .map(|(key, _)| ((*key).to_string(), std::env::var(key).ok()))
                .collect();
            for (key, value) in pairs {
                set_env(key, *value);
            }
            Self { saved }
        }
    }

    impl Drop for EnvSet {
        fn drop(&mut self) {
            for (key, value) in &self.saved {
                set_env(key, value.as_deref());
            }
        }
    }
}

/// `jev-router hook` reads stdin and uses the harness command.
fn hook_cli(stdin: &str) -> (i32, String) {
    let owned: Vec<(String, String)> = std::env::vars().collect();
    let pairs: Vec<(&str, &str)> = owned.iter().map(|(key, value)| (key.as_str(), value.as_str())).collect();
    let modes = crate::harness::modes_from_env(&pairs);
    let event = crate::harness::parse_hook_event(stdin).unwrap_or(crate::harness::HookEvent {
        tool_name: String::new(),
        command: String::new(),
    });
    let matched = !event.tool_name.is_empty()
        && crate::policy::pretool_stamp(&event.tool_name).is_some_and(|stamp| stamp.matched);
    let router = std::env::var("JEV_ROUTER").unwrap_or_default();
    if modes.bypass || !matched || router.is_empty() {
        return crate::harness::hook_command(stdin, &pairs);
    }
    let verdict = spawn_router(&router, &event);
    let decision = crate::harness::decide_hook(&event, &modes, verdict.as_ref());
    (decision.exit_code, crate::harness::hook_stdout(&decision))
}

fn spawn_router(bin: &str, event: &crate::harness::HookEvent) -> Option<crate::policy::GateVerdict> {
    let mut args = vec![
        "--tool-name".to_string(),
        event.tool_name.clone(),
        "--intent".to_string(),
        event.tool_name.clone(),
    ];
    if !event.command.is_empty() {
        args.push("--tool-input".to_string());
        args.push(event.command.clone());
    }
    let output = std::process::Command::new(bin)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.trim().lines().last().unwrap_or("").trim();
    if !line.starts_with('{') {
        return None;
    }
    let choice = json_field(line, "choice").unwrap_or_else(|| "unclassified".to_string());
    let gate = json_field(line, "gate").unwrap_or_else(|| "hold".to_string());
    let mode = json_field(line, "mode").unwrap_or_else(|| "shadow".to_string());
    let blocked = line.contains("\"blocked\":true");
    let mut verdict = crate::policy::GateVerdict::simple(&mode, &choice, &gate, blocked);
    verdict.bypass = line.contains("\"bypass\":true");
    Some(verdict)
}

fn json_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let rest = text[text.find(&needle)? + needle.len()..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest.split('"').next().unwrap_or("").to_string())
}

fn diff_argument(args: &[String]) -> String {
    let Some(index) = args.iter().position(|arg| arg == "--diff-file") else {
        return String::new();
    };
    let Some(path) = args.get(index + 1) else {
        return String::new();
    };
    std::fs::read_to_string(Path::new(path)).unwrap_or_default()
}

const FAMILY_LOOP: &str = r#"["continue","stop","escalate"]"#;
const FAMILY_CHECK: &str = r#"["none","test_safety","task_mismatch","cannot_tell"]"#;
const FAMILY_ROUTE: &str = r#"["check","review","cannot_tell"]"#;

struct RouterArgs {
    intent: String,
    step_digest: String,
    tool_name: String,
    tool_class: String,
    tool_input: String,
    repo: String,
    calls: bool,
    tools_file: String,
    top: String,
}

struct StampJson {
    matched: bool,
    class_id: Option<&'static str>,
    policy_id: Option<&'static str>,
    family: &'static str,
    tool_name: String,
    mismatch: bool,
}

/// Positional intent and `JEV_BYPASS`. `yes` is not a bypass. Unset mode is shadow.
fn router_verdict(args: &[String]) -> (i32, String) {
    let parsed = parse_router_args(args);
    let mode = router_mode();
    let bypass = std::env::var("JEV_BYPASS")
        .map(|value| crate::policy::is_jev_bypass(&value))
        .unwrap_or(false);
    let stamp = stamp_json(&parsed.tool_name, &parsed.tool_class);
    let class_id = stamp.as_ref().and_then(|row| row.class_id).unwrap_or("");
    let facts = facts_json(&parsed.repo);
    let catalog = tiny_catalog_json();
    let policy = crate::policy::LOOP_STOP_POLICY_ID;
    let route = crate::policy::ROUTING_POLICY_ID;
    let surfaces = surface_json(&parsed, class_id, bypass);
    let pretool = stamp.as_ref().map(stamp_body).unwrap_or_default();
    if bypass {
        let digest = jstr(&parsed.step_digest);
        let intent = jstr(&parsed.intent);
        let body = format!(
            r#"{{"ok":true,"mode":"{mode}","bypass":true,"choice":"bypass","gate":"auto","blocked":false,"exec":true,"hitl":false,"autoRetry":false,"autoPromote":false,"intent":{intent},"stepDigest":{digest},"loop":{{"choice":"bypass","outcome":"continue","reason":"bypass","hitl":false,"autoRetry":false,"autoPromote":false}},"facts":{facts},"routingPolicy":"{route}","loopStopPolicy":"{policy}"{surfaces}{pretool},"catalog":{catalog}}}"#
        );
        return (0, format!("{body}\n"));
    }
    if has_api_key() {
        return (2, String::new());
    }
    if mode == "shadow" {
        let digest = jstr(&clip_digest(&parsed.step_digest));
        let intent = jstr(&parsed.intent);
        let body = format!(
            r#"{{"ok":true,"mode":"shadow","bypass":false,"missingKey":true,"choice":"unclassified","gate":"auto","blocked":false,"exec":true,"hitl":false,"autoRetry":false,"autoPromote":false,"intent":{intent},"stepDigest":{digest},"loop":{{"policy":"{policy}","selected":"unclassified","outcome":"cannot_tell","reason":"missing_key","hitl":false,"autoRetry":false,"autoPromote":false,"shadow":true,"blocked":false}},"workflow":{{"choice":"unclassified","outcome":"cannot_tell","reason":"missing_key","shadow":true,"blocked":false}},"facts":{facts},"routingPolicy":"{route}","loopStopPolicy":"{policy}"{surfaces}{pretool},"catalog":{catalog}}}"#
        );
        return (0, format!("{body}\n"));
    }
    let digest = jstr(&clip_digest(&parsed.step_digest));
    let intent = jstr(&parsed.intent);
    let body = format!(
        r#"{{"ok":false,"mode":"{mode}","missingKey":true,"choice":"escalate","gate":"hold","blocked":true,"exec":false,"hitl":true,"autoRetry":false,"autoPromote":false,"intent":{intent},"stepDigest":{digest},"loop":{{"policy":"{policy}","selected":"cannot_tell","outcome":"cannot_tell","reason":"missing_key","confidence":0,"selectedProb":0,"margin":0,"hitl":true,"autoRetry":false,"autoPromote":false}}{surfaces}{pretool},"catalog":{catalog}}}"#
    );
    (2, format!("{body}\n"))
}

fn router_mode() -> String {
    match std::env::var("JEV_MODE") {
        Ok(value) if !value.is_empty() => value.to_ascii_lowercase(),
        _ => "shadow".to_string(),
    }
}

fn has_api_key() -> bool {
    std::env::var("TYPESAFE_API_KEY")
        .map(|value| !value.is_empty())
        .unwrap_or(false)
}

fn clip_digest(text: &str) -> String {
    text.chars().take(4000).collect()
}

fn parse_router_args(args: &[String]) -> RouterArgs {
    let mut out = RouterArgs {
        intent: String::new(),
        step_digest: String::new(),
        tool_name: String::new(),
        tool_class: String::new(),
        tool_input: String::new(),
        repo: String::new(),
        calls: false,
        tools_file: String::new(),
        top: String::new(),
    };
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        let next = args.get(index + 1).map(String::as_str);
        let take = |index: &mut usize, next: Option<&str>| -> Option<String> {
            let value = next?;
            *index += 1;
            Some(value.to_string())
        };
        match arg {
            "--loop-stop" | "--catalog" | "--calls" => {
                if arg == "--calls" {
                    out.calls = true;
                }
            }
            "--intent" => {
                if let Some(value) = take(&mut index, next) {
                    out.intent = value;
                }
            }
            "--base" => {
                let _ = take(&mut index, next);
            }
            "--repo" => {
                if let Some(value) = take(&mut index, next) {
                    out.repo = value;
                }
            }
            "--diff-file" | "--diff" => {
                let _ = take(&mut index, next);
            }
            "--step-digest" | "--digest" => {
                if let Some(value) = take(&mut index, next) {
                    out.step_digest = value;
                }
            }
            "--tool-name" | "--tool" => {
                if let Some(value) = take(&mut index, next) {
                    out.tool_name = value;
                }
            }
            "--tool-class" => {
                if let Some(value) = take(&mut index, next) {
                    out.tool_class = value;
                }
            }
            "--tool-input" | "--input" => {
                if let Some(value) = take(&mut index, next) {
                    out.tool_input = value;
                }
            }
            "--tools-file" | "--tools" => {
                if let Some(value) = take(&mut index, next) {
                    out.tools_file = value;
                }
            }
            "--top" => {
                if let Some(value) = take(&mut index, next) {
                    out.top = value;
                }
            }
            "--schema" | "--schema-dump" => {
                if next.is_some_and(|value| !value.starts_with('-')) {
                    index += 1;
                }
            }
            other if !other.is_empty() && !other.starts_with('-') && out.intent.is_empty() => {
                out.intent = other.to_string();
            }
            _ => {}
        }
        index += 1;
    }
    if out.intent.is_empty() {
        out.intent = std::env::var("JEV_INTENT").unwrap_or_default();
    }
    if out.repo.is_empty() {
        out.repo = std::env::var("JEV_REPO").unwrap_or_default();
    }
    if out.step_digest.is_empty() {
        out.step_digest = std::env::var("JEV_STEP_DIGEST").unwrap_or_default();
    }
    if out.tool_name.is_empty() {
        out.tool_name = std::env::var("JEV_TOOL_NAME").unwrap_or_default();
    }
    if out.tool_class.is_empty() {
        out.tool_class = std::env::var("JEV_TOOL_CLASS").unwrap_or_default();
    }
    if out.tool_input.is_empty() {
        out.tool_input = std::env::var("JEV_TOOL_INPUT").unwrap_or_default();
    }
    if !out.calls {
        if let Ok(flag) = std::env::var("JEV_CALLS") {
            out.calls = flag == "1" || flag == "true";
        }
    }
    if out.tools_file.is_empty() {
        out.tools_file = std::env::var("JEV_TOOLS_FILE").unwrap_or_default();
    }
    if out.top.is_empty() {
        out.top = std::env::var("JEV_CALLS_TOP").unwrap_or_default();
    }
    out
}

fn facts_json(repo: &str) -> String {
    let root = if repo.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(repo)
    };
    let gathered = gather(&GatherOpts {
        repo: root,
        base: String::new(),
        diff_file: None,
        diff_text: None,
        write: None,
    });
    let present = if gathered.diff.present { "true" } else { "false" };
    let available = if gathered.diff.present {
        "\"check\",\"review\""
    } else {
        ""
    };
    format!(
        r#"{{"diffPresent":{present},"diffSource":{},"available":[{available}],"capabilities":{{"check":{present},"review":{present}}}}}"#,
        jstr(&gathered.diff.source),
    )
}

fn surface_json(args: &RouterArgs, class_id: &str, bypass: bool) -> String {
    let mut parts = String::new();
    if class_id == "mcp" || args.calls {
        parts.push_str(",\"calls\":");
        parts.push_str(&calls_json(args, bypass));
    }
    if let Some(permission) = permission_json(class_id, &args.tool_name, &args.tool_input, bypass) {
        parts.push_str(",\"permission\":");
        parts.push_str(&permission);
    }
    parts
}

fn stamp_json(tool_name: &str, tool_class: &str) -> Option<StampJson> {
    if tool_name.is_empty() && tool_class.is_empty() {
        return None;
    }
    let by_name = if tool_name.is_empty() {
        None
    } else {
        crate::policy::match_pretool_class(tool_name).and_then(|(class_id, _)| class_row(class_id))
    };
    let by_class = if tool_class.is_empty() {
        None
    } else {
        class_row(tool_class)
    };
    let mismatch = match (by_class, by_name) {
        (Some(class_row), Some(name_row)) => class_row.0 != name_row.0,
        _ => false,
    };
    let Some(row) = by_class.or(by_name) else {
        return Some(StampJson {
            matched: false,
            class_id: None,
            policy_id: None,
            family: "[]",
            tool_name: tool_name.to_string(),
            mismatch,
        });
    };
    Some(StampJson {
        matched: true,
        class_id: Some(row.0),
        policy_id: Some(row.1),
        family: row.2,
        tool_name: tool_name.to_string(),
        mismatch,
    })
}

fn class_row(id: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match id {
        "web" => Some(("web", crate::policy::LOOP_STOP_POLICY_ID, FAMILY_LOOP)),
        "subagent" => Some(("subagent", crate::policy::LOOP_STOP_POLICY_ID, FAMILY_LOOP)),
        "shell" => Some(("shell", crate::policy::LOOP_STOP_POLICY_ID, FAMILY_LOOP)),
        "write" => Some(("write", crate::policy::CHECK_POLICY_ID, FAMILY_CHECK)),
        "mcp" => Some(("mcp", crate::policy::ROUTING_POLICY_ID, FAMILY_ROUTE)),
        _ => None,
    }
}

fn stamp_body(stamp: &StampJson) -> String {
    let class = match stamp.class_id {
        Some(class_id) => format!("\"{class_id}\""),
        None => "null".to_string(),
    };
    let policy = match stamp.policy_id {
        Some(policy_id) => format!("\"{policy_id}\""),
        None => "null".to_string(),
    };
    let name = if stamp.tool_name.is_empty() {
        "null".to_string()
    } else {
        jstr(&stamp.tool_name)
    };
    format!(
        r#","pretool":{{"matched":{},"class":{class},"policyId":{policy},"choiceFamily":{},"toolName":{name},"mismatch":{}}}"#,
        jbool(stamp.matched),
        stamp.family,
        jbool(stamp.mismatch),
    )
}

fn permission_json(class_id: &str, tool_name: &str, tool_input: &str, bypass: bool) -> Option<String> {
    let surface = match class_id {
        "shell" | "write" | "mcp" => class_id,
        _ => return None,
    };
    if bypass {
        return Some(permission_bypass_json(surface));
    }
    let (path, diff) = if surface == "write" {
        write_wire(tool_input)
    } else {
        (String::new(), String::new())
    };
    let requested = match std::env::var("JEV_PERMISSION_MODE") {
        Ok(value) if value.eq_ignore_ascii_case("active") => "active",
        _ => "shadow",
    };
    let class_id = match surface {
        "shell" => "shell",
        "write" => "write",
        _ => "mcp",
    };
    let verdict = crate::permission::decide_permission(&crate::permission::PermissionInput {
        class_id,
        requested_mode: requested,
        has_key: has_api_key(),
        content_present: surface == "shell" && !tool_input.trim().is_empty(),
        label: None,
        confidence: 0.0,
        allow_p: 0.0,
        deny_p: 0.0,
        ask_p: 0.0,
        path: &path,
        diff: &diff,
        concern: None,
        flags: None,
        tool_name,
        effect: "",
        routing_outcome: None,
        typed: None,
    })?;
    Some(verdict.json)
}

fn permission_bypass_json(surface: &str) -> String {
    let (parent, catalog, new_catalog, policy) = match surface {
        "write" => ("writer", "writer", false, crate::policy::CHECK_POLICY_ID),
        "mcp" => (
            "route-workflow",
            "route-workflow",
            false,
            crate::policy::ROUTING_POLICY_ID,
        ),
        _ => ("loop-stop", "permission", true, crate::policy::LOOP_STOP_POLICY_ID),
    };
    format!(
        r#"{{"surface":"{surface}","policyId":"{policy}","parent":"{parent}","catalog":"{catalog}","newCatalog":{},"mode":"shadow","honor":false,"shadow":true,"blocked":false,"exec":true,"hitl":false,"choice":"unclassified","gate":"auto","label":null,"modelLabel":null,"mapped":null,"reason":"bypass","codeDeny":false,"skipped":true,"missingKey":false,"autoAllow":false}}"#,
        jbool(new_catalog),
    )
}

fn write_wire(raw: &str) -> (String, String) {
    if let Some(rest) = raw.strip_prefix('{') {
        if let Some(path) = json_string_field(rest, "path") {
            if let Some(proposed) = json_string_field(rest, "proposed") {
                return (path.replace(['\r', '\n'], ""), proposed);
            }
        }
    }
    (raw.to_string(), String::new())
}

fn calls_json(args: &RouterArgs, bypass: bool) -> String {
    let top = clamp_top(&args.top);
    if bypass {
        return calls_pack("bypass", false, true, "null", "null", top);
    }
    if args.tools_file.is_empty() {
        return calls_pack("no_catalog", false, false, "null", "null", top);
    }
    let text = match std::fs::read_to_string(&args.tools_file) {
        Ok(text) => text,
        Err(_) => return calls_pack("catalog_unreadable", false, false, "null", "null", top),
    };
    let trimmed = text.trim();
    if !trimmed.starts_with('{') {
        let reason = if trimmed.starts_with('[') || trimmed == "null" || trimmed.parse::<f64>().is_ok() {
            "catalog_invalid"
        } else {
            "catalog_unreadable"
        };
        return calls_pack(reason, false, false, "null", "null", top);
    }
    let Some(tools) = catalog_tools(trimmed) else {
        return calls_pack("catalog_unreadable", false, false, "null", "null", top);
    };
    if tools.is_empty() {
        return calls_pack("no_catalog", false, false, "null", "null", top);
    }
    let interface = catalog_interface(trimmed);
    let mut events = Vec::new();
    let mut canonical = Vec::new();
    let mut seq = 1u32;
    for tool in tools {
        if !facet_ident(&tool.name) {
            continue;
        }
        let allow = effect_allows(&tool.effect);
        let effect_class = if known_effect(&tool.effect) {
            jstr(&tool.effect)
        } else {
            "null".to_string()
        };
        let decision = if allow { "allowed" } else { "denied" };
        let qualified = format!("{interface}.{}", tool.name);
        events.push(format!(
            r#"{{"seq":{seq},"op":"tool_expose","name":{},"effect_class":{effect_class},"mode":"pure","decision":"{decision}","policy_rule_id":null,"input_hash":"sha256:0"}}"#,
            jstr(&qualified),
        ));
        seq += 1;
        if !allow {
            continue;
        }
        canonical.push(format!(
            r#"{{"name":{},"description":{},"effect":{},"parameters":{{"type":"object","properties":{{}},"required":[]}}}}"#,
            jstr(&qualified),
            jstr(&tool.description),
            jstr(&tool.effect),
        ));
    }
    let canonical_json = format!(
        r#"{{"metadata":{{"facet_version":"2.1.3","profile":"hypervisor","mode":"pure","host_profile_id":"omapi-jev-router","document_hash":"sha256:0","policy_hash":"sha256:0","policy_version":"1","budget_units":0,"target_provider_id":"omapi-jev-router"}},"tools":[{}],"messages":[]}}"#,
        canonical.join(","),
    );
    let artifact = format!(
        r#"{{"metadata":{{"facet_version":"2.1.3","host_profile_id":"omapi-jev-router","document_hash":"sha256:0","policy_hash":"sha256:0","policy_version":"1"}},"provenance":{{"events":[{}],"hash_chain":{{"algo":"sha256","head":"sha256:0"}}}},"attestation":null}}"#,
        events.join(","),
    );
    calls_pack("missing_key", true, false, &canonical_json, &artifact, top)
}

struct CatalogTool {
    name: String,
    description: String,
    effect: String,
}

fn calls_pack(reason: &str, missing_key: bool, skipped: bool, canonical: &str, artifact: &str, top: i64) -> String {
    let (label, mapped) = if reason == "bypass" {
        ("null".to_string(), "null".to_string())
    } else {
        ("\"ask\"".to_string(), "\"escalate\"".to_string())
    };
    format!(
        r#"{{"transport":"facet","facetVersion":"2.1.3","policyId":"{}","policyVersion":"1","mode":"shadow","honor":false,"shadow":true,"choice":"unclassified","gate":"auto","blocked":false,"exec":true,"hitl":false,"label":{label},"mapped":{mapped},"reason":{},"code":null,"codeDeny":false,"skipped":{},"missingKey":{},"autoAllow":false,"autoPromote":false,"initiated":false,"topX":{top},"selected":null,"best":null,"top":[],"facet":null,"canonical":{canonical},"artifact":{artifact}}}"#,
        crate::policy::ROUTING_POLICY_ID,
        jstr(reason),
        jbool(skipped),
        jbool(missing_key),
    )
}

fn clamp_top(value: &str) -> i64 {
    if value.is_empty() {
        return 1;
    }
    let Ok(number) = value.parse::<f64>() else {
        return 3;
    };
    if !number.is_finite() || number.fract() != 0.0 {
        return 3;
    }
    let number = number as i64;
    if number < 1 {
        1
    } else if number > 8 {
        8
    } else {
        number
    }
}

fn catalog_interface(text: &str) -> String {
    json_string_field(text, "interface")
        .filter(|name| facet_ident(name))
        .unwrap_or_else(|| "Mcp".to_string())
}

fn catalog_tools(text: &str) -> Option<Vec<CatalogTool>> {
    let bytes = text.as_bytes();
    let key = b"\"tools\"";
    let start = text.find("\"tools\"")?;
    let mut index = start + key.len();
    index = skip_ws(bytes, index)?;
    if bytes.get(index) != Some(&b':') {
        return None;
    }
    index += 1;
    index = skip_ws(bytes, index)?;
    if bytes.get(index) != Some(&b'[') {
        return None;
    }
    index += 1;
    let mut tools = Vec::new();
    loop {
        index = skip_ws(bytes, index)?;
        match bytes.get(index) {
            Some(b']') => return Some(tools),
            Some(b',') => index += 1,
            Some(b'{') => {
                let (object, next) = json_object(text, index)?;
                if tools.len() < 64 {
                    if let Some(tool) = tool_row(&object) {
                        if !tools.iter().any(|row: &CatalogTool| row.name == tool.name) {
                            tools.push(tool);
                        }
                    }
                }
                index = next;
            }
            _ => return None,
        }
    }
}

fn tool_row(object: &str) -> Option<CatalogTool> {
    let name = json_string_field(object, "name")?;
    if name.is_empty() || name == "cannot_tell" {
        return None;
    }
    let description = json_string_field(object, "description").unwrap_or_else(|| name.clone());
    let description: String = description.chars().take(240).collect();
    let effect = json_string_field(object, "effect").unwrap_or_default();
    Some(CatalogTool {
        name,
        description,
        effect,
    })
}

fn json_string_field(text: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let mut from = 0;
    while let Some(found) = text[from..].find(&pattern) {
        let mut index = from + found + pattern.len();
        let bytes = text.as_bytes();
        index = skip_ws(bytes, index)?;
        if bytes.get(index) != Some(&b':') {
            from += found + pattern.len();
            continue;
        }
        index += 1;
        index = skip_ws(bytes, index)?;
        if bytes.get(index) != Some(&b'"') {
            from += found + pattern.len();
            continue;
        }
        let (value, _) = read_json_string(text, index)?;
        return Some(value);
    }
    None
}

fn json_object(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return None;
    }
    let mut depth = 0i32;
    let mut index = start;
    let mut in_string = false;
    let mut escape = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if byte == b'"' {
            in_string = true;
        } else if byte == b'{' {
            depth += 1;
        } else if byte == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some((text[start..=index].to_string(), index + 1));
            }
        }
        index += 1;
    }
    None
}

fn read_json_string(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut index = start + 1;
    let mut raw = Vec::new();
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => {
                index += 1;
                let escaped = *bytes.get(index)?;
                match escaped {
                    b'n' => raw.push(b'\n'),
                    b'r' => raw.push(b'\r'),
                    b't' => raw.push(b'\t'),
                    b'u' => {
                        let hex = std::str::from_utf8(bytes.get(index + 1..index + 5)?).ok()?;
                        let code = u32::from_str_radix(hex, 16).ok()?;
                        let ch = char::from_u32(code).unwrap_or('\u{FFFD}');
                        let mut buf = [0u8; 4];
                        let encoded = ch.encode_utf8(&mut buf);
                        raw.extend(encoded.as_bytes());
                        index += 4;
                    }
                    other => raw.push(other),
                }
            }
            b'"' => {
                let value = String::from_utf8(raw).ok()?;
                return Some((value, index + 1));
            }
            other => raw.push(other),
        }
        index += 1;
    }
    None
}

fn skip_ws(bytes: &[u8], mut index: usize) -> Option<usize> {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index < bytes.len() {
        Some(index)
    } else {
        None
    }
}

fn facet_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {
            chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        }
        _ => false,
    }
}

fn known_effect(effect: &str) -> bool {
    matches!(
        effect,
        "read" | "write" | "external" | "payment" | "filesystem" | "network"
    )
}

fn effect_allows(effect: &str) -> bool {
    matches!(effect, "read" | "write" | "filesystem" | "network" | "external")
}

fn jbool(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn jstr(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
