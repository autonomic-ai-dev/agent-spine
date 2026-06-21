use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_NODE_TIMEOUT_SECS: u64 = 300;

/// Declarative workflow definition loaded from YAML.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkflowDefinition {
    name: String,
    version: u32,
    start_node: String,
    /// Minimum agent-spine version required to run this workflow (e.g. "0.8.0").
    /// If set, the binary checks at parse/validate time and warns on mismatch.
    #[serde(default)]
    min_spine_version: Option<String>,
    #[serde(default)]
    nodes: Vec<WorkflowNode>,
    #[serde(default)]
    edges: Vec<WorkflowEdge>,
}

impl WorkflowDefinition {
    /// Create a workflow definition in memory.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        version: u32,
        start_node: impl Into<String>,
        nodes: Vec<WorkflowNode>,
        edges: Vec<WorkflowEdge>,
    ) -> Self {
        Self {
            name: name.into(),
            version,
            start_node: start_node.into(),
            min_spine_version: None,
            nodes,
            edges,
        }
    }

    /// Set the minimum required agent-spine version.
    #[must_use]
    pub fn with_min_spine_version(mut self, version: impl Into<String>) -> Self {
        self.min_spine_version = Some(version.into());
        self
    }

    /// Return the minimum required agent-spine version, if set.
    #[must_use]
    pub fn min_spine_version(&self) -> Option<&str> {
        self.min_spine_version.as_deref()
    }

    /// Load and parse a workflow definition from a YAML file.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, WorkflowValidationError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(|source| WorkflowValidationError::Read {
            path: path.to_path_buf(),
            source,
        })?;

        serde_yaml::from_str(&content).map_err(|source| WorkflowValidationError::Parse { source })
    }

    /// Parse a workflow definition from YAML text.
    pub fn from_yaml(content: &str) -> Result<Self, WorkflowValidationError> {
        serde_yaml::from_str(content).map_err(|source| WorkflowValidationError::Parse { source })
    }

    /// Validate the workflow as a state machine.
    pub fn validate(self) -> Result<ValidatedWorkflow, WorkflowValidationError> {
        if self.name.trim().is_empty() {
            return Err(WorkflowValidationError::EmptyWorkflowName);
        }

        if self.version == 0 {
            return Err(WorkflowValidationError::InvalidVersion { version: 0 });
        }

        if let Some(ref min_ver) = self.min_spine_version {
            check_spine_version(min_ver)?;
        }

        if self.nodes.is_empty() {
            return Err(WorkflowValidationError::MissingNodes);
        }

        let mut node_indexes = HashMap::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            let name = node.name.trim();
            if name.is_empty() {
                return Err(WorkflowValidationError::EmptyNodeName { index });
            }

            if let Some(ref rp) = node.retry_policy {
                if rp.max_attempts == 0 {
                    return Err(WorkflowValidationError::InvalidRetryPolicy {
                        node: name.to_owned(),
                        detail: "max_attempts must be greater than 0".to_owned(),
                    });
                }
                if rp.backoff_ms == 0 {
                    return Err(WorkflowValidationError::InvalidRetryPolicy {
                        node: name.to_owned(),
                        detail: "backoff_ms must be greater than 0".to_owned(),
                    });
                }
            }

            if node_indexes.insert(name.to_owned(), index).is_some() {
                return Err(WorkflowValidationError::DuplicateNodeName {
                    name: name.to_owned(),
                });
            }
        }

        if !node_indexes.contains_key(&self.start_node) {
            return Err(WorkflowValidationError::MissingStartNode {
                name: self.start_node,
            });
        }

        for edge in &self.edges {
            if !node_indexes.contains_key(edge.from()) {
                return Err(WorkflowValidationError::UnknownNode {
                    name: edge.from().to_owned(),
                });
            }
            if !node_indexes.contains_key(edge.to()) {
                return Err(WorkflowValidationError::UnknownNode {
                    name: edge.to().to_owned(),
                });
            }
        }

        Ok(ValidatedWorkflow { definition: self })
    }

    /// Return the workflow name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the workflow version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Return the start node.
    #[must_use]
    pub fn start_node(&self) -> &str {
        &self.start_node
    }

    /// Return the declared nodes.
    #[must_use]
    pub fn nodes(&self) -> &[WorkflowNode] {
        &self.nodes
    }

    /// Return the declared edges.
    #[must_use]
    pub fn edges(&self) -> &[WorkflowEdge] {
        &self.edges
    }
}

