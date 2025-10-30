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

/// ResourceType specifies what resource will be used to create the proofs.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "clap", derive(clap::Subcommand))]
pub enum ProverResourceType {
    #[default]
    Cpu,
    Gpu,
    /// Use a remote prover network
    Network(NetworkProverConfig),
}

#[cfg(feature = "clap")]
impl ProverResourceType {
    pub fn to_args(&self) -> Vec<&str> {
        match self {
            Self::Cpu => vec!["cpu"],
            Self::Gpu => vec!["gpu"],
            Self::Network(config) => core::iter::once("network")
                .chain(config.to_args())
                .collect(),
        }
    }
}
