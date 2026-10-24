//! Pre-delegation token budget gate via agent-heart `/budget/check`.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::path::Path;

use crate::mcp_bridge::McpBridge;
use crate::workflow::NodeKind;

#[derive(Debug, Clone)]
pub struct BudgetGate {
    heart_url: String,
    heart_bin: String,
    enabled: bool,
    client: reqwest::Client,
    /// Track freeze offenses per phase — first offense is warning-only.
    freeze_offenses: Arc<Mutex<std::collections::HashMap<String, u32>>>,
}

#[derive(Debug, Clone, Serialize)]
struct BudgetCheckRequest {
    phase: String,
    estimated_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BudgetDecision {
    pub allowed: bool,
    pub frozen: bool,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum BudgetGateError {
    #[error("token budget frozen: {reason}")]
    Frozen { reason: String },
    #[error("token budget check unavailable: {reason}")]
    Unavailable { reason: String },
}

#[derive(Debug, Error, PartialEq)]
pub enum PreconditionError {
    #[error("workflow budget ceiling exceeded: current ${current:.2} >= max ${max:.2}")]
    WorkflowBudgetExceeded { current: f64, max: f64 },
    #[error(
        "budget headroom insufficient for node '{node}': remaining ${remaining:.2} < required ${required:.2}"
    )]
    InsufficientHeadroom {
        node: String,
        remaining: f64,
        required: f64,
    },
    #[error(
        "provider quota exceeded for node '{node}': provider '{provider}' utilization {utilization:.1}% > max {max_allowed:.1}%"
    )]
    QuotaExceeded {
        node: String,
        provider: String,
        utilization: f64,
        max_allowed: f64,
    },
    #[error("budget unavailable for node '{node}': heart did not report headroom ({reason})")]
    BudgetUnavailable { node: String, reason: String },
    #[error("quota unavailable for node '{node}' provider '{provider}': {reason}")]
    QuotaUnavailable {
        node: String,
        provider: String,
        reason: String,
    },
    #[error("unknown precondition check '{check}' on node '{node}'")]
    UnknownCheck { node: String, check: String },
}