/// A validated workflow state machine.
#[derive(Clone, Debug, PartialEq)]
pub struct ValidatedWorkflow {
    definition: WorkflowDefinition,
}

impl ValidatedWorkflow {
    /// Return the validated workflow definition.
    #[must_use]
    pub const fn definition(&self) -> &WorkflowDefinition {
        &self.definition
    }
}

/// Policy for handling transient node failures.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub backoff_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff_ms: 100,
        }
    }
}

/// Configuration for a Debate node — spawns a coder and a critic in alternating rounds.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DebateConfig {
    /// Maximum number of debate rounds (each round = coder + critic).
    #[serde(default = "default_max_rounds")]
    pub max_rounds: u32,
    /// System prompt for the coder role.
    #[serde(default = "default_coder_prompt")]
    pub coder_prompt: String,
    /// System prompt for the critic role.
    #[serde(default = "default_critic_prompt")]
    pub critic_prompt: String,
}

fn default_max_rounds() -> u32 {
    3
}
fn default_coder_prompt() -> String {
    "You are an expert engineer. Implement the requested changes accurately and thoroughly.".into()
}
fn default_critic_prompt() -> String {
    "You are a senior reviewer. Identify bugs, edge cases, security issues, and design problems in the code. Be thorough and specific.".into()
}

impl Default for DebateConfig {
    fn default() -> Self {
        Self {
            max_rounds: 3,
            coder_prompt: default_coder_prompt(),
            critic_prompt: default_critic_prompt(),
        }
    }
}

/// Configuration for a Vote node — runs the same prompt N times and picks
/// the best output by majority vote or lowest validation error.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VoteConfig {
    /// Number of parallel voting runs.
    #[serde(default = "default_vote_count")]
    pub count: u32,
    /// Temperature for the runs (> 0 to introduce diversity).
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    /// Select by majority vote (true) or lowest validation error (false).
    #[serde(default = "default_vote_method")]
    pub majority_vote: bool,
}

fn default_vote_count() -> u32 {
    3
}
fn default_temperature() -> f64 {
    0.7
}
fn default_vote_method() -> bool {
    true
}

impl Default for VoteConfig {
    fn default() -> Self {
        Self {
            count: 3,
            temperature: 0.7,
            majority_vote: true,
        }
    }
}

/// Configuration for a Sandbox node — runs code in an isolated environment.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SandboxConfig {
    /// Docker image to use for the sandbox.
    #[serde(default = "default_sandbox_image")]
    pub image: String,
    /// Timeout in seconds for sandbox execution.
    #[serde(default = "default_sandbox_timeout")]
    pub timeout_secs: u64,
}

fn default_sandbox_image() -> String {
    "ubuntu:24.04".into()
}
fn default_sandbox_timeout() -> u64 {
    60
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            image: "ubuntu:24.04".into(),
            timeout_secs: 60,
        }
    }
}

/// A node in the declarative workflow graph.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkflowNode {
    name: String,
    kind: NodeKind,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    retry_policy: Option<RetryPolicy>,
    /// Optional model escalation — after retries are exhausted, re-run with this
    /// model identifier (e.g. "claude-4-opus", "gpt-5"). Injected into payload.
    #[serde(default)]
    escalation_model: Option<String>,
    /// Debate node configuration (only meaningful for Debate kind).
    #[serde(default)]
    debate_config: Option<DebateConfig>,
    /// Vote node configuration (only meaningful for Vote kind).
    #[serde(default)]
    vote_config: Option<VoteConfig>,
    /// Sandbox node configuration (only meaningful for Sandbox kind).
    #[serde(default)]
    sandbox_config: Option<SandboxConfig>,
    /// Per-node execution timeout in seconds (`timeout_s` alias supported in YAML).
    #[serde(default, alias = "timeout_s")]
    timeout_secs: Option<u64>,
}

impl WorkflowNode {
    /// Create an agent node.
    #[must_use]
    pub fn agent(name: impl Into<String>) -> Self {
        Self::new(name, NodeKind::Agent)
    }

    /// Create an approval gate node.
    #[must_use]
    pub fn approval_gate(name: impl Into<String>) -> Self {
        Self::new(name, NodeKind::ApprovalGate)
    }

