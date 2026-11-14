use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const STREAM_NAME: &str = "AUTONOMIC";

pub const STREAM_SUBJECT_WILDCARD: &str = "autonomic.>";

pub fn default_nats_url() -> Option<String> {
    std::env::var("AUTONOMIC_NATS_URL").ok()
}

pub fn default_duplicate_window() -> Duration {
    Duration::from_secs(120)
}

pub mod subjects {
    pub const SPINE_STATE: &str = "autonomic.spine.state";
    pub const EXECUTE_SANDBOX: &str = "autonomic.execute.sandbox";
    pub const EXECUTE_RESULT: &str = "autonomic.execute.result";
    pub const SYSTEM_DLQ: &str = "system.dlq";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransitionEvent {
    pub msg_id: String,
    pub execution_id: Option<String>,
    pub workflow_name: Option<String>,
    pub event_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxExecute {
    pub msg_id: String,
    pub job_id: String,
    pub command: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub memory_mb: Option<u32>,
    #[serde(default)]
    pub cpu_cores: Option<f32>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub wasm_path: Option<String>,
    #[serde(default)]
    pub fuel_limit: Option<u64>,
    #[serde(default)]
    pub memory_limit_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub msg_id: String,
    pub job_id: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    #[serde(default)]
    pub fuel_consumed: Option<u64>,
    #[serde(default)]
    pub memory_peak_bytes: Option<usize>,
}

#[cfg(feature = "nats")]
pub async fn connect_nats() -> Result<async_nats::Client, async_nats::ConnectError> {
    let url =
        std::env::var("AUTONOMIC_NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let mut opts = async_nats::ConnectOptions::new();

    if !nats_insecure_mode() {
        let user = std::env::var("AUTONOMIC_NATS_USER").unwrap_or_default();
        let pass = std::env::var("AUTONOMIC_NATS_PASSWORD").unwrap_or_default();
        if !user.is_empty() {
            opts = opts.user_and_password(user, pass);
        }
    }

    if std::env::var("AUTONOMIC_NATS_TLS").ok().as_deref() == Some("1") {
        opts = opts.require_tls(true);
        if let Ok(ca_path) = std::env::var("AUTONOMIC_NATS_CA") {
            opts = opts.add_root_certificates(std::path::PathBuf::from(ca_path));
        }
        if let (Ok(cert_path), Ok(key_path)) = (
            std::env::var("AUTONOMIC_NATS_CERT"),
            std::env::var("AUTONOMIC_NATS_KEY"),
        ) {
            opts = opts.add_client_certificate(
                std::path::PathBuf::from(cert_path),
                std::path::PathBuf::from(key_path),
            );
        }
    }

    opts.connect(url).await
}

#[cfg(feature = "nats")]
fn nats_insecure_mode() -> bool {
    std::env::var("AUTONOMIC_NATS_INSECURE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}