impl BudgetGate {
    #[must_use]
    pub fn from_env() -> Self {
        let enabled = std::env::var("AUTONOMIC_BUDGET_GATE")
            .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true);
        let heart_url =
            std::env::var("AUTONOMIC_HEART_URL").unwrap_or_else(|_| "http://127.0.0.1:3101".into());
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(500))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            heart_url: heart_url.trim_end_matches('/').to_string(),
            heart_bin: std::env::var("AGENT_HEART_BIN").unwrap_or_else(|_| "agent-heart".into()),
            enabled,
            client,
            freeze_offenses: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    #[must_use]
    pub fn disabled() -> Self {
        Self {
            heart_url: String::new(),
            heart_bin: String::new(),
            enabled: false,
            client: reqwest::Client::new(),
            freeze_offenses: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub async fn check_node(
        &self,
        node_kind: &str,
        estimated_tokens: u64,
    ) -> Result<(), BudgetGateError> {
        if !self.enabled {
            return Ok(());
        }

        let req = BudgetCheckRequest {
            phase: phase_for_kind(node_kind).into(),
            estimated_tokens: estimated_tokens.max(1),
            task_kind: Some(node_kind.into()),
        };

        let url = format!("{}/budget/check", self.heart_url);
        let resp = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| BudgetGateError::Unavailable {
                reason: format!("heart unreachable: {e}"),
            })?;

        if !resp.status().is_success() {
            return Err(BudgetGateError::Unavailable {
                reason: format!("heart returned HTTP {}", resp.status()),
            });
        }

        let body: serde_json::Value =
            resp.json()
                .await
                .map_err(|e| BudgetGateError::Unavailable {
                    reason: format!("heart response unparseable: {e}"),
                })?;

        if body.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(BudgetGateError::Unavailable {
                reason: "heart returned an error response".into(),
            });
        }

        let decision_val = body
            .get("decision")
            .ok_or_else(|| BudgetGateError::Unavailable {
                reason: "heart response has no decision".into(),
            })?;

        let decision: BudgetDecision =
            serde_json::from_value(decision_val.clone()).map_err(|e| {
                BudgetGateError::Unavailable {
                    reason: format!("heart decision unparseable: {e}"),
                }
            })?;

        if !decision.allowed || decision.frozen {
            // First freeze per phase is a warning — subsequent ones fail hard
            let phase = phase_for_kind(node_kind).to_string();
            let mut offenses = self.freeze_offenses.lock().unwrap();
            let count = offenses.entry(phase.clone()).or_insert(0);
            *count += 1;
            if *count <= 1 {
                tracing::warn!(
                    phase = %phase,
                    "budget gate first offense (warning only): {}",
                    decision.reason
                );
                return Ok(());
            }
            return Err(BudgetGateError::Frozen {
                reason: decision.reason,
            });
        }

        Ok(())
    }

    pub async fn query_budget_headroom(&self) -> Result<f64, String> {
        let mut heart = McpBridge::connect_to(Path::new(&self.heart_bin), "serve-mcp")
            .await
            .map_err(|e| e.to_string())?;
        let summary = heart
            .call_tool(
                "heart_usage_summary",
                serde_json::json!({ "lookback_days": 30 }),
            )
            .await
            .map_err(|e| e.to_string())?;
        headroom_from(&summary)
    }

    pub async fn query_quota_utilization(&self, provider: &str) -> Result<f64, String> {
        let mut heart = McpBridge::connect_to(Path::new(&self.heart_bin), "serve-mcp")
            .await
            .map_err(|e| e.to_string())?;
        let snapshots = heart
            .call_tool("heart_quota_status", serde_json::json!({}))
            .await
            .map_err(|e| e.to_string())?;
        quota_utilization_from(&snapshots, provider)
    }

    pub async fn evaluate_step_preconditions(
        &self,
        node_name: &str,
        preconditions: &[crate::workflow::StepPrecondition],
        current_workflow_cost: f64,
        max_workflow_cost: Option<f64>,
    ) -> Result<(), PreconditionError> {
        if let Some(max_cost) = max_workflow_cost {
            if current_workflow_cost >= max_cost {
                return Err(PreconditionError::WorkflowBudgetExceeded {
                    current: current_workflow_cost,
                    max: max_cost,
                });
            }
        }

        if !self.enabled || preconditions.is_empty() {
            return Ok(());
        }

        for pre in preconditions {
            match pre.check.as_str() {
                "budget_headroom" => {
                    let required = pre.min_remaining_usd.unwrap_or(0.0);
                    let remaining = match self.query_budget_headroom().await {
                        Ok(remaining) => remaining,
                        Err(reason) => {
                            return Err(PreconditionError::BudgetUnavailable {
                                node: node_name.to_owned(),
                                reason,
                            });
                        }
                    };
                    if remaining < required {
                        return Err(PreconditionError::InsufficientHeadroom {
                            node: node_name.to_owned(),
                            remaining,
                            required,
                        });
                    }
                }
                "quota_available" => {
                    let provider = pre.provider.as_deref().unwrap_or("unknown");
                    let max_allowed = pre.max_utilization_pct.unwrap_or(100.0);
                    let utilization = match self.query_quota_utilization(provider).await {
                        Ok(utilization) => utilization,
                        Err(reason) => {
                            return Err(PreconditionError::QuotaUnavailable {
                                node: node_name.to_owned(),
                                provider: provider.to_owned(),
                                reason,
                            });
                        }
                    };
                    if utilization > max_allowed {
                        return Err(PreconditionError::QuotaExceeded {
                            node: node_name.to_owned(),
                            provider: provider.to_owned(),
                            utilization,
                            max_allowed,
                        });
                    }
                }
                other => {
                    return Err(PreconditionError::UnknownCheck {
                        node: node_name.to_owned(),
                        check: other.to_owned(),
                    });
                }
            }
        }

        Ok(())
    }
}

#[must_use]
fn headroom_from(summary: &serde_json::Value) -> Result<f64, String> {
    summary["remaining_budget_usd"]
        .as_f64()
        .ok_or_else(|| "heart reported no remaining_budget_usd".to_string())
}

fn quota_utilization_from(snapshots: &serde_json::Value, provider: &str) -> Result<f64, String> {
    snapshots
        .as_array()
        .into_iter()
        .flatten()
        .filter(|snapshot| snapshot["provider"].as_str() == Some(provider))
        .max_by_key(|snapshot| snapshot["updated_at"].as_i64().unwrap_or(i64::MIN))
        .and_then(|snapshot| snapshot["utilization_pct"].as_f64())
        .ok_or_else(|| format!("heart has no quota snapshot for provider '{provider}'"))
}

pub fn phase_for_kind(kind: &str) -> &'static str {
    match kind {
        "approval_gate" => "approval",
        "verify" => "verifying",
        _ => "implementing",
    }
}

#[must_use]
pub fn estimated_tokens_from_payload(payload: &serde_json::Value) -> u64 {
    payload
        .get("_brain_tokens")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(100)
}