    /// Create a fork node — fans out into parallel branches.
    #[must_use]
    pub fn fork(name: impl Into<String>) -> Self {
        Self::new(name, NodeKind::Fork)
    }

    /// Create a join node — barrier for parallel branch convergence.
    #[must_use]
    pub fn join(name: impl Into<String>) -> Self {
        Self::new(name, NodeKind::Join)
    }

    /// Create a router node — injects state variables for dynamic branching.
    #[must_use]
    pub fn router(name: impl Into<String>) -> Self {
        Self::new(name, NodeKind::Router)
    }

    /// Create a debate node — alternates coder and critic rounds.
    #[must_use]
    pub fn debate(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: NodeKind::Debate,
            description: None,
            retry_policy: None,
            escalation_model: None,
            debate_config: Some(DebateConfig::default()),
            vote_config: None,
            sandbox_config: None,
            timeout_secs: None,
        }
    }

    /// Create a vote node — runs same prompt N times, picks best.
    #[must_use]
    pub fn vote(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: NodeKind::Vote,
            description: None,
            retry_policy: None,
            escalation_model: None,
            debate_config: None,
            vote_config: Some(VoteConfig::default()),
            sandbox_config: None,
            timeout_secs: None,
        }
    }

    /// Create a sandbox node — runs in isolated Docker environment.
    #[must_use]
    pub fn sandbox(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: NodeKind::Sandbox,
            description: None,
            retry_policy: None,
            escalation_model: None,
            debate_config: None,
            vote_config: None,
            sandbox_config: Some(SandboxConfig::default()),
            timeout_secs: None,
        }
    }

    /// Create a hydration node — gathers static context before agent execution.
    #[must_use]
    pub fn hydrate(name: impl Into<String>) -> Self {
        Self::new(name, NodeKind::Hydrate)
    }

    /// Create a node with the given kind.
    #[must_use]
    pub fn new(name: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            name: name.into(),
            kind,
            description: None,
            retry_policy: None,
            escalation_model: None,
            debate_config: None,
            vote_config: None,
            sandbox_config: None,
            timeout_secs: None,
        }
    }

    /// Attach a human-readable description to the node.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Return the node's retry policy.
    #[must_use]
    pub fn retry_policy(&self) -> RetryPolicy {
        self.retry_policy.clone().unwrap_or_default()
    }

    /// Per-node timeout in seconds (sandbox nodes fall back to `sandbox_config.timeout_secs`).
    #[must_use]
    pub fn timeout_secs(&self) -> u64 {
        if let Some(t) = self.timeout_secs {
            return t;
        }
        if self.kind == NodeKind::Sandbox {
            return self
                .sandbox_config
                .as_ref()
                .map(|c| c.timeout_secs)
                .unwrap_or(60);
        }
        DEFAULT_NODE_TIMEOUT_SECS
    }

    /// Return the node name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the node kind.
    #[must_use]
    pub const fn kind(&self) -> &NodeKind {
        &self.kind
    }

    /// Return the node description.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// Return the escalation model, if set.
    #[must_use]
    pub fn escalation_model(&self) -> Option<&str> {
        self.escalation_model.as_deref()
    }

    /// Return the debate config (only meaningful for Debate nodes).
    #[must_use]
    pub fn debate_config(&self) -> Option<&DebateConfig> {
        self.debate_config.as_ref()
    }

    /// Return the vote config (only meaningful for Vote nodes).
    #[must_use]
    pub fn vote_config(&self) -> Option<&VoteConfig> {
        self.vote_config.as_ref()
    }

    /// Return the sandbox config (only meaningful for Sandbox nodes).
    #[must_use]
    pub fn sandbox_config(&self) -> Option<&SandboxConfig> {
        self.sandbox_config.as_ref()
    }
}

