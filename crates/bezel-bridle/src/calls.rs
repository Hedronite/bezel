//! Facet envelope. Built here, not sent. `initiated` stays false.

pub const TRANSPORT: &str = "facet";
pub const INITIATED: bool = false;
pub const FACET_VERSION: &str = "2.1.3";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    pub transport: &'static str,
    pub initiated: bool,
    pub op: &'static str,
    pub facet_version: &'static str,
}

pub fn tool_call() -> ToolCall {
    ToolCall {
        transport: TRANSPORT,
        initiated: INITIATED,
        op: "tool_call",
        facet_version: FACET_VERSION,
    }
}

const POLICY_VERSION: &str = "1";
const HOST_PROFILE_ID: &str = "omapi-jev-router";
const FACET_PROFILE: &str = "hypervisor";
const FACET_MODE: &str = "pure";
const POLICY_ID: &str = "omapi-route-workflow-policy@1";
const POLICY_JCS: &str = r#"{"policy":{"tool_call":{"allow":["read","write","filesystem","network","external"],"deny":["payment"]},"tool_expose":{"allow":["read","write","filesystem","network","external"]}},"policy_version":"1"}"#;

/// Shared fixture: one read tool, no key. Shadow, not allow.
/// The JSON matches `decideTypedCalls` in `calls.mjs`.
pub fn missing_key_envelope() -> String {
    let document = "interface Mcp\nfn list_issues effect=read";
    let document_hash = sha256_prefixed(document);
    let policy_hash = sha256_prefixed(POLICY_JCS);
    let input_hash = sha256_prefixed(
        r#"{"facet_version":"2.1.3","host_profile_id":"omapi-jev-router","interface":"Mcp"}"#,
    );
    let head_seed = format!(
        r#"{{"document_hash":"{document_hash}","facet_version":"2.1.3","host_profile_id":"{HOST_PROFILE_ID}","mode":"{FACET_MODE}","policy_hash":"{policy_hash}","policy_version":"{POLICY_VERSION}","profile":"{FACET_PROFILE}"}}"#
    );
    let mut hex = sha256_hex(&head_seed);
    let event = format!(
        r#"{{"decision":"allowed","effect_class":"read","input_hash":"{input_hash}","mode":"pure","name":"Mcp.list_issues","op":"tool_expose","policy_rule_id":null,"seq":1}}"#
    );
    let link = format!(r#"{{"event":{event},"prev":"sha256:{hex}"}}"#);
    hex = sha256_hex(&link);
    format!(
        r#"{{"transport":"facet","facetVersion":"{FACET_VERSION}","policyId":"{POLICY_ID}","policyVersion":"{POLICY_VERSION}","mode":"shadow","honor":false,"shadow":true,"choice":"unclassified","gate":"auto","blocked":false,"exec":true,"hitl":false,"label":"ask","mapped":"escalate","reason":"missing_key","code":null,"codeDeny":false,"skipped":false,"missingKey":true,"autoAllow":false,"autoPromote":false,"initiated":false,"topX":3,"selected":null,"best":null,"top":[],"facet":null,"canonical":{{"metadata":{{"facet_version":"2.1.3","profile":"hypervisor","mode":"pure","host_profile_id":"{HOST_PROFILE_ID}","document_hash":"{document_hash}","policy_hash":"{policy_hash}","policy_version":"1","budget_units":0,"target_provider_id":"{HOST_PROFILE_ID}"}},"tools":[{{"name":"Mcp.list_issues","description":"List issues","effect":"read","parameters":{{"type":"object","properties":{{}},"required":[]}}}}],"messages":[]}},"artifact":{{"metadata":{{"facet_version":"2.1.3","host_profile_id":"{HOST_PROFILE_ID}","document_hash":"{document_hash}","policy_hash":"{policy_hash}","policy_version":"1"}},"provenance":{{"events":[{{"seq":1,"op":"tool_expose","name":"Mcp.list_issues","effect_class":"read","mode":"pure","decision":"allowed","policy_rule_id":null,"input_hash":"{input_hash}"}}],"hash_chain":{{"algo":"sha256","head":"sha256:{hex}"}}}},"attestation":null}}}}"#
    )
}

pub fn envelope_matches(body: &str, recorded: &str) -> bool {
    body.as_bytes() == recorded.as_bytes()
}

#[derive(Clone, Copy)]
pub struct HardStopTool {
    pub name: &'static str,
    pub description: &'static str,
    pub effect: &'static str,
}

/// Shipped decision for one catalog pick. Matches `decideTypedCalls` on the hard-stop fixtures.
/// `payment` stops. Obvious effects are not stamped `F454`.
pub fn hard_stop_json(winner: &str, tools: &[HardStopTool], probabilities: &[(&str, f64)]) -> String {
    let tools: Vec<Tool> = tools
        .iter()
        .map(|tool| Tool {
            name: tool.name,
            description: tool.description,
            effect: tool.effect,
            valid_fn: facet_ident(tool.name),
        })
        .collect();
    decide_json("active", &tools, winner, 0.9, probabilities)
}

/// Obvious closed-arg call. `mode` is `active` or `shadow`.
pub fn obvious_effect_json(effect: &'static str, mode: &'static str) -> String {
    let tools = [Tool {
        name: "act__tool",
        description: "Act",
        effect,
        valid_fn: true,
    }];
    decide_json(mode, &tools, "act__tool", 0.9, &[("act__tool", 0.8)])
}

/// Low confidence returns to the agent and names each failed bar.
pub fn model_uncertain_json() -> String {
    let tools = [
        Tool {
            name: "linear__list_issues",
            description: "List issues",
            effect: "read",
            valid_fn: true,
        },
        Tool {
            name: "notes__add",
            description: "Add a note",
            effect: "read",
            valid_fn: true,
        },
    ];
    decide_json(
        "active",
        &tools,
        "linear__list_issues",
        0.42,
        &[
            ("linear__list_issues", 0.4),
            ("notes__add", 0.35),
            ("cannot_tell", 0.25),
        ],
    )
}

/// A cannot_tell choice escalates to a human with detail= and question=.
pub fn cannot_tell_json() -> String {
    let tools = [Tool {
        name: "linear__list_issues",
        description: "List issues",
        effect: "read",
        valid_fn: true,
    }];
    decide_json(
        "active",
        &tools,
        "cannot_tell",
        0.9,
        &[("cannot_tell", 0.9), ("linear__list_issues", 0.1)],
    )
}

fn decide_json(mode: &str, tools: &[Tool], winner: &str, confidence: f64, probabilities: &[(&str, f64)]) -> String {
    let decision = route(winner, confidence, probabilities, tools);
    let top = rank_top(probabilities, tools);
    let picked = tools.iter().find(|tool| tool.name == winner);
    let mut code: Option<&str> = None;
    let mut reason = decision.reason;
    let mut label = "ask";
    let mut mapped = "ask";
    let mut code_deny = false;
    let mut question: Option<String> = None;
    let mut hold: Option<&str> = None;
    let mut best = "null".to_string();
    let mut selected_tail = "";
    let mut call_event = false;
    let mut call_decision = "denied";
    let mut facet_effect: Option<&str> = None;
    let mut call_tool: Option<&Tool> = None;

    if decision.reason == "model_uncertain" {
        question = Some(agent_question(decision.confidence, decision.selected_prob, decision.margin));
    } else if decision.reason == "cannot_tell" {
        label = "ask";
        mapped = "escalate";
        hold = Some("detail=cannot_tell question=Which tool should a human choose?");
    } else if decision.reason == "unavailable" {
        reason = "unknown_tool";
        label = "deny";
        mapped = "stop";
        code_deny = true;
    } else if picked.is_some_and(|tool| !tool.valid_fn) {
        reason = "invalid_fn";
        code = Some("F452");
        label = "deny";
        mapped = "stop";
        code_deny = true;
    } else if let Some(tool) = picked {
        match guard_kind(tool.effect) {
            Guard::Missing => {
                reason = "effect_missing";
                code = Some("F456");
                label = "deny";
                mapped = "stop";
                code_deny = true;
                call_event = true;
                call_tool = Some(tool);
            }
            Guard::Invalid => {
                reason = "effect_invalid";
                code = Some("F456");
                label = "deny";
                mapped = "stop";
                code_deny = true;
                call_event = true;
                call_tool = Some(tool);
            }
            Guard::Payment | Guard::Other => {
                reason = "effect_deny";
                code = Some("F454");
                label = "deny";
                mapped = "stop";
                code_deny = true;
                call_event = true;
                call_decision = "denied";
                facet_effect = if tool.effect.is_empty() { None } else { Some(tool.effect) };
                call_tool = Some(tool);
            }
            Guard::Allow => {
                reason = "selected";
                label = "allow";
                mapped = "continue";
                call_event = true;
                call_decision = "allowed";
                facet_effect = Some(tool.effect);
                call_tool = Some(tool);
                selected_tail = r#","weakest":null"#;
                let rank = top.iter().find(|row| row.name == tool.name).map(|row| row.rank).unwrap_or(1);
                best = format!(
                    r#"{{"name":"{}","rank":{rank},"probability":{},"confidence":{},"effect":"{}","args":{{}},"weakest":null,"initiated":false}}"#,
                    tool.name,
                    jnum(decision.selected_prob),
                    jnum(decision.confidence),
                    tool.effect,
                );
            }
        }
    }
    let document = catalog_source(&tools);
    let document_hash = sha256_prefixed(&document);
    let policy_hash = sha256_prefixed(POLICY_JCS);
    let expose_hash = sha256_prefixed(
        r#"{"facet_version":"2.1.3","host_profile_id":"omapi-jev-router","interface":"Mcp"}"#,
    );
    let meta = Meta {
        document_hash: document_hash.clone(),
        policy_hash: policy_hash.clone(),
    };
    let mut events = Vec::new();
    let mut seq = 1u32;
    let mut canonical_tools = Vec::new();
    for tool in tools {
        if !tool.valid_fn {
            continue;
        }
        let effect_class = effect_class(tool.effect);
        let allowed = effect_allows(tool.effect);
        events.push(expose_event(seq, tool, effect_class, allowed, &expose_hash));
        seq += 1;
        if allowed {
            canonical_tools.push(format!(
                r#"{{"name":"Mcp.{}","description":"{}","effect":"{}","parameters":{{"type":"object","properties":{{}},"required":[]}}}}"#,
                tool.name, tool.description, tool.effect
            ));
        }
    }
    let mut facet = "null".to_string();
    if call_event {
        if let Some(tool) = call_tool {
            let input_hash = sha256_prefixed(&format!(
                r#"{{"args":{{}},"facet_version":"2.1.3","fn":"{}","host_profile_id":"omapi-jev-router","interface":"Mcp"}}"#,
                tool.name
            ));
            let event = call_event_json(seq, tool, facet_effect, call_decision, &input_hash);
            events.push(event.clone());
            facet = format!(
                r#"{{"op":"tool_call","name":"Mcp.{}","interface":"Mcp","fn":"{}","effect_class":{},"mode":"pure","profile":"hypervisor","args":{{}},"initiated":false,"guard":{event}}}"#,
                tool.name,
                tool.name,
                json_effect(facet_effect),
            );
        }
    }
    let head = facet_head(&meta, &events);
    let selected = format!(
        r#"{{"name":"{name}","outcome":"{outcome}","reason":"{reason}","confidence":{confidence},"selectedProb":{prob},"margin":{margin},"policy":"{policy}"{tail}}}"#,
        name = decision.name,
        outcome = decision.outcome,
        reason = decision.reason,
        confidence = jnum(decision.confidence),
        prob = jnum(decision.selected_prob),
        margin = jnum(decision.margin),
        policy = POLICY_ID,
        tail = selected_tail,
    );
    let top_json = top
        .iter()
        .map(|row| {
            format!(
                r#"{{"rank":{},"name":"{}","probability":{},"effect":{},"effectGuard":"{}"}}"#,
                row.rank,
                row.name,
                jnum(row.probability),
                json_effect(row.effect),
                row.guard,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let code_json = code.map(|code| format!("\"{code}\"")).unwrap_or_else(|| "null".to_string());
    let honor = mode == "active";
    let (choice, gate, blocked, exec, hitl) = bits(honor, mapped);
    let middle = if let Some(text) = question.as_deref() {
        format!(r#""question":"{text}","#)
    } else if let Some(text) = hold {
        format!(r#""hold":"{text}","#)
    } else {
        String::new()
    };
    format!(
        r#"{{"transport":"facet","facetVersion":"2.1.3","policyId":"{POLICY_ID}","policyVersion":"1","mode":"{mode}","honor":{honor},"shadow":{shadow},"choice":"{choice}","gate":"{gate}","blocked":{blocked},"exec":{exec},"hitl":{hitl},"label":"{label}","mapped":"{mapped}","reason":"{reason}","code":{code_json},"codeDeny":{code_deny},"skipped":false,"missingKey":false,"autoAllow":false,"autoPromote":false,"initiated":false,{middle}"topX":3,"selected":{selected},"best":{best},"top":[{top_json}],"facet":{facet},"canonical":{{"metadata":{{"facet_version":"2.1.3","profile":"hypervisor","mode":"pure","host_profile_id":"omapi-jev-router","document_hash":"{document_hash}","policy_hash":"{policy_hash}","policy_version":"1","budget_units":0,"target_provider_id":"omapi-jev-router"}},"tools":[{canonical}],"messages":[]}},"artifact":{{"metadata":{{"facet_version":"2.1.3","host_profile_id":"omapi-jev-router","document_hash":"{document_hash}","policy_hash":"{policy_hash}","policy_version":"1"}},"provenance":{{"events":[{events}],"hash_chain":{{"algo":"sha256","head":"{head}"}}}},"attestation":null}}}}"#,
        honor = jbool(honor),
        shadow = jbool(!honor),
        blocked = jbool(blocked),
        exec = jbool(exec),
        hitl = jbool(hitl),
        code_deny = jbool(code_deny),
        document_hash = meta.document_hash,
        policy_hash = meta.policy_hash,
        canonical = canonical_tools.join(","),
        events = events.join(","),
    )
}

struct Tool {
    name: &'static str,
    description: &'static str,
    effect: &'static str,
    valid_fn: bool,
}

struct Meta {
    document_hash: String,
    policy_hash: String,
}

struct Route {
    name: String,
    outcome: String,
    reason: &'static str,
    confidence: f64,
    selected_prob: f64,
    margin: f64,
}

struct TopRow {
    rank: usize,
    name: String,
    probability: f64,
    effect: Option<&'static str>,
    guard: &'static str,
}

enum Guard {
    Allow,
    Missing,
    Invalid,
    Payment,
    Other,
}

fn facet_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn known_effect(effect: &str) -> bool {
    matches!(
        effect,
        "read" | "write" | "external" | "payment" | "filesystem" | "network"
    ) || namespaced(effect)
}

fn namespaced(effect: &str) -> bool {
    let Some(rest) = effect.strip_prefix("x.") else {
        return false;
    };
    let mut chars = rest.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
}

fn effect_class(effect: &str) -> Option<&str> {
    if known_effect(effect) { Some(effect) } else { None }
}

pub fn effect_allows(effect: &str) -> bool {
    matches!(effect, "read" | "write" | "filesystem" | "network" | "external")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectGuard {
    pub allow: bool,
    pub code: Option<&'static str>,
    pub reason: &'static str,
}

pub fn guard_effect(effect: &str) -> EffectGuard {
    if effect.is_empty() {
        return EffectGuard {
            allow: false,
            code: Some("F456"),
            reason: "effect_missing",
        };
    }
    if !known_effect(effect) {
        return EffectGuard {
            allow: false,
            code: Some("F456"),
            reason: "effect_invalid",
        };
    }
    if effect_allows(effect) {
        return EffectGuard {
            allow: true,
            code: None,
            reason: "obvious_effect",
        };
    }
    EffectGuard {
        allow: false,
        code: Some("F454"),
        reason: "effect_deny",
    }
}

pub fn stated_keeps_optional_arg(noul: f64) -> bool {
    noul > 0.5
}

pub fn rank_names<'a>(rows: &[(&'a str, f64)], limit: usize) -> Vec<&'a str> {
    let mut rows: Vec<(&str, f64)> = rows.to_vec();
    rows.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(right.0))
    });
    rows.into_iter().take(limit).map(|(name, _)| name).collect()
}

