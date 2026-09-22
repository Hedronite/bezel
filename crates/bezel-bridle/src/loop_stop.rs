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
