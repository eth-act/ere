use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

/// zkVM kind supported in Ere.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum zkVMKind {
    Airbender,
    Jolt,
    Miden,
    Nexus,
    OpenVM,
    Pico,
    Risc0,
    SP1,
    Ziren,
    Zisk,
}

impl zkVMKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Airbender => "airbender",
            Self::Jolt => "jolt",
            Self::Miden => "miden",
            Self::Nexus => "nexus",
            Self::OpenVM => "openvm",
            Self::Pico => "pico",
            Self::Risc0 => "risc0",
            Self::SP1 => "sp1",
            Self::Ziren => "ziren",
            Self::Zisk => "zisk",
        }
    }
}

impl From<zkVMKind> for String {
    fn from(value: zkVMKind) -> Self {
        value.as_str().to_string()
    }
}

impl FromStr for zkVMKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "airbender" => Self::Airbender,
            "jolt" => Self::Jolt,
            "miden" => Self::Miden,
            "nexus" => Self::Nexus,
            "openvm" => Self::OpenVM,
            "pico" => Self::Pico,
            "risc0" => Self::Risc0,
            "sp1" => Self::SP1,
            "ziren" => Self::Ziren,
            "zisk" => Self::Zisk,
            _ => return Err(s.to_string()),
        })
    }
}

impl TryFrom<String> for zkVMKind {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl Display for zkVMKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