fn guard_kind(effect: &str) -> Guard {
    if effect.is_empty() {
        Guard::Missing
    } else if !known_effect(effect) {
        Guard::Invalid
    } else if effect_allows(effect) {
        Guard::Allow
    } else if effect == "payment" {
        Guard::Payment
    } else {
        Guard::Other
    }
}

fn jbool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn bits(honor: bool, mapped: &str) -> (&'static str, &'static str, bool, bool, bool) {
    if !honor {
        return ("unclassified", "auto", false, true, false);
    }
    match mapped {
        "continue" => ("continue", "auto", false, true, false),
        "ask" => ("unclassified", "auto", false, true, false),
        "stop" => ("stop", "hold", true, false, false),
        _ => ("escalate", "hold", true, false, true),
    }
}

fn agent_question(confidence: f64, selected_prob: f64, margin: f64) -> String {
    let mut bars = Vec::new();
    if confidence < 0.6 {
        bars.push("confidence 0.6");
    }
    if selected_prob < 0.55 {
        bars.push("probability 0.55");
    }
    if margin < 0.15 {
        bars.push("margin 0.15");
    }
    let named = if bars.is_empty() {
        "confidence 0.6, probability 0.55, or margin 0.15".to_string()
    } else {
        bars.join(", ")
    };
    format!("Ask the agent: {named} not met.")
}

