use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug)]
pub struct SandboxResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub async fn run_sandbox(
    command: &str,
    image: &str,
    timeout: Duration,
    workdir: Option<&Path>,
) -> Result<SandboxResult, String> {
    run_sandbox_hardened(command, image, timeout, workdir, 256, 1.0).await
}

pub async fn run_sandbox_hardened(
    command: &str,
    image: &str,
    timeout: Duration,
    workdir: Option<&Path>,
    memory_mb: u32,
    cpu_cores: f32,
) -> Result<SandboxResult, String> {
    let docker_args = build_docker_args(command, image, workdir, memory_mb, cpu_cores);

    let child = Command::new("docker")
        .args(&docker_args)
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn docker: {e}"))?;

    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| format!("Sandbox timed out after {}s", timeout.as_secs()))?
        .map_err(|e| format!("Docker execution failed: {e}"))?;

    Ok(SandboxResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}

fn build_docker_args(
    command: &str,
    image: &str,
    workdir: Option<&Path>,
    memory_mb: u32,
    cpu_cores: f32,
) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "-i".to_string(),
        "--network=none".to_string(),
        "--memory".to_string(),
        format!("{memory_mb}m"),
        "--cpus".to_string(),
        format!("{cpu_cores}"),
        "--pids-limit".to_string(),
        "64".to_string(),
    ];

    if let Some(dir) = workdir {
        let host_dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        args.push("-v".to_string());
        args.push(format!("{}:/workspace:ro", host_dir.display()));
        args.push("-w".to_string());
        args.push("/workspace".to_string());
    }

    args.push(image.to_string());
    args.push("sh".to_string());
    args.push("-c".to_string());
    args.push(command.to_string());
    args
}

pub fn sandbox_output_to_payload(result: SandboxResult) -> Value {
    serde_json::json!({
        "_sandbox_stdout": result.stdout,
        "_sandbox_stderr": result.stderr,
        "_sandbox_exit_code": result.exit_code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_args_include_network_and_resource_limits() {
        let args = build_docker_args("echo hi", "ubuntu:24.04", None, 256, 1.0);
        assert!(args.contains(&"--network=none".to_string()));
        assert!(args.contains(&"--memory".to_string()));
        assert!(args.contains(&"256m".to_string()));
        assert!(args.contains(&"--cpus".to_string()));
    }
}
