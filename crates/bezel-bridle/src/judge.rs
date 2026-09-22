//! The three `systemOne` sites. Each one replays a recorded Node answer.
//! This module does not construct a client and does not open a network call.

use crate::cli::Client;

pub const CHECK_JUDGE: &str = "check-judge";
pub const SHADOW_WORKFLOW: &str = "shadow-workflow";
pub const MAIN_GATE: &str = "main-gate";

const SITES: &[(&str, &str)] = &[
    (CHECK_JUDGE, "check-judge.json"),
    (SHADOW_WORKFLOW, "shadow-workflow.json"),
    (MAIN_GATE, "main-gate.json"),
];

pub fn fixture_name(site: &str) -> Option<&'static str> {
    SITES.iter().find(|(id, _)| *id == site).map(|(_, name)| *name)
}

/// Replay the recorded body for one site. A missing file is a mismatch.
pub fn recorded_answer(client: &Client, site: &str) -> Result<Vec<u8>, String> {
    let name = fixture_name(site).ok_or_else(|| format!("unknown systemOne site {site}"))?;
    client.system_one(name)
}

/// True only when `body` is the recorded Node answer, byte for byte.
pub fn matches_recorded(body: &[u8], recorded: &[u8]) -> bool {
    body == recorded
}