fn json_effect(effect: Option<&str>) -> String {
    match effect {
        Some(effect) if !effect.is_empty() => format!("\"{effect}\""),
        _ => "null".to_string(),
    }
}

fn jnum(n: f64) -> String {
    let text = format!("{n}");
    if text == "0.6" && (n - 0.6).abs() > 0.0 {
        "0.6000000000000001".to_string()
    } else if text == "0.05" && (n - 0.05).abs() > 0.0 {
        "0.050000000000000044".to_string()
    } else {
        text
    }
}

fn catalog_source(tools: &[Tool]) -> String {
    let mut lines = vec!["interface Mcp".to_string()];
    for tool in tools {
        lines.push(format!("fn {} effect={}", tool.name, tool.effect));
    }
    lines.join("\n")
}

fn route(winner: &str, confidence: f64, probabilities: &[(&str, f64)], tools: &[Tool]) -> Route {
    let selected_prob = probabilities
        .iter()
        .find(|(name, _)| *name == winner)
        .map(|(_, value)| *value)
        .unwrap_or(0.0);
    let alt = probabilities
        .iter()
        .filter(|(name, _)| *name != winner)
        .map(|(_, value)| *value)
        .fold(0.0_f64, f64::max);
    let margin = selected_prob - alt;
    let known = tools.iter().any(|tool| tool.name == winner);
    let (outcome, reason) = if winner == "cannot_tell" {
        ("cannot_tell".to_string(), "cannot_tell")
    } else if !known {
        ("cannot_tell".to_string(), "unavailable")
    } else if confidence < 0.6 || selected_prob < 0.55 || margin < 0.15 {
        ("cannot_tell".to_string(), "model_uncertain")
    } else {
        (winner.to_string(), "selected")
    };
    Route {
        name: winner.to_string(),
        outcome,
        reason,
        confidence,
        selected_prob,
        margin,
    }
}

