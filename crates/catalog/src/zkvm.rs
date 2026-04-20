use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};

/// zkVM kind supported in Ere.
#[allow(non_camel_case_types)]
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
    serialize_all = "lowercase",
    parse_err_fn = ParseError::from,
    parse_err_ty = ParseError
)]
pub enum zkVMKind {
    Airbender,
    OpenVM,
    Risc0,
    SP1,
    Zisk,
}

impl zkVMKind {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }

    pub fn name(&self) -> &'static str {
        self.as_str()
    }
}

impl From<zkVMKind> for String {
    fn from(value: zkVMKind) -> Self {
        value.as_str().to_string()
    }
}

impl TryFrom<String> for zkVMKind {
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
        let supported = Vec::from_iter(zkVMKind::iter().map(|k| k.as_str())).join(", ");
        write!(
            f,
            "Unsupported zkVM kind `{unsupported}`, expect one of [{supported}]",
        )
    }
}

impl Error for ParseError {}

#[cfg(test)]
mod tests {
    use crate::zkvm::{ParseError, zkVMKind};

    #[test]
    fn parse_zkvm_kind() {
        // Valid
        for (ss, kind) in [
            (["airbender", "Airbender"], zkVMKind::Airbender),
            (["openvm", "OpenVM"], zkVMKind::OpenVM),
            (["risc0", "Risc0"], zkVMKind::Risc0),
            (["sp1", "SP1"], zkVMKind::SP1),
            (["zisk", "Zisk"], zkVMKind::Zisk),
        ] {
            ss.iter().for_each(|s| assert_eq!(s.parse(), Ok(kind)));
            assert_eq!(kind.as_str(), ss[0]);
        }

        // Invalid
        assert_eq!("xxx".parse::<zkVMKind>(), Err(ParseError::from("xxx")));
        assert_eq!(
            ParseError::from("xxx").to_string(),
            "Unsupported zkVM kind `xxx`, expect one of \
                        [airbender, openvm, risc0, sp1, zisk]"
                .to_string()
        );
    }
}
