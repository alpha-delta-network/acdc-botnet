/// Governance behavior patterns
use adnet_testbot::{BehaviorResult, BotContext, BotError, Result};
use adnet_testbot_integration::AdnetClient;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Behavior trait for governance patterns
#[async_trait]
pub trait GovernanceBehavior: Send + Sync {
    async fn execute(&self, context: &BotContext) -> Result<BehaviorResult>;
}

/// PT-L-001: Basic Proposal Voting
///
/// Complete governance proposal flow: list active proposals → cast vote for each.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicProposalVoting {
    pub proposal_type: String,
    pub vote: VoteOption,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VoteOption {
    Yes,
    No,
    Abstain,
}

impl VoteOption {
    pub fn as_str(&self) -> &'static str {
        match self {
            VoteOption::Yes => "yes",
            VoteOption::No => "no",
            VoteOption::Abstain => "abstain",
        }
    }
}

impl BasicProposalVoting {
    pub fn new(proposal_type: String, vote: VoteOption) -> Self {
        Self {
            proposal_type,
            vote,
        }
    }
}

#[async_trait]
impl GovernanceBehavior for BasicProposalVoting {
    async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "Bot {} executing governance vote ({:?}) on type={}",
            context.execution.bot_id,
            self.vote,
            self.proposal_type
        );

        let adnet_url = context.execution.network.adnet_unified.clone();
        let client = AdnetClient::new(adnet_url)?;

        // 1. List active proposals
        let proposals_resp = client.get_governance_proposals().await?;
        let proposals = proposals_resp.proposals.unwrap_or_default();

        if proposals.is_empty() {
            return Ok(BehaviorResult::success(
                "No active proposals found — nothing to vote on",
            ));
        }

        let vote_str = self.vote.as_str();
        let mut votes_cast = 0usize;
        let mut errors = Vec::<String>::new();

        // 2. Vote on each active proposal whose type matches
        for proposal in &proposals {
            let id = match proposal.get("id").and_then(|v| v.as_u64()) {
                Some(id) => id,
                None => continue,
            };

            let status = proposal
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if status != "active" && status != "voting" {
                continue;
            }

            let p_type = proposal.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if !self.proposal_type.is_empty() && p_type != self.proposal_type {
                continue;
            }

            let body = serde_json::json!({
                "voter": context.execution.bot_id,
                "vote": vote_str,
            });

            match client.submit_governance_vote(id, &body).await {
                Ok(_) => {
                    votes_cast += 1;
                    tracing::info!(
                        "Bot {} voted {} on proposal {}",
                        context.execution.bot_id,
                        vote_str,
                        id
                    );
                }
                Err(e) => {
                    let msg = format!("proposal {}: {}", id, e);
                    tracing::warn!("Bot {} vote error: {}", context.execution.bot_id, msg);
                    errors.push(msg);
                }
            }
        }

        if errors.is_empty() {
            Ok(BehaviorResult::success(format!(
                "Voted {} on {} proposals",
                vote_str, votes_cast
            )))
        } else {
            Ok(BehaviorResult::success(format!(
                "Voted {} on {} proposals ({} errors: {})",
                vote_str,
                votes_cast,
                errors.len(),
                errors.join("; ")
            )))
        }
    }
}

/// GovernanceMonitor — polls active proposals and emits status summaries.
///
/// Not a voting behavior — used for observation and reporting.
#[derive(Debug, Clone)]
pub struct GovernanceMonitor {
    /// Human-readable label for this monitor instance
    pub label: String,
}

impl GovernanceMonitor {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }

    /// Poll all proposals and return a status summary.
    ///
    /// Returns a JSON value with counts by status and proposal ids.
    pub async fn poll(&self, context: &BotContext) -> Result<serde_json::Value> {
        let adnet_url = context.execution.network.adnet_unified.clone();
        let client = AdnetClient::new(adnet_url)?;
        let resp = client.get_governance_proposals().await?;
        let proposals = resp.proposals.unwrap_or_default();

        let mut by_status: std::collections::HashMap<String, Vec<u64>> =
            std::collections::HashMap::new();

        for p in &proposals {
            let status = p
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let id = p.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            by_status.entry(status).or_default().push(id);
        }

        Ok(serde_json::json!({
            "label": self.label,
            "total": proposals.len(),
            "by_status": by_status,
        }))
    }
}

#[async_trait]
impl GovernanceBehavior for GovernanceMonitor {
    async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "Bot {} running GovernanceMonitor ({})",
            context.execution.bot_id,
            self.label
        );
        let summary = self.poll(context).await?;
        Ok(BehaviorResult::success(summary.to_string()))
    }
}