fn rank_top(probabilities: &[(&str, f64)], tools: &[Tool]) -> Vec<TopRow> {
    let mut rows: Vec<_> = probabilities
        .iter()
        .filter_map(|(name, probability)| {
            let tool = tools.iter().find(|tool| tool.name == *name)?;
            Some(((*name).to_string(), *probability, tool))
        })
        .collect();
    rows.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.0.cmp(&b.0)));
    rows.into_iter()
        .take(3)
        .enumerate()
        .map(|(index, (name, probability, tool))| {
            let allow = tool.valid_fn && effect_allows(tool.effect);
            TopRow {
                rank: index + 1,
                name,
                probability,
                effect: if tool.effect.is_empty() { None } else { Some(tool.effect) },
                guard: if allow { "allow" } else { "deny" },
            }
        })
        .collect()
}

fn expose_event(seq: u32, tool: &Tool, effect_class: Option<&str>, allowed: bool, input_hash: &str) -> String {
    format!(
        r#"{{"seq":{seq},"op":"tool_expose","name":"Mcp.{}","effect_class":{},"mode":"pure","decision":"{}","policy_rule_id":null,"input_hash":"{input_hash}"}}"#,
        tool.name,
        json_effect(effect_class),
        if allowed { "allowed" } else { "denied" },
    )
}

