//! Runs shell commands through agent-muscle so its AST guard and command timeout apply.

use std::path::Path;

use serde_json::Value;
use tokio::process::Command;

#[derive(Debug, PartialEq, Eq)]
pub struct MuscleOutput {
    pub stdout: String,
    pub exit_code: i64,
}

pub async fn run(command: &str, cwd: Option<&Path>) -> Result<MuscleOutput, String> {
    let cwd_arg = cwd.map(|dir| dir.display().to_string());
    let mut args = vec!["run", "--progress", "quiet"];
    if let Some(dir) = &cwd_arg {
        args.extend(["-c", dir.as_str()]);
    }
    args.push(command);

    let output = Command::new("agent-muscle")
        .args(&args)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|err| format!("agent-muscle unavailable: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let parsed: Value = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("unparseable agent-muscle output: {err}"))?;
    Ok(MuscleOutput {
        stdout: parsed["stdout"].as_str().unwrap_or_default().to_string(),
        exit_code: parsed["exit_code"].as_i64().unwrap_or(-1),
    })
}