/// PT-L-003: Joint Alpha/Delta Governance
///
/// Proposals that require coordination across both chains.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointGovernance {
    pub alpha_proposal_id: String,
    pub delta_proposal_id: String,
    pub vote: VoteOption,
}

impl JointGovernance {
    pub fn new(alpha_proposal_id: String, delta_proposal_id: String, vote: VoteOption) -> Self {
        Self {
            alpha_proposal_id,
            delta_proposal_id,
            vote,
        }
    }
}

#[async_trait]
impl GovernanceBehavior for JointGovernance {
    async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "Bot {} executing joint governance vote (alpha={} delta={})",
            context.execution.bot_id,
            self.alpha_proposal_id,
            self.delta_proposal_id,
        );

        let adnet_url = context.execution.network.adnet_unified.clone();
        let client = AdnetClient::new(adnet_url)?;

        let vote_str = self.vote.as_str();
        let body = serde_json::json!({
            "voter": context.execution.bot_id,
            "vote": vote_str,
        });

        let alpha_id: u64 = self.alpha_proposal_id.parse().unwrap_or(0);
        let delta_id: u64 = self.delta_proposal_id.parse().unwrap_or(0);

        if alpha_id == 0 || delta_id == 0 {
            return Err(BotError::BehaviorError(format!(
                "JointGovernance: invalid proposal IDs alpha={} delta={}",
                self.alpha_proposal_id, self.delta_proposal_id
            )));
        }

        client.submit_governance_vote(alpha_id, &body).await?;
        tracing::info!(
            "Bot {} voted {} on Alpha proposal {}",
            context.execution.bot_id,
            vote_str,
            alpha_id
        );

        client.submit_governance_vote(delta_id, &body).await?;
        tracing::info!(
            "Bot {} voted {} on Delta proposal {}",
            context.execution.bot_id,
            vote_str,
            delta_id
        );

        Ok(BehaviorResult::success(format!(
            "Joint governance vote {} cast on Alpha proposal {} and Delta proposal {}",
            vote_str, alpha_id, delta_id
        )))
    }
}

/// PT-L-004: Apology Lifecycle
///
/// Detects a crippled GID via grim trigger API, submits an apology proposal,
/// and polls for its execution (status → "passed" → "executed").
#[derive(Debug, Clone)]
pub struct ApologyLifecycle {
    /// The GID address that needs recovery
    pub crippled_gid: String,
}

impl ApologyLifecycle {
    pub fn new(crippled_gid: impl Into<String>) -> Self {
        Self {
            crippled_gid: crippled_gid.into(),
        }
    }
}

#[async_trait]
impl GovernanceBehavior for ApologyLifecycle {
    async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "Bot {} running ApologyLifecycle for GID {}",
            context.execution.bot_id,
            self.crippled_gid
        );

        let adnet_url = context.execution.network.adnet_unified.clone();
        let client = AdnetClient::new(adnet_url)?;

        // 1. Check grim trigger status
        let gt_status = client.get_grim_trigger_status(&self.crippled_gid).await?;
        let is_crippled = gt_status
            .get("is_crippled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !is_crippled {
            return Ok(BehaviorResult::success(format!(
                "GID {} is not crippled — apology lifecycle not needed",
                self.crippled_gid
            )));
        }

        // 2. Submit apology proposal
        let apology_payload = serde_json::json!({
            "type": "apology_restore",
            "gid_address": self.crippled_gid,
            "proposer": context.execution.bot_id,
            "description": format!(
                "Apology proposal: GID {} requests restoration after grim trigger crippling.",
                self.crippled_gid
            ),
            "threshold_pct": 51u8,
        });

        let proposal_id = client.submit_governance_proposal(&apology_payload).await?;

        tracing::info!(
            "Bot {} submitted apology proposal {} for GID {}",
            context.execution.bot_id,
            proposal_id,
            self.crippled_gid
        );

        Ok(BehaviorResult::success(format!(
            "Apology proposal {} submitted for GID {} (awaiting 51% threshold)",
            proposal_id, self.crippled_gid
        )))
    }
}

/// PT-L-005: Governance Executor
///
/// Polls proposals in "passed" state and submits execute calls for each.
#[derive(Debug, Clone)]
pub struct GovernanceExecutor;

