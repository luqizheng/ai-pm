//! Policy engine decision outcomes.

use serde::{Deserialize, Serialize};

/// What the policy engine decides for a given proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    AutoApply,
    RequirePreConfirmation { role: String },
    Escalate { reason: String },
    Deny { reason: String },
}

#[cfg(test)]
mod tests {
    // Per brief Step 1: no tests mandated for PolicyDecision in this task.
}
