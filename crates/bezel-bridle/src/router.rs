//! No-key router JSON for the Bash fixture. Matches `jev-router.mjs`.
//! Active missing-key is escalate / hold on the loop-stop policy, not `jev uncertain`.

use crate::catalog::tiny_catalog_json;
use crate::facts::{gather, GatherOpts};
use crate::policy::LOOP_STOP_POLICY_ID;
use std::path::Path;

const PERMISSION: &str = r#"{"surface":"shell","policyId":"omapi-loop-stop-policy@1","parent":"loop-stop","catalog":"permission","newCatalog":true,"mode":"shadow","honor":false,"shadow":true,"blocked":false,"exec":true,"hitl":false,"choice":"unclassified","gate":"auto","label":null,"modelLabel":null,"mapped":null,"reason":"missing_key","codeDeny":false,"skipped":false,"missingKey":true,"autoAllow":false}"#;

const PRETOOL: &str = r#"{"matched":true,"class":"shell","policyId":"omapi-loop-stop-policy@1","choiceFamily":["continue","stop","escalate"],"toolName":"Bash","mismatch":false}"#;

/// Active or shadow, no API key, Bash fixture (`--tool-name Bash --intent Bash`).
pub fn bash_no_key(mode: &str, repo: &Path) -> String {
    let catalog = tiny_catalog_json();
    let body = if mode.eq_ignore_ascii_case("active") {
        format!(
            r#"{{"ok":false,"mode":"active","missingKey":true,"choice":"escalate","gate":"hold","blocked":true,"exec":false,"hitl":true,"autoRetry":false,"autoPromote":false,"intent":"Bash","stepDigest":"","loop":{{"policy":"{policy}","selected":"cannot_tell","outcome":"cannot_tell","reason":"missing_key","confidence":0,"selectedProb":0,"margin":0,"hitl":true,"autoRetry":false,"autoPromote":false}},"permission":{PERMISSION},"pretool":{PRETOOL},"catalog":{catalog}}}"#,
            policy = LOOP_STOP_POLICY_ID,
        )
    } else {
        let gathered = gather(&GatherOpts {
            repo: repo.to_path_buf(),
            ..GatherOpts::default()
        });
        let present = if gathered.diff.present { "true" } else { "false" };
        let check = if gathered.diff.present { "true" } else { "false" };
        let available = format!(
            "[{}]",
            gathered
                .available
                .iter()
                .map(|id| format!("\"{id}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        format!(
            r#"{{"ok":true,"mode":"shadow","bypass":false,"missingKey":true,"choice":"unclassified","gate":"auto","blocked":false,"exec":true,"hitl":false,"autoRetry":false,"autoPromote":false,"intent":"Bash","stepDigest":"","loop":{{"policy":"{policy}","selected":"unclassified","outcome":"cannot_tell","reason":"missing_key","hitl":false,"autoRetry":false,"autoPromote":false,"shadow":true,"blocked":false}},"workflow":{{"choice":"unclassified","outcome":"cannot_tell","reason":"missing_key","shadow":true,"blocked":false}},"facts":{{"diffPresent":{present},"diffSource":"{source}","available":{available},"capabilities":{{"check":{check},"review":{check}}}}},"routingPolicy":"omapi-route-workflow-policy@1","loopStopPolicy":"{policy}","permission":{PERMISSION},"pretool":{PRETOOL},"catalog":{catalog}}}"#,
            policy = LOOP_STOP_POLICY_ID,
            source = gathered.diff.source,
        )
    };
    format!("{body}\n")
}
