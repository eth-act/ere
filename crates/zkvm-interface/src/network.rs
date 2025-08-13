use serde::{Deserialize, Serialize};

/// Configuration for network-based proving
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct NetworkProverConfig {
    #[cfg_attr(feature = "clap", arg(long))]
    /// The endpoint URL of the prover network service
    pub endpoint: String,

    #[cfg_attr(feature = "clap", arg(long))]
    /// Optional API key for authentication
    pub api_key: Option<String>,
}

#[cfg(feature = "clap")]
impl NetworkProverConfig {
    pub fn to_args(&self) -> Vec<&str> {
        core::iter::once(["--endpoint", self.endpoint.as_str()])
            .chain(self.api_key.as_deref().map(|val| ["--api-key", val]))
            .flatten()
            .collect()
    }
}
