use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};

use crate::{
    host::{ProgramTestCase, workspace},
    program::zkvm_interface::{Accelerator, Outcome, Vector, Vectors, ZkvmInterfaceProgram},
};

/// Encodes `bytes` as a `0x`-prefixed lowercase hex string.
fn to_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// Decodes a `0x`-prefixed hex string, or `None` when `text` is not one.
pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    hex::decode(text.strip_prefix("0x")?).ok()
}

/// Test case for the vectors of one accelerator. The guest checks each outcome in place, so a
/// mismatch fails the execution.
pub type ZkvmInterfaceTestCase = ProgramTestCase<ZkvmInterfaceProgram>;

/// One fixture file, holding every recorded vector of a single [`Accelerator`].
#[derive(Debug, Deserialize, Serialize)]
pub struct Fixture {
    /// Name of the accelerator these vectors run.
    pub accelerator: String,
    /// Recorded vectors, in the order the guest runs them.
    pub vectors: Vec<FixtureVector>,
}

/// One recorded call in its on-disk form, with every byte string `0x`-prefixed hex.
#[derive(Debug, Deserialize, Serialize)]
pub struct FixtureVector {
    /// Unpadded arguments, in the order the `revm::precompile::Crypto` method takes them. The
    /// by-value integers come last.
    pub inputs: Vec<String>,
    /// Reference output buffer, and `null` when the call failed.
    pub output: Option<String>,
    /// `0` when the reference call succeeded, `-1` when it failed.
    pub status: i32,
}

impl Fixture {
    /// Fixture holding `vectors`, which must all belong to `accelerator`.
    pub fn new(accelerator: Accelerator, vectors: &[Vector]) -> Self {
        Self {
            accelerator: accelerator.to_string(),
            vectors: vectors
                .iter()
                .map(|vector| {
                    assert_eq!(vector.accelerator, accelerator);
                    FixtureVector {
                        inputs: vector.inputs.iter().map(|input| to_hex(input)).collect(),
                        output: vector.expected.output.as_deref().map(to_hex),
                        status: vector.expected.status,
                    }
                })
                .collect(),
        }
    }

    fn into_vectors(self) -> Vec<Vector> {
        let accelerator = self
            .accelerator
            .parse()
            .unwrap_or_else(|_| panic!("{:?} is not a known accelerator", self.accelerator));
        self.vectors
            .into_iter()
            .map(|vector| Vector {
                accelerator,
                inputs: vector
                    .inputs
                    .iter()
                    .map(|input| from_hex(input).expect("input is hex"))
                    .collect(),
                expected: Outcome {
                    status: vector.status,
                    output: vector
                        .output
                        .map(|output| from_hex(&output).expect("output is hex")),
                },
            })
            .collect()
    }
}

/// Vectors of the fixture file at `path`.
fn load_fixture(path: impl AsRef<Path>) -> Vec<Vector> {
    let path = path.as_ref();
    let bytes = fs::read(path).unwrap_or_else(|err| panic!("reading {path:?}: {err}"));
    let fixture: Fixture =
        serde_json::from_slice(&bytes).unwrap_or_else(|err| panic!("parsing {path:?}: {err}"));
    fixture.into_vectors()
}

/// Every fixture file under `directory`, ascending by accelerator name. Panics unless every
/// [`Accelerator`] has a file, so an incomplete corpus fails here rather than shrinking the test.
fn load_fixture_directory(directory: impl AsRef<Path>) -> Vec<(Accelerator, Vec<Vector>)> {
    let directory = directory.as_ref();
    let mut entries = fs::read_dir(directory)
        .unwrap_or_else(|err| panic!("reading {directory:?}: {err}"))
        .map(|entry| entry.expect("reading a directory entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .map(|path| {
            let vectors = load_fixture(&path);
            let accelerator = vectors
                .first()
                .unwrap_or_else(|| panic!("{path:?} holds no vector"))
                .accelerator;
            (accelerator, vectors)
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(accelerator, _)| <&'static str>::from(*accelerator));

    let loaded = entries
        .iter()
        .map(|(accelerator, _)| *accelerator)
        .collect::<BTreeSet<_>>();
    let missing = Accelerator::iter()
        .filter(|accelerator| !loaded.contains(accelerator))
        .map(<&'static str>::from)
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "{directory:?} has no fixture for {missing:?}"
    );
    entries
}

/// One test case per fixture file, so one guest execution covers one accelerator.
pub fn test_cases() -> Vec<ZkvmInterfaceTestCase> {
    load_fixture_directory(
        workspace()
            .join("tests")
            .join("fixtures")
            .join("zkvm_interface"),
    )
    .into_iter()
    .map(|(_, vectors)| ProgramTestCase::new(Vectors(vectors)))
    .collect()
}
