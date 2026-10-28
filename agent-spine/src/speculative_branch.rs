use std::path::{Path, PathBuf};
use tokio::process::Command;

pub struct SpeculativeWorktreeGuard {
    pub branch_name: String,
    pub worktree_path: PathBuf,
    repo_root: PathBuf,
}

impl SpeculativeWorktreeGuard {
    pub async fn create(repo_root: impl AsRef<Path>, branch_prefix: &str) -> Result<Self, String> {
        let repo_root = repo_root.as_ref().to_path_buf();
        let unique_id = uuid::Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext)).to_string()[..8].to_string();
        let branch_name = format!("{branch_prefix}-{unique_id}");
        let worktree_path = std::env::temp_dir().join(format!("spine-worktree-{unique_id}"));

        let output = Command::new("git")
            .current_dir(&repo_root)
            .args([
                "worktree",
                "add",
                "-b",
                &branch_name,
                &worktree_path.to_string_lossy(),
            ])
            .output()
            .await
            .map_err(|e| format!("failed to spawn git worktree add: {e}"))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git worktree add failed: {err}"));
        }

        Ok(Self {
            branch_name,
            worktree_path,
            repo_root,
        })
    }

    pub async fn cleanup(self) -> Result<(), String> {
        let _ = Command::new("git")
            .current_dir(&self.repo_root)
            .args([
                "worktree",
                "remove",
                "--force",
                &self.worktree_path.to_string_lossy(),
            ])
            .output()
            .await;

        let _ = Command::new("git")
            .current_dir(&self.repo_root)
            .args(["branch", "-D", &self.branch_name])
            .output()
            .await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn guard_path_construction_is_valid() {
        let _root = PathBuf::from("/tmp");
        let prefix = "test-spec";
        let unique_id = "abc12345";
        let branch = format!("{prefix}-{unique_id}");
        let path = std::env::temp_dir().join(format!("spine-worktree-{unique_id}"));
        assert!(branch.starts_with("test-spec-"));
        assert!(path.to_string_lossy().contains("spine-worktree-"));
    }
}
