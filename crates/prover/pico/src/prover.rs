use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CommonError, CostEstimation, CostProfile, Input, ProverResource, ProverResourceKind,
    PublicValues, zkVMProver,
};
use ere_verifier_pico::{PicoProgramVk, PicoProof, PicoVerifier};
use pico_prover::{CostComponent, Prover, ProverBuilder, Stdin};

use crate::error::Error;

pub struct PicoProver {
    elf: Elf,
    /// Emulates without keygen, so execution does not wait for a running prove.
    emulator: ProverBuilder,
    /// Locked, because proving keeps recursion keys across calls and takes `&mut self`.
    prover: Mutex<Prover>,
    verifier: PicoVerifier,
}

impl PicoProver {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        if !matches!(resource, ProverResource::Cpu | ProverResource::Gpu) {
            Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
            ))?;
        }
        #[cfg(not(feature = "cuda"))]
        if matches!(resource, ProverResource::Gpu) {
            Err(Error::CudaFeatureDisabled)?;
        }
        // `pico-vm/cuda` makes every proving machine a GPU one, so such a build has no CPU prover.
        #[cfg(feature = "cuda")]
        if matches!(resource, ProverResource::Cpu) {
            Err(Error::CpuUnsupportedWithCuda)?;
        }

        let (prover, verifier) = load(&elf)?;
        Ok(Self {
            emulator: builder(&elf),
            elf,
            prover: Mutex::new(prover),
            verifier,
        })
    }

    fn prover(&self) -> Result<MutexGuard<'_, Prover>, Error> {
        self.prover.lock().map_err(|_| Error::ProverPoisoned)
    }
}

impl zkVMProver for PicoProver {
    type Verifier = PicoVerifier;
    type Error = Error;

    fn verifier(&self) -> &PicoVerifier {
        &self.verifier
    }

    fn setup(&mut self, elf: Elf) -> Result<(), Error> {
        // A prover poisoned by a panic is rebuilt even for the same program.
        if self.elf == elf && !self.prover.is_poisoned() {
            return Ok(());
        }

        let (prover, verifier) = load(&elf)?;
        self.emulator = builder(&elf);
        self.elf = elf;
        self.prover = Mutex::new(prover);
        self.verifier = verifier;
        Ok(())
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, Duration), Error> {
        let stdin = stdin(input)?;

        let start = Instant::now();
        let execution = self.emulator.execute(stdin).map_err(Error::Execute)?;
        let execution_duration = start.elapsed();

        Ok((execution.committed_bytes.into(), execution_duration))
    }

    fn execute_estimated_cost(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, CostEstimation), Error> {
        let stdin = stdin(input)?;

        // `estimate_cost` does not return the committed bytes, so this emulates twice.
        let execution = self
            .emulator
            .execute(stdin.clone())
            .map_err(Error::Execute)?;
        let estimate = self
            .emulator
            .estimate_cost(stdin)
            .map_err(Error::EstimateCost)?;

        let cost = [
            ("rv64", CostComponent::Rv64),
            ("precompile", CostComponent::Precompile),
            ("system", CostComponent::System),
        ]
        .into_iter()
        .map(|(name, component)| (name.to_owned(), estimate.cells(component)))
        .collect::<BTreeMap<_, _>>();

        Ok((execution.committed_bytes.into(), CostEstimation { cost }))
    }

    fn profile(&self, _input: &Input) -> Result<(PublicValues, CostProfile), Error> {
        Err(Error::ProfileUnsupported)
    }

    fn prove(&self, input: &Input) -> Result<(PublicValues, PicoProof, Duration), Error> {
        let stdin = stdin(input)?;

        let mut prover = self.prover()?;
        let start = Instant::now();
        let proof = prover.prove(stdin).map_err(Error::Prove)?;
        let proving_time = start.elapsed();

        let (envelope, committed) = proof.export_v3().map_err(Error::Prove)?;
        let public_values = committed.as_slice().into();
        let proof = PicoProof::new(envelope, committed);

        Ok((public_values, proof, proving_time))
    }
}

/// Whitelist mode, under the whitelist compiled into `pico-prover`.
fn builder(elf: &Elf) -> ProverBuilder {
    Prover::builder(&elf.0).shipped_whitelist()
}

fn load(elf: &Elf) -> Result<(Prover, PicoVerifier), Error> {
    let prover = builder(elf).build().map_err(Error::ProverInit)?;
    let bundle = prover.vk_bundle().map_err(Error::ProverInit)?;
    let verifier = PicoVerifier::new(PicoProgramVk::new(bundle)?);
    Ok((prover, verifier))
}

