//! Oracle: typed calls. Every behavior `packages/jev-router/test.mjs` asserts
//! for `decideTypedCalls`, `callsBypass`, the effect guard, and the facet
//! envelope, asserted against the shipped crate's public API.

use bezel_bridle::{
    cannot_tell_json, envelope_matches, hard_stop_json, missing_key_envelope, model_uncertain_json,
    obvious_effect_json, tool_call, HardStopTool,
};
use std::path::PathBuf;

fn calls_golden(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/jev-router/testdata/calls")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

#[test]
fn transport_is_facet_and_nothing_is_initiated() {
    let call = tool_call();
    assert_eq!(call.transport, "facet");
    assert!(!call.initiated);
    assert_eq!(call.op, "tool_call");
    assert_eq!(call.facet_version, "2.1.3");
}

#[test]
fn missing_key_envelope_is_byte_identical_to_the_node_fixture() {
    let body = missing_key_envelope();
    let recorded = calls_golden("missing-key.json");
    assert_eq!(body.as_bytes(), recorded.as_slice());
    let text = String::from_utf8(recorded).unwrap();
    assert!(envelope_matches(&body, &text));
    // Behaviors test.mjs asserts on the shadow, missing-key decision.
    assert!(body.contains("\"transport\":\"facet\""));
    assert!(body.contains("\"facetVersion\":\"2.1.3\""));
    assert!(body.contains("\"policyId\":\"omapi-route-workflow-policy@1\""));
    assert!(body.contains("\"mode\":\"shadow\""));
    assert!(body.contains("\"honor\":false"));
    assert!(body.contains("\"shadow\":true"));
    assert!(body.contains("\"choice\":\"unclassified\""));
    assert!(body.contains("\"blocked\":false"));
    assert!(body.contains("\"exec\":true"));
    assert!(body.contains("\"hitl\":false"));
    assert!(body.contains("\"label\":\"ask\""));
    assert!(body.contains("\"mapped\":\"escalate\""));
    assert!(body.contains("\"reason\":\"missing_key\""));
    assert!(body.contains("\"missingKey\":true"));
    assert!(body.contains("\"autoAllow\":false"));
    assert!(body.contains("\"autoPromote\":false"));
    assert!(body.contains("\"initiated\":false"));
    assert!(body.contains("\"best\":null"));
    // No jsonrpc, no tools/call, no F454 stamp, no allow label.
    assert!(!body.contains("jsonrpc"));
    assert!(!body.contains("tools/call"));
    assert!(!body.contains("F454"));
    assert!(!body.contains("\"label\":\"allow\""));
    assert!(!body.contains("\"choice\":\"allow\""));
    // The artifact hash chain head is a sha256 link and expose events exist.
    assert!(body.contains("\"hash_chain\":{\"algo\":\"sha256\",\"head\":\"sha256:"));
    assert!(body.contains("\"op\":\"tool_expose\""));
    // A paraphrased envelope is not the recorded answer.
    let mut paraphrased = body.clone().into_bytes();
    paraphrased[0] = b' ';
    let paraphrased = String::from_utf8(paraphrased).unwrap();
    assert!(!envelope_matches(&paraphrased, &text));
}

#[test]
fn hard_stops_stop_by_code_and_never_promote() {
    let next = HardStopTool {
        name: "linear__list_issues",
        description: "List issues",
        effect: "read",
    };
    let cases: [(&str, &str, HardStopTool, &str); 4] = [
        (
            "invalid-fn.json",
            "linear.save",
            HardStopTool {
                name: "linear.save",
                description: "Bad name",
                effect: "read",
            },
            "F452",
        ),
        (
            "missing-effect.json",
            "bare__tool",
            HardStopTool {
                name: "bare__tool",
                description: "No effect",
                effect: "",
            },
            "F456",
        ),
        (
            "invalid-effect.json",
            "odd__tool",
            HardStopTool {
                name: "odd__tool",
                description: "Odd",
                effect: "not-an-effect",
            },
            "F456",
        ),
        (
            "payment.json",
            "pay__now",
            HardStopTool {
                name: "pay__now",
                description: "Pay",
                effect: "payment",
            },
            "F454",
        ),
    ];
    for (file, winner, tool, code) in cases {
        let body = hard_stop_json(winner, &[tool, next], &[(winner, 0.8), ("linear__list_issues", 0.2)]);
        let recorded = calls_golden(file);
        assert_eq!(body.as_bytes(), recorded.as_slice(), "{file}");
        assert!(body.contains(&format!("\"code\":\"{code}\"")), "{file}");
        assert!(body.contains("\"mapped\":\"stop\""), "{file}");
        assert!(body.contains("\"choice\":\"stop\""), "{file}");
        assert!(body.contains("\"codeDeny\":true"), "{file}");
        assert!(body.contains("\"best\":null"), "{file}");
        assert!(body.contains("\"autoPromote\":false"), "{file}");
        assert!(body.contains("\"initiated\":false"), "{file}");
        assert!(!body.contains("\"decision\":\"allow\""), "{file}");
    }
}

#[test]
fn obvious_effects_continue_and_are_never_stamped_f454() {
    for effect in ["read", "write", "filesystem", "network", "external"] {
        let body = obvious_effect_json(effect, "active");
        let recorded = calls_golden(&format!("{effect}.json"));
        assert_eq!(body.as_bytes(), recorded.as_slice(), "{effect}");
        assert!(body.contains("\"mapped\":\"continue\""), "{effect}");
        assert!(body.contains("\"choice\":\"continue\""), "{effect}");
        assert!(body.contains("\"blocked\":false"), "{effect}");
        assert!(body.contains("\"code\":null"), "{effect}");
        assert!(body.contains("\"codeDeny\":false"), "{effect}");
        assert!(body.contains("\"initiated\":false"), "{effect}");
        assert!(body.contains("\"autoAllow\":false"), "{effect}");
        assert!(body.contains("\"autoPromote\":false"), "{effect}");
        assert!(body.contains(&format!("\"effect\":\"{effect}\"")), "{effect}");
        assert!(!body.contains("F454"), "{effect}");
        assert!(!body.contains("\"decision\":\"allow\""), "{effect}");
    }
}

#[test]
fn obvious_effect_in_shadow_honors_nothing() {
    let shadow = obvious_effect_json("write", "shadow");
    assert_eq!(shadow.as_bytes(), calls_golden("write-shadow.json").as_slice());
    assert!(shadow.contains("\"honor\":false"));
    assert!(shadow.contains("\"mode\":\"shadow\""));
    assert!(shadow.contains("\"blocked\":false"));
    assert!(shadow.contains("\"choice\":\"unclassified\""));
    assert!(shadow.contains("\"mapped\":\"continue\""));
    assert!(!shadow.contains("F454"));
    assert!(!shadow.contains("\"decision\":\"allow\""));
}

#[test]
fn model_uncertain_names_every_failed_bar_and_asks() {
    let ask = model_uncertain_json();
    assert_eq!(ask.as_bytes(), calls_golden("model-uncertain.json").as_slice());
    assert!(ask.contains("\"reason\":\"model_uncertain\""));
    assert!(ask.contains("\"mapped\":\"ask\""));
    assert!(ask.contains("\"label\":\"ask\""));
    assert!(ask.contains("\"best\":null"));
    assert!(ask.contains("\"facet\":null"));
    assert!(ask.contains("\"blocked\":false"));
    assert!(ask.contains("\"hitl\":false"));
    // The question names all three threshold bars by number.
    assert!(ask.contains("confidence 0.6"));
    assert!(ask.contains("probability 0.55"));
    assert!(ask.contains("margin 0.15"));
    assert!(!ask.contains("F454"));
    assert!(!ask.contains("\"label\":\"allow\""));
}

#[test]
fn cannot_tell_escalates_to_a_human_with_detail_and_question() {
    let human = cannot_tell_json();
    assert_eq!(human.as_bytes(), calls_golden("cannot-tell.json").as_slice());
    assert!(human.contains("\"reason\":\"cannot_tell\""));
    assert!(human.contains("\"mapped\":\"escalate\""));
    assert!(human.contains("\"hitl\":true"));
    assert!(human.contains("\"blocked\":true"));
    assert!(human.contains("\"exec\":false"));
    assert!(human.contains("detail=cannot_tell"));
    assert!(human.contains("question="));
    assert!(!human.contains("F454"));
    assert!(!human.contains("\"decision\":\"allow\""));
}
