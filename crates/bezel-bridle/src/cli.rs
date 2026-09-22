//! Offline argv. `--catalog`, `--schema`, and `--check` do not construct a client.

use crate::catalog::{schema_dump_json, tiny_catalog_json};
use crate::check::{offline_check_json, run_offline_check};
use std::path::Path;

pub fn run(args: &[String]) -> (i32, String) {
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

fn diff_argument(args: &[String]) -> String {
    let Some(index) = args.iter().position(|arg| arg == "--diff-file") else {
        return String::new();
    };
    let Some(path) = args.get(index + 1) else {
        return String::new();
    };
    std::fs::read_to_string(Path::new(path)).unwrap_or_default()
}
