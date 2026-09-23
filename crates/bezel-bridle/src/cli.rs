//! Offline argv. This is the only module that may construct a client.
//! The live command is `facet request run`. It stays off for this gate.

use crate::catalog::{schema_dump_json, tiny_catalog_json};
use crate::check::{offline_check_json, run_offline_check};
use crate::facts::{gather, GatherOpts};
use crate::transport::{recorded_dir, FixtureTransport};
use std::path::{Path, PathBuf};

pub const LIVE_COMMAND: &str = "facet request run --environment typesafe --no-record";
pub const LIVE_ON: bool = false;

pub struct Client {
    fixture: FixtureTransport,
}

/// Fixture client only. Does not shell out, and does not link a TypeSafe SDK.
pub fn construct_client(dir: impl Into<PathBuf>) -> Client {
    debug_assert!(!LIVE_ON);
    let _live = LIVE_COMMAND;
    Client {
        fixture: FixtureTransport::open(dir),
    }
}

impl Client {
    pub fn system_one(&self, name: &str) -> Result<Vec<u8>, String> {
        if LIVE_ON {
            return Err(format!("live transport is off; refusing {LIVE_COMMAND}"));
        }
        self.fixture.system_one(name)
    }
}

pub fn fixture_client() -> Client {
    construct_client(recorded_dir())
}

pub fn run(args: &[String]) -> (i32, String) {
    if args.first().map(String::as_str) == Some("--version") && args.len() == 1 {
        return (0, format!("bezel-bridle {}\n", env!("CARGO_PKG_VERSION")));
    }
    if args.first().map(String::as_str) == Some("hook") {
        return hook_verdict();
    }
    if args.iter().any(|arg| arg == "--catalog") {
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
    (2, String::new())
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn version_matches_package() {
        let (code, out) = run(&["--version".into()]);
        assert_eq!(code, 0);
        assert_eq!(out, format!("bezel-bridle {}\n", env!("CARGO_PKG_VERSION")));
    }
}

/// `jev-router hook` with no key. Same bytes as the JavaScript shadow verdict.
fn hook_verdict() -> (i32, String) {
    let active = std::env::var("JEV_MODE")
        .map(|value| value.eq_ignore_ascii_case("active"))
        .unwrap_or(false);
    let repo = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let gathered = gather(&GatherOpts {
        repo,
        base: String::new(),
        diff_file: None,
        diff_text: None,
        write: None,
    });
    let present = if gathered.diff.present { "true" } else { "false" };
    let caps = present;
    let available = if gathered.diff.present {
        "\"check\",\"review\""
    } else {
        ""
    };
    let catalog = tiny_catalog_json();
    let policy = crate::policy::LOOP_STOP_POLICY_ID;
    let route = crate::policy::ROUTING_POLICY_ID;
    let facts = format!(
        r#"{{"diffPresent":{present},"diffSource":"{}","available":[{available}],"capabilities":{{"check":{caps},"review":{caps}}}}}"#,
        gathered.diff.source,
    );
    if !active {
        let body = format!(
            r#"{{"ok":true,"mode":"shadow","bypass":false,"missingKey":true,"choice":"unclassified","gate":"auto","blocked":false,"exec":true,"hitl":false,"autoRetry":false,"autoPromote":false,"intent":"hook","stepDigest":"","loop":{{"policy":"{policy}","selected":"unclassified","outcome":"cannot_tell","reason":"missing_key","hitl":false,"autoRetry":false,"autoPromote":false,"shadow":true,"blocked":false}},"workflow":{{"choice":"unclassified","outcome":"cannot_tell","reason":"missing_key","shadow":true,"blocked":false}},"facts":{facts},"routingPolicy":"{route}","loopStopPolicy":"{policy}","catalog":{catalog}}}"#
        );
        return (0, format!("{body}\n"));
    }
    let _facts = facts;
    let body = format!(
        r#"{{"ok":false,"mode":"active","missingKey":true,"choice":"escalate","gate":"hold","blocked":true,"exec":false,"hitl":true,"autoRetry":false,"autoPromote":false,"intent":"hook","stepDigest":"","loop":{{"policy":"{policy}","selected":"cannot_tell","outcome":"cannot_tell","reason":"missing_key","confidence":0,"selectedProb":0,"margin":0,"hitl":true,"autoRetry":false,"autoPromote":false}},"catalog":{catalog}}}"#
    );
    let _route = route;
    (2, format!("{body}\n"))
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
