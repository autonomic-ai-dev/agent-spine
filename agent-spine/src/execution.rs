use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable identifier for one workflow execution.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ExecutionId(Uuid);

impl ExecutionId {
    /// Create a time-ordered execution identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::str::FromStr for ExecutionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::from_str(s)?))
    }
}

impl std::fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
