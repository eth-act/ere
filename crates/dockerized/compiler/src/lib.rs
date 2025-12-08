use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

/// Compiler kind to use to compile the guest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum CompilerKind {
    /// Stock Rust compiler
    Rust,
    /// Rust compiler with customized toolchain
    RustCustomized,
    /// Go compiler with customized toolchain
    GoCustomized,
    /// Miden assembly compiler
    MidenAsm,
}

impl CompilerKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::RustCustomized => "rust-customized",
            Self::GoCustomized => "go-customized",
            Self::MidenAsm => "miden-asm",
        }
    }
}

impl From<CompilerKind> for String {
    fn from(value: CompilerKind) -> Self {
        value.as_str().to_string()
    }
}

impl FromStr for CompilerKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "rust" => Self::Rust,
            "rust-customized" => Self::RustCustomized,
            "go-customized" => Self::GoCustomized,
            "miden-asm" => Self::MidenAsm,
            _ => return Err(format!("Unsupported compiler kind {s}")),
        })
    }
}

impl TryFrom<String> for CompilerKind {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl Display for CompilerKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
