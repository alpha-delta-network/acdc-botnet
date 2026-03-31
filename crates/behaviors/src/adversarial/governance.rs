// Governance manipulation attack patterns
use adnet_testbot::{BehaviorResult, BotContext, Result};
use adnet_testbot_integration::AdnetClient;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// PT-A-040: Malicious Proposal (parameter change to extreme value)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaliciousProposal {
    pub parameter: String,
    pub malicious_value: serde_json::Value,
}

impl MaliciousProposal {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!(
            "ATTACK: Malicious governance proposal — {}={:?}",
            self.parameter,
            self.malicious_value
        );
        let client = AdnetClient::new(context.execution.network.adnet_unified.clone())?;
        let result = client
            .submit_governance_proposal(&json!({
                "type": "parameter_update",
                "parameter": &self.parameter,
                "new_value": &self.malicious_value,
            }))
            .await;
        match result {
            Err(_) => Ok(BehaviorResult::error(
                "malicious proposal rejected by governance validation",
            )),
            Ok(pid) => Ok(BehaviorResult::success(format!(
                "WARNING: malicious proposal #{pid} accepted!"
            ))),
        }
    }
}

/// PT-A-041: Grim Trigger Abuse (fraudulent GID cripple report)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrimTriggerAbuse {
    pub target_gid: String,
}

impl GrimTriggerAbuse {
    pub async fn execute(&self, context: &BotContext) -> Result<BehaviorResult> {
        tracing::warn!(
            "ATTACK: Fraudulent grim trigger against GID {}",
            self.target_gid
        );
        let client = AdnetClient::new(context.execution.network.adnet_unified.clone())?;
        let result = client
            .submit_governance_proposal(&json!({
                "type": "grim_trigger_report",
                "target_gid": &self.target_gid,
                "fraudulent": true,
            }))
            .await;
        match result {
            Err(_) => Ok(BehaviorResult::error(
                "fraudulent grim trigger rejected — evidence validation failed",
            )),
            Ok(_) => Ok(BehaviorResult::success(
                "WARNING: fraudulent trigger accepted — governance regression!",
            )),
        }
    }
}
