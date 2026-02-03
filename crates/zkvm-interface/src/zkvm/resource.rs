use serde::{Deserialize, Serialize};
use strum::{Display, EnumDiscriminants, EnumIs, EnumIter, EnumString};

/// Configuration for remote proving
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct RemoteProverConfig {
    /// The endpoint URL of the remote prover
    #[cfg_attr(feature = "clap", arg(long))]
    pub endpoint: String,
    /// Optional API key for authentication
    #[cfg_attr(feature = "clap", arg(long))]
    pub api_key: Option<String>,
}

#[cfg(feature = "clap")]
impl RemoteProverConfig {
    pub fn to_args(&self) -> Vec<&str> {
        core::iter::once(["--endpoint", self.endpoint.as_str()])
            .chain(self.api_key.as_deref().map(|val| ["--api-key", val]))
            .flatten()
            .collect()
    }
}

/// ResourceType specifies what resource will be used to create the proofs.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, EnumDiscriminants, EnumIs,
)]
#[strum_discriminants(
    name(ProverResourceKind),
    derive(Display, EnumString, EnumIter, Hash),
    strum(serialize_all = "lowercase")
)]
#[cfg_attr(feature = "clap", derive(clap::Subcommand))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProverResource {
    #[default]
    Cpu,
    Gpu,
    /// Official proving network
    Network(RemoteProverConfig),
    /// Self-hosted proving cluster
    Cluster(RemoteProverConfig),
}

impl ProverResource {
    /// Returns [`ProverResourceKind`].
    pub fn kind(&self) -> ProverResourceKind {
        self.into()
    }
}

#[cfg(feature = "clap")]
impl ProverResource {
    pub fn to_args(&self) -> Vec<&str> {
        match self {
            Self::Cpu => vec!["cpu"],
            Self::Gpu => vec!["gpu"],
            Self::Network(config) => core::iter::once("network")
                .chain(config.to_args())
                .collect(),
            Self::Cluster(config) => core::iter::once("cluster")
                .chain(config.to_args())
                .collect(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::zkvm::resource::ProverResource;
    use core::fmt::Debug;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Config {
        resources: Vec<ProverResource>,
    }

    fn test_round_trip<'de, SE: Debug, DE: Debug>(
        config: &'de str,
        ser: impl Fn(&Config) -> Result<String, SE>,
        de: impl Fn(&'de str) -> Result<Config, DE>,
    ) {
        assert_eq!(config.trim(), ser(&de(config).unwrap()).unwrap().trim())
    }

    #[test]
    fn test_round_trip_toml() {
        const TOML: &str = r#"
[[resources]]
kind = "cpu"

[[resources]]
kind = "gpu"

[[resources]]
kind = "network"
endpoint = "http://localhost:3000"
api_key = "my_api_key"

[[resources]]
kind = "cluster"
endpoint = "http://localhost:3000"
    "#;
        test_round_trip(TOML, toml::to_string, toml::from_str);
    }

    #[test]
    fn test_round_trip_yaml() {
        const YAML: &str = r#"
resources:
- kind: cpu
- kind: gpu
- kind: network
  endpoint: http://localhost:3000
  api_key: my_api_key
- kind: cluster
  endpoint: http://localhost:3000
  api_key: null
"#;
        test_round_trip(YAML, serde_yaml::to_string, serde_yaml::from_str);
    }

    #[test]
    fn test_round_trip_json() {
        const JSON: &str = r#"
{
  "resources": [
    {
      "kind": "cpu"
    },
    {
      "kind": "gpu"
    },
    {
      "kind": "network",
      "endpoint": "http://localhost:3000",
      "api_key": "my_api_key"
    },
    {
      "kind": "cluster",
      "endpoint": "http://localhost:3000",
      "api_key": null
    }
  ]
}
"#;
        test_round_trip(JSON, serde_json::to_string_pretty, serde_json::from_str);
    }
}
