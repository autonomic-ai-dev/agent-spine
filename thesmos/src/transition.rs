use serde::{Deserialize, Serialize};

/// Describes a directed workflow edge.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    from: String,
    to: String,
}

impl Transition {
    /// Create a transition between two non-empty node names.
    #[must_use]
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }

    #[must_use]
    pub fn from(&self) -> &str {
        &self.from
    }

    #[must_use]
    pub fn to(&self) -> &str {
        &self.to
    }
}
