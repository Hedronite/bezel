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
