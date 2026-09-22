//! Tiny tool index and on-demand schema dump.
//! No client. A schema dump defers or denies. It does not allow.

use crate::policy::{match_pretool_class, CHECK_POLICY_ID, LOOP_STOP_POLICY_ID, ROUTING_POLICY_ID};

const MCP_PATTERN: &str = "[A-Za-z0-9][A-Za-z0-9_.-]*__[A-Za-z0-9_.-]+";

pub fn tiny_catalog_json() -> String {
    format!(
        r#"{{"kind":"tiny","entries":[{{"class":"web","policyId":"{loop_id}","tools":["web_search","WebSearch","web_fetch","WebFetch"]}},{{"class":"subagent","policyId":"{loop_id}","tools":["spawn_subagent","Task"]}},{{"class":"shell","policyId":"{loop_id}","tools":["Bash","run_terminal_command","run_terminal_cmd"]}},{{"class":"write","policyId":"{check_id}","tools":["Write","Edit","MultiEdit","search_replace"]}},{{"class":"mcp","policyId":"{route_id}","pattern":"{pattern}"}}]}}"#,
        loop_id = LOOP_STOP_POLICY_ID,
        check_id = CHECK_POLICY_ID,
        route_id = ROUTING_POLICY_ID,
        pattern = MCP_PATTERN,
    )
}

pub fn schema_dump_json(tool_name: &str) -> String {
    let name = tool_name;
    let Some((class_id, policy_id)) = match_pretool_class(name) else {
        let tool = if name.is_empty() {
            "null".to_string()
        } else {
            format!("\"{name}\"")
        };
        return format!(
            r#"{{"ok":false,"call":"schema-dump","tool":{tool},"found":false,"class":null,"policyId":null,"schema":null,"autoAllow":false,"decision":"deny","reason":"unknown_tool"}}"#
        );
    };
    let Some(schema) = schema_json(name, class_id) else {
        return format!(
            r#"{{"ok":false,"call":"schema-dump","tool":"{name}","found":false,"class":"{class_id}","policyId":"{policy_id}","schema":null,"autoAllow":false,"decision":"deny","reason":"schema_missing"}}"#
        );
    };
    let typed = if class_id == "mcp" {
        r#","typedCall":false"#
    } else {
        ""
    };
    format!(
        r#"{{"ok":true,"call":"schema-dump","tool":"{name}","found":true,"class":"{class_id}","policyId":"{policy_id}","schema":{schema},"autoAllow":false,"decision":"defer","reason":"schema_dump"{typed}}}"#
    )
}

fn schema_json(tool_name: &str, class_id: &str) -> Option<String> {
    let key = if class_id == "mcp" {
        "mcp"
    } else {
        match tool_name {
            "web_search" | "WebSearch" => "web_search",
            "web_fetch" | "WebFetch" => "web_fetch",
            "spawn_subagent" | "Task" => "task",
            "Bash" | "run_terminal_command" | "run_terminal_cmd" => "shell",
            "Write" => "write",
            "Edit" | "search_replace" => "edit",
            "MultiEdit" => "multi_edit",
            _ => return None,
        }
    };
    let body = match key {
        "web_search" => {
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["query"],"properties":{"query":{"type":"string","description":"Search query."}}}"#
        }
        "web_fetch" => {
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["url"],"properties":{"url":{"type":"string","description":"Absolute URL to fetch."}}}"#
        }
        "task" => {
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["prompt"],"properties":{"prompt":{"type":"string","description":"Task for the subagent."},"description":{"type":"string","description":"Short label for the task."},"subagent_type":{"type":"string","description":"Subagent kind, when the host has one."}}}"#
        }
        "shell" => {
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["command"],"properties":{"command":{"type":"string","description":"Shell command text. Reading this schema does not allow the command."}}}"#
        }
        "write" => {
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["path","contents"],"properties":{"path":{"type":"string","description":"File path to write."},"contents":{"type":"string","description":"Full file contents."}}}"#
        }
        "edit" => {
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["path","old_string","new_string"],"properties":{"path":{"type":"string","description":"File path to edit."},"old_string":{"type":"string","description":"Exact text to replace."},"new_string":{"type":"string","description":"Replacement text."}}}"#
        }
        "multi_edit" => {
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["path","edits"],"properties":{"path":{"type":"string","description":"File path to edit."},"edits":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["old_string","new_string"],"properties":{"old_string":{"type":"string"},"new_string":{"type":"string"}}}}}}"#
        }
        "mcp" => {
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["server","tool"],"properties":{"server":{"type":"string","description":"MCP server id from the qualified tool name."},"tool":{"type":"string","description":"Tool name on that server."},"arguments":{"type":"object","description":"Tool arguments. This dump does not fill them or allow the call."}}}"#
        }
        _ => return None,
    };
    let (head, tail) = body.split_once('{')?;
    Some(format!(r#"{head}{{"title":"{tool_name}",{tail}"#))
}
