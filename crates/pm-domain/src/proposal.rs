//! Analysis results and change proposals produced by AI skills.

use serde::{Deserialize, Serialize};

use crate::command::ProjectCommand;

/// Self-assessed confidence of an analysis skill in its own output.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// Risk of executing the proposed command.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// One executable command plus the evidence an AI skill offers for it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Proposal {
    pub command: ProjectCommand,
    pub rationale: String,
    pub risk_level: RiskLevel,
    pub confidence: Confidence,
    pub evidence: Vec<String>,
}

/// A single observation produced by an analysis skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Finding {
    pub skill: String,
    pub severity: String,
    pub description: String,
    pub evidence_refs: Vec<String>,
}

/// Full output of one AI analysis run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalysisResult {
    pub summary: String,
    pub findings: Vec<Finding>,
    pub proposals: Vec<Proposal>,
}

#[cfg(test)]
mod tests {
    // Per brief Step 1: tests here are covered by command roundtrip tests
    // (Proposal embeds ProjectCommand); no additional scope mandated.
}
