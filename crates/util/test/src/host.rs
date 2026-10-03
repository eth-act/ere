use core::{marker::PhantomData, ops::Deref};
use std::{env, fs, path::PathBuf};

use ere_codec::{Decode, Encode};
use ere_prover_core::{CostEstimation, CostProfile, Elf, Input, PublicValues, zkVMProver};
use sha2::{Digest, Sha256};

use crate::{
    codec::BincodeLegacy,
    program::{
        Program,
        basic::BasicProgram,
        zkvm_interface::{self, Accelerator},
    },
};

pub(crate) fn workspace() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.pop();
    path
}

pub fn testing_guest_directory(zkvm_name: &str, program: &str) -> PathBuf {
    workspace().join("tests").join(zkvm_name).join(program)
}

pub fn run_zkvm_execute(zkvm: &impl zkVMProver, test_case: &impl TestCase) -> PublicValues {
    let (public_values, _report) = zkvm
        .execute(&test_case.input())
        .expect("execute should not fail with valid input");

    test_case.assert_output(&public_values);

    public_values
}

pub fn run_zkvm_execute_estimated_cost(
    zkvm: &impl zkVMProver,
    test_case: &impl TestCase,
) -> PublicValues {
    let (public_values, estimation) = zkvm
        .execute_estimated_cost(&test_case.input())
        .expect("execute_estimated_cost should not fail with valid input");

    assert!(!estimation.cost.is_empty(), "cost must not be empty");

    test_case.assert_output(&public_values);

    public_values
}

pub fn run_zkvm_profile(zkvm: &impl zkVMProver, test_case: &impl TestCase) -> CostProfile {
    let input = test_case.input();
    let (public_values, profile) = zkvm
        .profile(&input)
        .expect("profile should not fail with valid input");
    let (_, estimation) = zkvm
        .execute_estimated_cost(&input)
        .expect("execute_estimated_cost should not fail with valid input");

    assert_profile(&profile, &estimation);

    test_case.assert_output(&public_values);

    profile
}

/// Checks that `profile` splits `estimation` and its peak heap, that a stack ends in the guest
/// `main`, which the guest enters, and that both peaks are nonzero.
pub fn assert_profile(profile: &CostProfile, estimation: &CostEstimation) {
    assert_eq!(
        profile.cost_estimation(),
        *estimation,
        "profile must split the estimated cost"
    );
    assert_eq!(
        stacks(profile, "heap_growth")
            .iter()
            .map(|(_, bytes)| bytes)
            .sum::<u64>(),
        profile.peak_heap_bytes,
        "heap growth must split the peak heap"
    );
    assert!(
        stacks(profile, "calls")
            .iter()
            .any(|(frames, calls)| frames.last() == Some(&"main") && *calls > 0),
        "a stack must end in the guest `main`, which the guest enters"
    );
    assert!(profile.peak_stack_bytes > 0, "peak stack must not be zero");
    assert!(profile.peak_heap_bytes > 0, "peak heap must not be zero");
}

/// Profiles the Sha256 test case of the zkVM-accelerator program, and checks that a stack holds
/// `zkvm_sha256` below `main`.
pub fn run_zkvm_profile_zkvm_interface(zkvm: &impl zkVMProver) {
    let test_case = zkvm_interface::test_cases()
        .into_iter()
        .find(|test_case| test_case.0[0].accelerator == Accelerator::Sha256)
        .unwrap();
    let profile = run_zkvm_profile(zkvm, &test_case);
    assert!(profile_stacks(&profile).iter().any(|(frames, _)| {
        frames
            .iter()
            .skip_while(|frame| **frame != "main")
            .any(|frame| *frame == "zkvm_sha256")
    }));
}

/// Frames from the root, and the cost, of each stack in `profile`.
pub fn profile_stacks(profile: &CostProfile) -> Vec<(Vec<&str>, u64)> {
    stacks(profile, "cost")
}