fn stdin(input: &Input) -> Result<Stdin, Error> {
    if input.proofs.is_some() {
        Err(CommonError::unsupported_input("no dedicated proofs stream"))?
    }

    let mut stdin = Stdin::new();
    stdin.write_bytes(input.stdin());
    Ok(stdin)
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_pico::{PicoRustRv64ima, PicoRustRv64imaCustomized};
    use ere_prover_core::{Input, ProverResource, codec::Encode, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{
            TestCase, run_zkvm_execute, run_zkvm_execute_estimated_cost, run_zkvm_prove,
            testing_guest_directory,
        },
        program::basic::BasicProgram,
    };

    use crate::prover::PicoProver;

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            PicoRustRv64imaCustomized
                .compile(testing_guest_directory("pico", "basic"), &[])
                .unwrap()
        })
        .clone()
    }

    fn stock_nightly_no_std_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            PicoRustRv64ima
                .compile(testing_guest_directory("pico", "stock_nightly_no_std"), &[])
                .unwrap()
        })
        .clone()
    }

    /// One prover per resource, shared by the tests, since each prove takes tens of GiB.
    fn zkvm(gpu: bool) -> MutexGuard<'static, PicoProver> {
        static ZKVM: [OnceLock<Mutex<PicoProver>>; 2] = [OnceLock::new(), OnceLock::new()];
        ZKVM[gpu as usize]
            .get_or_init(|| {
                let resource = if gpu {
                    ProverResource::Gpu
                } else {
                    ProverResource::Cpu
                };
                Mutex::new(PicoProver::new(basic_elf(), resource).unwrap())
            })
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn with_basic_elf(mut zkvm: MutexGuard<PicoProver>) -> MutexGuard<PicoProver> {
        zkvm.setup(basic_elf()).unwrap();
        zkvm
    }

    /// Switches from the basic program to `stock_nightly_no_std` and back, then runs both again.
    ///
    /// The other zkVMs switch to `zkvm_interface`, which Pico does not support yet.
    fn run_switchable(zkvm: &mut PicoProver, prove: bool) {
        zkvm.setup(basic_elf()).unwrap();
        let basic_vk = zkvm.program_vk().encode_to_vec().unwrap();
        zkvm.setup(stock_nightly_no_std_elf()).unwrap();
        let stock_nightly_no_std_vk = zkvm.program_vk().encode_to_vec().unwrap();

        zkvm.setup(basic_elf()).unwrap();
        assert_eq!(zkvm.program_vk().encode_to_vec().unwrap(), basic_vk);
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        if prove {
            run_zkvm_prove(&*zkvm, &test_case);
        } else {
            run_zkvm_execute(&*zkvm, &test_case);
        }

        zkvm.setup(stock_nightly_no_std_elf()).unwrap();
        assert_eq!(
            zkvm.program_vk().encode_to_vec().unwrap(),
            stock_nightly_no_std_vk
        );
        if prove {
            let (_, proof, _) = zkvm.prove(&Input::new()).unwrap();
            zkvm.verify(&proof).unwrap();
        } else {
            zkvm.execute(&Input::new()).unwrap();
        }
    }

    /// The prover a build has: CPU without `cuda`, GPU with it.
    fn default_zkvm() -> MutexGuard<'static, PicoProver> {
        zkvm(cfg!(feature = "cuda"))
    }

    #[test]
    fn test_execute() {
        let zkvm = with_basic_elf(default_zkvm());
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&*zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let zkvm = with_basic_elf(default_zkvm());
        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_execute_estimated_cost() {
        let zkvm = with_basic_elf(default_zkvm());
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute_estimated_cost(&*zkvm, &test_case);
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_prove() {
        let zkvm = with_basic_elf(zkvm(false));
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&*zkvm, &test_case);
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_prove_invalid_test_case() {
        let zkvm = with_basic_elf(zkvm(false));
        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&*zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_gpu() {
        let zkvm = with_basic_elf(zkvm(true));
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&*zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let zkvm = with_basic_elf(zkvm(true));
        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&*zkvm, &test_case);
    }

    #[test]
    fn test_execute_switchable() {
        run_switchable(&mut default_zkvm(), false);
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_prove_switchable() {
        run_switchable(&mut zkvm(false), true);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_switchable_gpu() {
        run_switchable(&mut zkvm(true), true);
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_gpu_without_cuda_feature() {
        let err = PicoProver::new(basic_elf(), ProverResource::Gpu)
            .err()
            .unwrap();
        assert!(matches!(err, crate::Error::CudaFeatureDisabled));
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_cpu_with_cuda_feature() {
        let err = PicoProver::new(basic_elf(), ProverResource::Cpu)
            .err()
            .unwrap();
        assert!(matches!(err, crate::Error::CpuUnsupportedWithCuda));
    }

    // TODO: Add `test_execute_zkvm_interface` when Pico exports the `zkvm_*`
    // accelerator symbols.
}
