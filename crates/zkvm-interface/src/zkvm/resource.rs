use serde::{Deserialize, Deserializer, Serialize, de::Unexpected};
use serde_untagged::UntaggedEnumVisitor;

/// Configuration for network-based proving
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "clap", derive(clap::Args))]
pub struct NetworkProverConfig {
    /// The endpoint URL of the prover network service
    #[cfg_attr(feature = "clap", arg(long))]
    pub endpoint: String,
    /// Optional API key for authentication
    #[cfg_attr(feature = "clap", arg(long))]
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

impl Serialize for ProverResourceType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Cpu => "cpu".serialize(serializer),
            Self::Gpu => "gpu".serialize(serializer),
            Self::Network(config) => config.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ProverResourceType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .string(|resource| match resource {
                "cpu" => Ok(Self::Cpu),
                "gpu" => Ok(Self::Gpu),
                _ => Err(serde::de::Error::invalid_value(
                    Unexpected::Str(resource),
                    &r#""cpu" or "gpu""#,
                )),
            })
            .map(|map| map.deserialize().map(Self::Network))
            .deserialize(deserializer)
    }
}

#[cfg(test)]
mod test {
    use crate::zkvm::resource::ProverResourceType;
    use core::fmt::Debug;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct Config {
        resources: Vec<ProverResourceType>,
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
resources = ["cpu", "gpu", { endpoint = "http://localhost:3000" }]
    "#;
        test_round_trip(TOML, toml::to_string, toml::from_str);
    }

    #[test]
    fn test_round_trip_yaml() {
        const YAML: &str = r#"
resources:
- cpu
- gpu
- endpoint: http://localhost:3000
  api-key: null
"#;
        test_round_trip(YAML, serde_yaml::to_string, serde_yaml::from_str);
    }

    #[test]
    fn test_round_trip_json() {
        const JSON: &str = r#"
{
  "resources": [
    "cpu",
    "gpu",
    {
      "endpoint": "",
      "api-key": null
    }
  ]
}
"#;
        test_round_trip(JSON, serde_json::to_string_pretty, serde_json::from_str);
    }
}
