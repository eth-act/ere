//! Wire types for the Axiom Edge manager's HTTP API.
//!
//! Declared locally rather than taken from the server's `protocol` crate so
//! this client stays a plain HTTP consumer with no dependency on the cluster's
//! source tree. Only the fields the client actually reads are modelled.

use serde::{Deserialize, Serialize};

/// Identifies one program version in the deployment loadout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramRef {
    pub name: String,
    pub version: u32,
}

impl core::fmt::Display for ProgramRef {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}@v{}", self.name, self.version)
    }
}

/// `POST /start_proof` request body.
#[derive(Debug, Serialize)]
pub struct StartProofRequest {
    pub proof_uuid: String,
    pub program: ProgramRef,
    /// Always `false`: the input is staged on the manager, which fans it out.
    pub input_already_uploaded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

/// `POST /cancel_proof` request body.
#[derive(Debug, Serialize)]
pub struct CancelProofRequest {
    pub proof_uuid: String,
}

/// Proof states reported by `GET /proof_state` and `GET /proof_events`.
///
/// The failure variants carry the manager's reason, so this mirrors the
/// server's shape rather than pairing a bare status with a separate error
/// field.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofStatus {
    InProgress,
    Completed,
    /// A worker reported a fatal error and the manager is draining its peers.
    /// Transient, so it still settles into `Failed`.
    Failing(String),
    Failed(String),
    Canceled,
}

impl ProofStatus {
    /// Whether this is the proof's last status.
    pub fn is_settled(&self) -> bool {
        matches!(
            self,
            ProofStatus::Completed | ProofStatus::Failed(_) | ProofStatus::Canceled
        )
    }
}

/// `GET /proof_state/{proof_uuid}` response.
///
/// The manager returns a wider record than this. The client reads it once a
/// proof has settled, purely for the timings, since the status arrives over
/// `GET /proof_events`.
#[derive(Debug, Deserialize)]
pub struct ProofStateResponse {
    /// Wall-clock from job admission to completion, so it covers the input
    /// fan-out to the workers as well as proving itself. This is the boundary
    /// `ere-cluster-client-zisk` reports, so the two stay comparable.
    #[serde(default)]
    pub e2e_latency_ms: Option<u64>,
}
