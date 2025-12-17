use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};

/// Compiler kind to use to compile the guest.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    EnumIter,
    EnumString,
    IntoStaticStr,
    Display,
)]
#[serde(into = "String", try_from = "String")]
#[strum(
    ascii_case_insensitive,
    serialize_all = "kebab-case",
    parse_err_fn = ParseError::from,
    parse_err_ty = ParseError
)]
pub enum CompilerKind {
    /// Stock Rust compiler
    Rust,
    /// Rust compiler with customized toolchain
    #[strum(serialize = "rust-customized", serialize = "RustCustomized")]
    RustCustomized,
    /// Go compiler with customized toolchain
    #[strum(serialize = "go-customized", serialize = "GoCustomized")]
    GoCustomized,
    /// Miden assembly compiler
    #[strum(serialize = "miden-asm", serialize = "MidenAsm")]
    MidenAsm,
}

impl CompilerKind {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

impl From<CompilerKind> for String {
    fn from(value: CompilerKind) -> Self {
        value.as_str().to_string()
    }
}

impl TryFrom<String> for CompilerKind {
    type Error = ParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParseError(String);

impl From<&str> for ParseError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let unsupported = &self.0;
        let supported = Vec::from_iter(CompilerKind::iter().map(|k| k.as_str())).join(", ");
        write!(
            f,
            "Unsupported compiler kind `{unsupported}`, expect one of [{supported}]",
        )
    }
}

impl Error for ParseError {}

#[cfg(test)]
mod test {
    use crate::compiler::{CompilerKind, CompilerKind::*, ParseError};

    #[test]
    fn parse_compiler_kind() {
        // Valid
        for (ss, kind) in [
            (["rust", "Rust"], Rust),
            (["rust-customized", "RustCustomized"], RustCustomized),
            (["go-customized", "GoCustomized"], GoCustomized),
            (["miden-asm", "MidenAsm"], MidenAsm),
        ] {
            ss.iter().for_each(|s| assert_eq!(s.parse(), Ok(kind)));
            assert_eq!(kind.as_str(), ss[0]);
        }

        // Invalid
        assert_eq!("xxx".parse::<CompilerKind>(), Err(ParseError::from("xxx")));
        assert_eq!(
            ParseError::from("xxx").to_string(),
            "Unsupported compiler kind `xxx`, expect one of \
                [rust, rust-customized, go-customized, miden-asm]"
                .to_string()
        );
    }
}
