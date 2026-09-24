//! Loop-stop thresholds. Escalate stays human-in-the-loop.
//! A sub-threshold continue is `cannot_tell`, not a continue.

use crate::policy::{LOOP_STOP_MIN_CONFIDENCE, LOOP_STOP_MIN_MARGIN, LOOP_STOP_MIN_PROBABILITY};

#[derive(Debug, Clone, PartialEq)]
pub struct LoopStopDecision {
    pub outcome: &'static str,
    pub reason: &'static str,
}

pub fn apply_loop_stop_thresholds(choice: &str, confidence: f64, probabilities: &[(&str, f64)]) -> LoopStopDecision {
    let selected_prob = probabilities
        .iter()
        .find(|(label, _)| *label == choice)
        .map(|(_, value)| *value)
        .unwrap_or(0.0);
    let runner_up = probabilities
        .iter()
        .filter(|(label, _)| *label != choice)
        .map(|(_, value)| *value)
        .fold(0.0_f64, f64::max);
    let margin = selected_prob - runner_up;
    let confident = confidence >= LOOP_STOP_MIN_CONFIDENCE
        && selected_prob >= LOOP_STOP_MIN_PROBABILITY
        && margin >= LOOP_STOP_MIN_MARGIN;

    if choice == "escalate" {
        return LoopStopDecision {
            outcome: "escalate",
            reason: if confident { "selected" } else { "escalate_uncertain" },
        };
    }
    if choice != "continue" && choice != "stop" {
        return LoopStopDecision {
            outcome: "cannot_tell",
            reason: "cannot_tell",
        };
    }
    if !confident {
        return LoopStopDecision {
            outcome: "cannot_tell",
            reason: "model_uncertain",
        };
    }
    LoopStopDecision {
        outcome: if choice == "continue" { "continue" } else { "stop" },
        reason: "selected",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoopEnvelope {
    pub choice: &'static str,
    pub gate: &'static str,
    pub blocked: bool,
    pub exec: bool,
    pub hitl: bool,
    pub auto_retry: bool,
    pub auto_promote: bool,
    pub intent: String,
    pub step_digest: String,
    pub loop_selected: String,
    pub loop_outcome: &'static str,
    pub loop_reason: &'static str,
    pub loop_hitl: bool,
}

pub fn decide_loop_stop(
    choice: &str,
    confidence: f64,
    probabilities: &[(&str, f64)],
    mode: &str,
    intent: &str,
    step_digest: &str,
) -> LoopEnvelope {
    let decision = apply_loop_stop_thresholds(choice, confidence, probabilities);
    let active = mode.eq_ignore_ascii_case("active");
    let (choice_word, gate, reason) = if decision.outcome == "continue" {
        ("continue", "auto", decision.reason)
    } else if decision.outcome == "stop" {
        ("stop", "hold", decision.reason)
    } else if decision.outcome == "escalate" {
        ("escalate", "hold", decision.reason)
    } else if active {
        (
            "escalate",
            "hold",
            if decision.reason == "cannot_tell" {
                "cannot_tell"
            } else {
                "model_uncertain"
            },
        )
    } else {
        ("unclassified", "auto", decision.reason)
    };
    let hitl = choice_word == "escalate";
    let blocked = active && (choice_word == "stop" || choice_word == "escalate" || gate != "auto");
    let exec = crate::policy::should_exec_agent(if active { "active" } else { "shadow" }, choice_word, gate, blocked);
    LoopEnvelope {
        choice: choice_word,
        gate,
        blocked,
        exec,
        hitl,
        auto_retry: false,
        auto_promote: false,
        intent: intent.to_string(),
        step_digest: crate::facts::clip_to_max_hunk_chars(step_digest, Some(crate::policy::MAX_DIGEST_CHARS)),
        loop_selected: decision.outcome.to_string(),
        loop_outcome: decision.outcome,
        loop_reason: reason,
        loop_hitl: hitl,
    }
}

pub fn missing_key_loop(mode: &str, intent: &str, step_digest: &str) -> LoopEnvelope {
    if mode.eq_ignore_ascii_case("active") {
        let mut decided = decide_loop_stop("cannot_tell", 0.0, &[], "active", intent, step_digest);
        decided.loop_reason = "missing_key";
        return decided;
    }
    LoopEnvelope {
        choice: "unclassified",
        gate: "auto",
        blocked: false,
        exec: true,
        hitl: false,
        auto_retry: false,
        auto_promote: false,
        intent: intent.to_string(),
        step_digest: crate::facts::clip_to_max_hunk_chars(step_digest, Some(crate::policy::MAX_DIGEST_CHARS)),
        loop_selected: "unclassified".to_string(),
        loop_outcome: "cannot_tell",
        loop_reason: "missing_key",
        loop_hitl: false,
    }
}