#[must_use]
pub fn requires_budget_check(kind: &NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Agent
            | NodeKind::Checkpoint
            | NodeKind::Router
            | NodeKind::Debate
            | NodeKind::Vote
            | NodeKind::Sandbox
            | NodeKind::MuscleExec
            | NodeKind::ApprovalGate
    )
}

pub fn can_continue_with_mouth_approval(
    precondition_err: &PreconditionError,
    approval_granted: bool,
) -> bool {
    if !approval_granted {
        return false;
    }
    matches!(
        precondition_err,
        PreconditionError::WorkflowBudgetExceeded { .. }
            | PreconditionError::InsufficientHeadroom { .. }
            | PreconditionError::QuotaExceeded { .. }
    )
}

pub fn substitute_fallback_model(
    current_model: &str,
    utilization_pct: f64,
    soft_quota_ceiling_pct: f64,
) -> String {
    if utilization_pct < soft_quota_ceiling_pct {
        return current_model.to_string();
    }
    match current_model {
        "claude-3-7-sonnet" | "claude-3-5-sonnet" => "claude-3-5-haiku".to_string(),
        "gpt-4o" => "gpt-4o-mini".to_string(),
        "gemini-1.5-pro" | "gemini-2.0-pro" => "gemini-2.0-flash".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn phase_mapping() {
        assert_eq!(phase_for_kind("approval_gate"), "approval");
        assert_eq!(phase_for_kind("agent"), "implementing");
    }

    #[test]
    fn tokens_from_payload() {
        assert_eq!(estimated_tokens_from_payload(&json!({})), 100);
        assert_eq!(
            estimated_tokens_from_payload(&json!({ "_brain_tokens": 900 })),
            900
        );
    }

    #[test]
    fn requires_check_for_agent_not_fork() {
        assert!(requires_budget_check(&NodeKind::Agent));
        assert!(!requires_budget_check(&NodeKind::Fork));
    }

    #[tokio::test]
    async fn evaluate_preconditions_workflow_ceiling_exceeded() {
        let gate = BudgetGate::disabled();
        let err = gate
            .evaluate_step_preconditions("plan", &[], 15.50, Some(15.00))
            .await
            .unwrap_err();
        assert_eq!(
            err,
            PreconditionError::WorkflowBudgetExceeded {
                current: 15.50,
                max: 15.00
            }
        );
    }

    fn http_response(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    async fn heart_answering(response: String) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 4096];
            let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut request).await;
            tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                .await
                .unwrap();
        });
        format!("http://{addr}")
    }

    fn gate_for(heart_url: String) -> BudgetGate {
        BudgetGate {
            heart_url,
            heart_bin: String::new(),
            enabled: true,
            client: reqwest::Client::new(),
            freeze_offenses: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    async fn check_against(response: String) -> Result<(), BudgetGateError> {
        gate_for(heart_answering(response).await)
            .check_node("agent", 100)
            .await
    }

    #[tokio::test]
    async fn node_is_denied_when_heart_is_unreachable() {
        let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = closed.local_addr().unwrap();
        drop(closed);
        let err = gate_for(format!("http://{addr}"))
            .check_node("agent", 100)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            BudgetGateError::Unavailable { ref reason } if reason.starts_with("heart unreachable")
        ));
    }

    #[tokio::test]
    async fn node_is_denied_when_heart_returns_an_http_error() {
        let err = check_against(http_response("500 Internal Server Error", "{}"))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            BudgetGateError::Unavailable { ref reason } if reason.starts_with("heart returned HTTP 500")
        ));
    }

    #[tokio::test]
    async fn node_is_denied_when_heart_returns_an_unparseable_body() {
        let err = check_against(http_response("200 OK", "not json"))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            BudgetGateError::Unavailable { ref reason } if reason.starts_with("heart response unparseable")
        ));
    }

    #[tokio::test]
    async fn node_is_denied_when_heart_reports_an_error_response() {
        let err = check_against(http_response("200 OK", r#"{"ok":false,"error":"db"}"#))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            BudgetGateError::Unavailable { ref reason } if reason == "heart returned an error response"
        ));
    }

    #[tokio::test]
    async fn node_is_denied_when_the_response_has_no_decision() {
        let err = check_against(http_response("200 OK", r#"{"ok":true}"#))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            BudgetGateError::Unavailable { ref reason } if reason == "heart response has no decision"
        ));
    }

    #[tokio::test]
    async fn node_is_allowed_when_heart_approves_the_spend() {
        let approved = r#"{"ok":true,"decision":{"allowed":true,"frozen":false,"reason":""}}"#;
        assert!(
            check_against(http_response("200 OK", approved))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn headroom_check_fails_closed_when_heart_cannot_be_reached() {
        let gate = BudgetGate {
            heart_url: String::new(),
            heart_bin: "/nonexistent/agent-heart".into(),
            enabled: true,
            client: reqwest::Client::new(),
            freeze_offenses: Arc::new(Mutex::new(std::collections::HashMap::new())),
        };
        let preconditions = vec![crate::workflow::StepPrecondition {
            check: "budget_headroom".into(),
            min_remaining_usd: Some(5.00),
            provider: None,
            window: None,
            max_utilization_pct: None,
        }];
        let result = gate
            .evaluate_step_preconditions("plan", &preconditions, 0.0, None)
            .await;
        assert!(matches!(
            result,
            Err(PreconditionError::BudgetUnavailable { .. })
        ));
    }

    #[test]
    fn quota_is_read_from_the_latest_snapshot_for_the_provider() {
        let snapshots = json!([
            {"provider": "anthropic", "utilization_pct": 40.0, "updated_at": 100},
            {"provider": "anthropic", "utilization_pct": 72.5, "updated_at": 200},
            {"provider": "openai", "utilization_pct": 99.0, "updated_at": 300}
        ]);
        assert_eq!(
            quota_utilization_from(&snapshots, "anthropic").unwrap(),
            72.5
        );
    }

    #[test]
    fn quota_for_an_untracked_provider_fails_closed() {
        let snapshots = json!([{"provider": "openai", "utilization_pct": 10.0, "updated_at": 1}]);
        assert!(quota_utilization_from(&snapshots, "anthropic").is_err());
    }

    #[test]
    fn headroom_is_read_from_the_usage_summary() {
        assert_eq!(
            headroom_from(&json!({"remaining_budget_usd": 12.5})).unwrap(),
            12.5
        );
        assert!(headroom_from(&json!({})).is_err());
    }

    #[tokio::test]
    async fn quota_check_fails_closed_when_heart_cannot_be_reached() {
        let gate = BudgetGate {
            heart_url: String::new(),
            heart_bin: "/nonexistent/agent-heart".into(),
            enabled: true,
            client: reqwest::Client::new(),
            freeze_offenses: Arc::new(Mutex::new(std::collections::HashMap::new())),
        };
        let preconditions = vec![crate::workflow::StepPrecondition {
            check: "quota_available".into(),
            min_remaining_usd: None,
            provider: Some("anthropic".into()),
            window: Some("5h_rolling".into()),
            max_utilization_pct: Some(85.0),
        }];
        let result = gate
            .evaluate_step_preconditions("plan", &preconditions, 0.0, None)
            .await;
        assert!(matches!(
            result,
            Err(PreconditionError::QuotaUnavailable { .. })
        ));
    }

    #[test]
    fn test_workflow_continuing_when_approval_granted_via_agent_mouth() {
        let budget_err = PreconditionError::WorkflowBudgetExceeded {
            current: 16.00,
            max: 15.00,
        };
        // Unapproved -> cannot continue
        assert!(!can_continue_with_mouth_approval(&budget_err, false));

        // Approved via agent-mouth -> workflow continues
        assert!(can_continue_with_mouth_approval(&budget_err, true));

        let quota_err = PreconditionError::QuotaExceeded {
            node: "heavy-agent".into(),
            provider: "anthropic".into(),
            utilization: 92.0,
            max_allowed: 85.0,
        };
        assert!(can_continue_with_mouth_approval(&quota_err, true));
    }

    #[test]
    fn test_fallback_model_substitution_when_soft_quota_ceiling_is_hit() {
        // Below ceiling (75% < 85%) -> keep primary model
        assert_eq!(
            substitute_fallback_model("claude-3-7-sonnet", 75.0, 85.0),
            "claude-3-7-sonnet"
        );
        assert_eq!(substitute_fallback_model("gpt-4o", 60.0, 80.0), "gpt-4o");

        // At or above soft quota ceiling (88% >= 85%) -> substitute lighter fallback
        assert_eq!(
            substitute_fallback_model("claude-3-7-sonnet", 88.0, 85.0),
            "claude-3-5-haiku"
        );
        assert_eq!(
            substitute_fallback_model("gpt-4o", 85.0, 80.0),
            "gpt-4o-mini"
        );
        assert_eq!(
            substitute_fallback_model("gemini-1.5-pro", 90.0, 85.0),
            "gemini-2.0-flash"
        );
    }
}
