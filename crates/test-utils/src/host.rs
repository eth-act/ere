use crate::program::Program;
use core::ops::Deref;
use ere_io::Io;
use ere_zkvm_interface::{
    Input,
    zkvm::{ProofKind, PublicValues, zkVM},
};
use sha2::{Digest, Sha256};
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
        .execute(&test_case.input())
        .expect("execute should not fail with valid input");

    test_case.assert_output(&public_values);

    public_values
}

pub fn run_zkvm_prove(zkvm: &impl zkVM, test_case: &impl TestCase) -> PublicValues {
    let (prover_public_values, proof, _report) = zkvm
        .prove(&test_case.input(), ProofKind::default())
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
    fn input(&self) -> Input;

    fn assert_output(&self, public_values: &[u8]);
}

/// Wrapper for [`ProgramInput`] that implements [`TestCase`].
pub struct ProgramTestCase<P: Program> {
    input: <P::Io as Io>::Input,
    _marker: PhantomData<P>,
}

impl<P: Program> ProgramTestCase<P> {
    pub fn new(input: <P::Io as Io>::Input) -> Self {
        Self {
            input,
            _marker: PhantomData,
        }
    }

    /// Wrap into [`OutputHashedProgramTestCase`] with [`Sha256`].
    pub fn into_output_sha256(self) -> impl TestCase {
        OutputHashedProgramTestCase::<_, Sha256>::new(self)
    }
}

impl<P: Program> Deref for ProgramTestCase<P> {
    type Target = <P::Io as Io>::Input;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<P: Program> TestCase for ProgramTestCase<P> {
    fn input(&self) -> Input {
        Input::new().with_prefixed_stdin(P::Io::serialize_input(&self.input).unwrap())
    }

    fn assert_output(&self, public_values: &[u8]) {
        assert_eq!(
            P::compute(self.input.clone()),
            P::Io::deserialize_output(public_values).unwrap()
        )
    }
}

/// Wrapper for [`ProgramTestCase`] that asserts output to be hashed.
pub struct OutputHashedProgramTestCase<P: Program, D> {
    test_case: ProgramTestCase<P>,
    _marker: PhantomData<D>,
}

impl<P: Program, D> OutputHashedProgramTestCase<P, D> {
    pub fn new(test_case: ProgramTestCase<P>) -> Self {
        Self {
            test_case,
            _marker: PhantomData,
        }
    }
}

impl<P, D> TestCase for OutputHashedProgramTestCase<P, D>
where
    P: Program,
    D: Digest,
{
    fn input(&self) -> Input {
        self.test_case.input()
    }

    fn assert_output(&self, public_values: &[u8]) {
        let output = P::compute(self.test_case.clone());
        let digest = D::digest(P::Io::serialize_output(&output).unwrap());
        assert_eq!(&*digest, public_values)
    }
}
