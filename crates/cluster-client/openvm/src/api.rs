//! Wire types for the Axiom Edge manager's HTTP API.

use core::{
    fmt::{self, Display, Formatter},
    slice,
};

use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

/// Identifies one program version in the deployment loadout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramRef {
    pub name: String,
    pub version: u32,
}

impl Display for ProgramRef {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}@v{}", self.name, self.version)
    }
}

/// `POST /upload_input/{proof_uuid}` request body.
///
/// Mirrors `openvm_sdk::StdIn` so the client needs no OpenVM SDK dependency.
/// Upstream pairs the input queue with a list of deferred continuation states,
/// which this client never submits, so that list stays empty and untyped.
#[derive(Debug)]
pub struct StdIn<'a> {
    buffer: &'a [u8],
}

impl<'a> StdIn<'a> {
    /// Stages `buffer` as the guest's whole input.
    pub fn from_bytes(buffer: &'a [u8]) -> Self {
        Self { buffer }
    }
}

impl Serialize for StdIn<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        const DEFERRALS: &[()] = &[];

        // Both fields are sequences upstream, so both are written as slices. An
        // array would encode as a tuple, which drops the length prefix the
        // workers rely on when decoding the staged input.
        let mut stdin = serializer.serialize_struct("StdIn", 2)?;
        stdin.serialize_field("buffer", slice::from_ref(&self.buffer))?;
        stdin.serialize_field("deferrals", DEFERRALS)?;
        stdin.end()
    }
}

/// `POST /start_proof` request body.
#[derive(Debug, Serialize)]
pub struct StartProofRequest {
    pub proof_uuid: String,
    pub program: ProgramRef,
    /// Always false, since the input is staged on the manager, which fans it
    /// out.
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
/// The failure variants carry the manager's reason, mirroring the server's
/// shape.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofStatus {
    InProgress,
    Completed,
    /// A worker reported a fatal error and the manager is draining its peers.
    /// Transient, so it still settles into `Failed`.
    Failing(String),
    Failed(String),
    /// Spelled with one l to match the manager's wire value.
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
/// The manager returns a wider record. The client reads it once a proof has
/// settled, purely for the timings.
#[derive(Debug, Deserialize)]
pub struct ProofStateResponse {
    /// Wall-clock from job admission to completion, so it covers the input
    /// fan-out as well as proving.
    #[serde(default)]
    pub e2e_latency_ms: Option<u64>,
}