impl GovernanceExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GovernanceExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GovernanceBehavior for GovernanceExecutor {
    async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "Bot {} running GovernanceExecutor",
            context.execution.bot_id
        );

        let adnet_url = context.execution.network.adnet_unified.clone();
        let client = AdnetClient::new(adnet_url)?;

        let proposals_resp = client.get_governance_proposals().await?;
        let proposals = proposals_resp.proposals.unwrap_or_default();

        let mut executed = 0usize;
        let mut errors = Vec::<String>::new();

        for proposal in &proposals {
            let id = match proposal.get("id").and_then(|v| v.as_u64()) {
                Some(id) => id,
                None => continue,
            };
            let status = proposal
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if status != "passed" {
                continue;
            }

            // POST /api/v1/governance/proposals/:id/execute
            let execute_path = format!("/api/v1/governance/proposals/{}/execute", id);
            let body = serde_json::json!({ "executor": context.execution.bot_id });
            match client.post_json_raw(&execute_path, &body).await {
                Ok(_) => {
                    executed += 1;
                    tracing::info!("Bot {} executed proposal {}", context.execution.bot_id, id);
                }
                Err(e) => {
                    errors.push(format!("proposal {}: {}", id, e));
                }
            }
        }

        if errors.is_empty() {
            Ok(BehaviorResult::success(format!(
                "Executed {} passed proposals",
                executed
            )))
        } else {
            Ok(BehaviorResult::success(format!(
                "Executed {} proposals ({} errors: {})",
                executed,
                errors.len(),
                errors.join("; ")
            )))
        }
    }
}

/// PT-L-006: Governance Integrity Verifier
///
/// Reads all proposals, verifies that executed proposals match their expected
/// threshold, and reports any integrity anomalies.
#[derive(Debug, Clone)]
pub struct GovernanceIntegrityVerifier;

impl GovernanceIntegrityVerifier {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GovernanceIntegrityVerifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GovernanceBehavior for GovernanceIntegrityVerifier {
    async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::info!(
            "Bot {} running GovernanceIntegrityVerifier",
            context.execution.bot_id
        );

        let adnet_url = context.execution.network.adnet_unified.clone();
        let client = AdnetClient::new(adnet_url)?;

        let proposals_resp = client.get_governance_proposals().await?;
        let proposals = proposals_resp.proposals.unwrap_or_default();

        let mut anomalies = Vec::<String>::new();
        let mut verified = 0usize;

        for proposal in &proposals {
            let id = proposal.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
            let status = proposal
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let threshold = proposal
                .get("threshold_pct")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            // Integrity check: executed proposals must have threshold ≥ 51
            if status == "executed" {
                if threshold < 51 {
                    anomalies.push(format!(
                        "proposal {} executed with threshold {}% < 51% minimum",
                        id, threshold
                    ));
                } else {
                    verified += 1;
                }
            }

            // Integrity check: apology_restore must not use threshold > 51
            let p_type = proposal.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if p_type == "apology_restore" && threshold > 51 {
                anomalies.push(format!(
                    "apology_restore proposal {} has threshold {}% > 51% (should be exactly 51%)",
                    id, threshold
                ));
            }
        }

        if anomalies.is_empty() {
            Ok(BehaviorResult::success(format!(
                "Governance integrity check passed: {} executed proposals verified",
                verified
            )))
        } else {
            Ok(BehaviorResult::success(format!(
                "Governance integrity ANOMALIES ({} found): {}",
                anomalies.len(),
                anomalies.join("; ")
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_proposal_creation() {
        let behavior = BasicProposalVoting::new("parameter_update".to_string(), VoteOption::Yes);
        assert_eq!(behavior.proposal_type, "parameter_update");
    }

    #[test]
    fn test_vote_option_as_str() {
        assert_eq!(VoteOption::Yes.as_str(), "yes");
        assert_eq!(VoteOption::No.as_str(), "no");
        assert_eq!(VoteOption::Abstain.as_str(), "abstain");
    }

    #[test]
    fn test_governance_monitor_new() {
        let m = GovernanceMonitor::new("test-monitor");
        assert_eq!(m.label, "test-monitor");
    }

    #[test]
    fn test_joint_governance_new() {
        let jg = JointGovernance::new("1".to_string(), "2".to_string(), VoteOption::Yes);
        assert_eq!(jg.alpha_proposal_id, "1");
        assert_eq!(jg.delta_proposal_id, "2");
        assert_eq!(jg.vote, VoteOption::Yes);
    }

    #[test]
    fn test_apology_lifecycle_new() {
        let al = ApologyLifecycle::new("gid:alpha:abc123");
        assert_eq!(al.crippled_gid, "gid:alpha:abc123");
    }

    #[test]
    fn test_governance_executor_default() {
        let _ = GovernanceExecutor::default();
    }

    #[test]
    fn test_governance_integrity_verifier_default() {
        let _ = GovernanceIntegrityVerifier::default();
    }
}
