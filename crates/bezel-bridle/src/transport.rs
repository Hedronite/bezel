//! Fixture transport. Replays recorded `systemOne` bytes. Does not construct a client.

use std::path::{Path, PathBuf};

pub struct FixtureTransport {
    dir: PathBuf,
}

impl FixtureTransport {
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Exact file bytes. No trim, no re-encoding. A missing record is a mismatch.
    pub fn system_one(&self, name: &str) -> Result<Vec<u8>, String> {
        let path = self.dir.join(name);
        std::fs::read(&path).map_err(|err| format!("systemOne mismatch {}: {err}", path.display()))
    }
}

pub fn recorded_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/jev-router/testdata/system-one")
}
