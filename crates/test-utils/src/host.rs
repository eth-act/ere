use crate::program::{Program, ProgramInput};
use ere_io_serde::IoSerde;
use ere_zkvm_interface::zkvm::{ProofKind, PublicValues, zkVM};
use sha2::Digest;
use std::{marker::PhantomData, path::PathBuf};

fn workspace() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

pub fn testing_guest_directory(zkvm_name: &str, program: &str) -> PathBuf {
    workspace().join("tests").join(zkvm_name).join(program)
}

pub fn run_zkvm_execute(zkvm: &impl zkVM, test_case: &impl TestCase) -> PublicValues {
    let (public_values, _report) = zkvm
        .execute(&test_case.serialized_input())
        .expect("execute should not fail with valid input");

    test_case.assert_output(&public_values);

    public_values
}

pub fn run_zkvm_prove(zkvm: &impl zkVM, test_case: &impl TestCase) -> PublicValues {
    let (prover_public_values, proof, _report) = zkvm
        .prove(&test_case.serialized_input(), ProofKind::default())
        .expect("prove should not fail with valid input");

    let verifier_public_values = zkvm
        .verify(&proof)
        .expect("verify should not fail with valid input");

    assert_eq!(prover_public_values, verifier_public_values);

    test_case.assert_output(&verifier_public_values);

    verifier_public_values
}

/// Test case for specific [`Program`] that provides serialized
/// [`Program::Input`], and is able to assert if the [`PublicValues`] returned
/// by [`zkVM`] methods is correct or not.
pub trait TestCase {
    fn serialized_input(&self) -> Vec<u8>;

    fn assert_output(&self, public_values: &[u8]);
}

/// Auto-implementation [`TestCase`] for [`ProgramInput`] that can be shared
/// between host and guest, if the guest is also written in Rust.
impl<T: ProgramInput> TestCase for T {
    fn serialized_input(&self) -> Vec<u8> {
        T::Program::io_serde().serialize(&self.clone()).unwrap()
    }

    fn assert_output(&self, public_values: &[u8]) {
        assert_eq!(
            T::Program::compute(self.clone()),
            T::Program::io_serde().deserialize(public_values).unwrap()
        )
    }
}

/// Wrapper for [`TestCase`] that asserts output to be hashed.
pub struct OutputHashedTestCase<T, D> {
    inner: T,
    _marker: PhantomData<D>,
}

impl<T, D> OutputHashedTestCase<T, D> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }
}

impl<T, D> TestCase for OutputHashedTestCase<T, D>
where
    T: ProgramInput,
    D: Digest,
{
    fn serialized_input(&self) -> Vec<u8> {
        self.inner.serialized_input()
    }

    fn assert_output(&self, public_values: &[u8]) {
        let output = T::Program::compute(self.inner.clone());
        let digest = D::digest(T::Program::io_serde().serialize(&output).unwrap());
        assert_eq!(&*digest, public_values)
    }
}
