//! Internal identity boundaries. Transport IDs remain strings for compatibility.
use std::borrow::Borrow;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct BusinessActionId(String);

impl BusinessActionId {
    pub fn new(id: String) -> Self {
        Self(id)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExecutionCandidateId(String);

impl From<String> for ExecutionCandidateId {
    fn from(id: String) -> Self {
        Self(id)
    }
}
impl Borrow<str> for ExecutionCandidateId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// History eligibility and long-lived shortcut eligibility are separate decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingPolicy {
    Persistent,
    Unavailable { reason: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PersistentActionBinding {
    pub action_id: BusinessActionId,
    /// Resolve a fresh object of one of these types at invocation time.
    pub context_types: Vec<String>,
}