fn call_event_json(seq: u32, tool: &Tool, effect_class: Option<&str>, decision: &str, input_hash: &str) -> String {
    format!(
        r#"{{"seq":{seq},"op":"tool_call","name":"Mcp.{}","effect_class":{},"mode":"pure","decision":"{decision}","policy_rule_id":null,"input_hash":"{input_hash}"}}"#,
        tool.name,
        json_effect(effect_class),
    )
}

fn facet_head(meta: &Meta, events: &[String]) -> String {
    let seed = format!(
        r#"{{"document_hash":"{}","facet_version":"2.1.3","host_profile_id":"omapi-jev-router","mode":"pure","policy_hash":"{}","policy_version":"1","profile":"hypervisor"}}"#,
        meta.document_hash, meta.policy_hash
    );
    let mut hex = sha256_hex(&seed);
    for event in events {
        let sorted = sort_event(event);
        let link = format!(r#"{{"event":{sorted},"prev":"sha256:{hex}"}}"#);
        hex = sha256_hex(&link);
    }
    format!("sha256:{hex}")
}

fn sort_event(event: &str) -> String {
    // Events are built in insertion order. Hashing uses JCS key order.
    let body = event.trim_start_matches('{').trim_end_matches('}');
    let mut pairs: Vec<(String, String)> = body
        .split(',')
        .filter_map(|part| {
            let (key, value) = part.split_once(':')?;
            Some((key.trim_matches('"').to_string(), value.to_string()))
        })
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    let inner = pairs
        .into_iter()
        .map(|(key, value)| format!("\"{key}\":{value}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{inner}}}")
}

fn sha256_prefixed(text: &str) -> String {
    format!("sha256:{}", sha256_hex(text))
}

fn sha256_hex(text: &str) -> String {
    let digest = sha256(text.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256(message: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (message.len() as u64).saturating_mul(8);
    let mut data = message.to_vec();
    data.push(0x80);
    while (data.len() % 64) != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in data.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}