/// The role a node plays in the workflow.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Agent,
    Checkpoint,
    Verify,
    ApprovalGate,
    /// Fan-out node — splits into N parallel branches, one per outgoing edge.
    /// All branches converge at a downstream `Join` node.
    Fork,
    /// Barrier node — waits for all branches of the corresponding `Fork`
    /// to complete before proceeding.
    Join,
    /// Injects state variables into the payload for dynamic branching.
    /// The agent returns variables used in conditional edge expressions.
    Router,
    /// Alternates between coder and critic agents in iterative rounds.
    /// Each round: coder produces output, critic reviews, feedback passed
    /// back to coder. Stops after configurable max rounds or critic approval.
    Debate,
    /// Runs the same prompt N times with temperature > 0 for diversity.
    /// Selects the best output by majority vote or lowest validation error.
    Vote,
    /// Spins up an ephemeral sandbox (Docker container) where the agent
    /// writes and executes scratchpad scripts to verify behavior.
    Sandbox,
    /// Gathers static context from the environment (git diff, test output,
    /// lint results, file contents) and injects raw data into the payload
    /// before the agent executes.
    Hydrate,
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent => write!(f, "agent"),
            Self::Checkpoint => write!(f, "checkpoint"),
            Self::Verify => write!(f, "verify"),
            Self::ApprovalGate => write!(f, "approval_gate"),
            Self::Fork => write!(f, "fork"),
            Self::Join => write!(f, "join"),
            Self::Router => write!(f, "router"),
            Self::Debate => write!(f, "debate"),
            Self::Vote => write!(f, "vote"),
            Self::Sandbox => write!(f, "sandbox"),
            Self::Hydrate => write!(f, "hydrate"),
        }
    }
}

/// A directed edge between two workflow nodes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkflowEdge {
    from: String,
    to: String,
    /// Optional condition expression evaluated against the current payload.
    /// If the condition evaluates to `false`, the edge is skipped at routing time.
    /// Format: `path.to.field <operator> <value>` e.g. `state.task_type == "frontend"`
    #[serde(default)]
    condition: Option<String>,
}

impl WorkflowEdge {
    /// Create a directed edge between two nodes.
    #[must_use]
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            condition: None,
        }
    }

    /// Create a conditional edge.
    #[must_use]
    pub fn conditional(
        from: impl Into<String>,
        to: impl Into<String>,
        condition: impl Into<String>,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            condition: Some(condition.into()),
        }
    }

    /// Return the source node.
    #[must_use]
    pub fn from(&self) -> &str {
        &self.from
    }

    /// Return the destination node.
    #[must_use]
    pub fn to(&self) -> &str {
        &self.to
    }

    /// Return the optional condition expression.
    #[must_use]
    pub fn condition(&self) -> Option<&str> {
        self.condition.as_deref()
    }
}

#[derive(Debug, Error)]
pub enum WorkflowValidationError {
    #[error("workflow name must not be empty")]
    EmptyWorkflowName,
    #[error("invalid retry policy for node '{node}': {detail}")]
    InvalidRetryPolicy { node: String, detail: String },
    #[error("workflow version must be greater than zero")]
    InvalidVersion { version: u32 },
    #[error("workflow must declare at least one node")]
    MissingNodes,
    #[error("workflow node name must not be empty at index {index}")]
    EmptyNodeName { index: usize },
    #[error("duplicate workflow node name: {name}")]
    DuplicateNodeName { name: String },
    #[error("start_node references unknown node: {name}")]
    MissingStartNode { name: String },
    #[error("workflow edge references unknown node: {name}")]
    UnknownNode { name: String },
    #[error("workflow requires agent-spine >= {required}, but running {current}")]
    SpineVersionTooOld { required: String, current: String },
    #[error("failed to read workflow definition from {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse workflow definition: {source}")]
    Parse {
        #[source]
        source: serde_yaml::Error,
    },
}

/// Compare the running agent-spine version against a required minimum.
///
/// Version strings are split on `.` and compared component-wise.
/// Returns `Ok(())` if the running version is >= required.
fn check_spine_version(required: &str) -> Result<(), WorkflowValidationError> {
    let current = env!("CARGO_PKG_VERSION");
    let req_parts: Vec<u32> = required.split('.').filter_map(|p| p.parse().ok()).collect();
    let cur_parts: Vec<u32> = current.split('.').filter_map(|p| p.parse().ok()).collect();

    for i in 0..req_parts.len().max(cur_parts.len()) {
        let req = req_parts.get(i).copied().unwrap_or(0);
        let cur = cur_parts.get(i).copied().unwrap_or(0);
        if cur > req {
            return Ok(());
        }
        if cur < req {
            return Err(WorkflowValidationError::SpineVersionTooOld {
                required: required.to_owned(),
                current: current.to_owned(),
            });
        }
    }
    Ok(())
}