/// Frames from the root, and the value of the sample type `type_name`, of each stack in `profile`.
fn stacks<'a>(profile: &'a CostProfile, type_name: &str) -> Vec<(Vec<&'a str>, u64)> {
    let pprof = &profile.pprof;
    let index = pprof
        .sample_type
        .iter()
        .position(|sample_type| pprof.string_table[sample_type.r#type as usize] == type_name)
        .unwrap_or_else(|| panic!("profile must have the sample type `{type_name}`"));
    let frame = |location_id: &u64| {
        let function_id = pprof.location[*location_id as usize - 1].line[0].function_id;
        let name = pprof.function[function_id as usize - 1].name;
        pprof.string_table[name as usize].as_str()
    };
    pprof
        .sample
        .iter()
        .map(|sample| {
            let frames = sample.location_id.iter().rev().map(frame).collect();
            (frames, sample.value[index] as u64)
        })
        .collect()
}

pub fn run_zkvm_prove(zkvm: &impl zkVMProver, test_case: &impl TestCase) -> PublicValues {
    let (prover_public_values, proof, _report) = zkvm
        .prove(&test_case.input())
        .expect("prove should not fail with valid input");

    let verifier_public_values = zkvm
        .verify(&proof)
        .expect("verify should not fail with valid input");

    assert_eq!(prover_public_values, verifier_public_values);

    test_case.assert_output(&verifier_public_values);

    if env::var_os("ERE_GENERATE_VERIFIER_FIXTURE").is_some() {
        let fixture_dir =
            workspace().join(format!("crates/verifier/{}/tests/fixtures", zkvm.name()));
        let proof = proof.encode_to_vec().unwrap();
        let program_vk = zkvm.program_vk().encode_to_vec().unwrap();
        fs::write(fixture_dir.join("proof.bin"), proof).unwrap();
        fs::write(fixture_dir.join("program_vk.bin"), program_vk).unwrap();
        fs::write(fixture_dir.join("public_values.bin"), &prover_public_values).unwrap();
    }

    verifier_public_values
}

/// Switches `zkvm` between `basic_elf` and `zkvm_interface_elf` and back, checks that each program
/// keeps its program VK, and runs a test case of each, proving it when `prove` is set.
pub fn run_zkvm_switchable(
    zkvm: &mut impl zkVMProver,
    basic_elf: Elf,
    zkvm_interface_elf: Elf,
    prove: bool,
) {
    zkvm.setup(basic_elf.clone()).unwrap();
    let basic_vk = zkvm.program_vk().encode_to_vec().unwrap();
    zkvm.setup(zkvm_interface_elf.clone()).unwrap();
    let zkvm_interface_vk = zkvm.program_vk().encode_to_vec().unwrap();

    zkvm.setup(basic_elf).unwrap();
    assert_eq!(zkvm.program_vk().encode_to_vec().unwrap(), basic_vk);
    let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
    if prove {
        run_zkvm_prove(zkvm, &test_case);
    } else {
        run_zkvm_execute(zkvm, &test_case);
    }

    zkvm.setup(zkvm_interface_elf).unwrap();
    assert_eq!(
        zkvm.program_vk().encode_to_vec().unwrap(),
        zkvm_interface_vk
    );
    let test_case = zkvm_interface::test_cases()
        .into_iter()
        .find(|test_case| test_case.0[0].accelerator == Accelerator::Sha256)
        .unwrap();
    if prove {
        run_zkvm_prove(zkvm, &test_case);
    } else {
        run_zkvm_execute(zkvm, &test_case);
    }
}

/// Test case for specific [`Program`] that provides serialized
/// [`Program::Input`], and is able to assert if the [`PublicValues`] returned
/// by [`zkVMProver`] methods is correct or not.
pub trait TestCase {
    fn input(&self) -> Input;

    fn assert_output(&self, public_values: &[u8]);
}

/// Wrapper for [`ProgramInput`] that implements [`TestCase`].
pub struct ProgramTestCase<P: Program> {
    input: P::Input,
    _marker: PhantomData<P>,
}

impl<P: Program> ProgramTestCase<P> {
    pub fn new(input: P::Input) -> Self {
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
    type Target = P::Input;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<P: Program> TestCase for ProgramTestCase<P> {
    fn input(&self) -> Input {
        Input::new().with_stdin(self.input.encode_to_vec().unwrap())
    }

    fn assert_output(&self, public_values: &[u8]) {
        assert_eq!(
            P::compute(self.input.clone()),
            P::Output::decode_from_slice(public_values).unwrap()
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
        let digest = D::digest(output.encode_to_vec().unwrap());
        assert_eq!(&*digest, &public_values[..digest.len()]);
        assert!(public_values[digest.len()..].iter().all(|byte| *byte == 0));
    }
}
