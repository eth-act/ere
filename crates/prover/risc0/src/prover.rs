use core::ops::RangeInclusive;
use std::{env, rc::Rc, time::Instant};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, ProverResource,
    ProverResourceKind, PublicValues, zkVMProver,
};
use ere_verifier_risc0::{Risc0ProgramVk, Risc0Proof, Risc0Verifier};
use risc0_zkvm::{
    AssumptionReceipt, DEFAULT_MAX_PO2, DefaultProver, ExecutorEnv, ExternalProver, ProverOpts,
    default_executor, default_prover,
};

mod error;

pub use error::Error;

/// Default logarithmic segment size from [`DEFAULT_SEGMENT_LIMIT_PO2`].
///
/// [`DEFAULT_SEGMENT_LIMIT_PO2`]: https://github.com/risc0/risc0/blob/v3.0.5/risc0/circuit/rv32im/src/execute/mod.rs#L39.
const DEFAULT_SEGMENT_PO2: usize = 20;

/// Supported range of logarithmic segment size.
///
/// The minimum is by [`MIN_LIFT_PO2`] to be lifted.
///
/// The maximum is by [`DEFAULT_MAX_PO2`], although the real maximum is `24`,
/// but it requires us to set the `control_ids` manually in the `ProverOpts`.
///
/// [`MIN_LIFT_PO2`]: https://github.com/risc0/risc0/blob/v3.0.5/risc0/circuit/recursion/src/control_id.rs#L19
/// [`DEFAULT_MAX_PO2`]: https://github.com/risc0/risc0/blob/v3.0.5/risc0/zkvm/src/receipt.rs#L898
const SEGMENT_PO2_RANGE: RangeInclusive<usize> = 14..=DEFAULT_MAX_PO2;

/// Default logarithmic keccak size from [`KECCAK_DEFAULT_PO2`].
///
/// [`KECCAK_DEFAULT_PO2`]: https://github.com/risc0/risc0/blob/v3.0.5/risc0/circuit/keccak/src/lib.rs#L27.
const DEFAULT_KECCAK_PO2: usize = 17;

/// Supported range of logarithmic keccak size from [`KECCAK_PO2_RANGE`].
///
/// [`KECCAK_PO2_RANGE`]: https://github.com/risc0/risc0/blob/v3.0.5/risc0/circuit/keccak/src/lib.rs#L29.
const KECCAK_PO2_RANGE: RangeInclusive<usize> = 14..=18;

pub struct Risc0Prover {
    elf: Elf,
    verifier: Risc0Verifier,
    resource: ProverResource,
    segment_po2: usize,
    keccak_po2: usize,
}

impl Risc0Prover {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        if !matches!(resource, ProverResource::Cpu | ProverResource::Gpu) {
            Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
            ))?;
        }

        let image_id = risc0_binfmt::compute_image_id(&elf).map_err(Error::ComputeImageId)?;
        let verifier = Risc0Verifier::new(Risc0ProgramVk(image_id));

        let parse_env = |key: &str, default: usize, range: RangeInclusive<usize>| {
            let Ok(val) = env::var(key) else {
                return Ok(default);
            };

            match val.parse() {
                Ok(val) if range.contains(&val) => Ok(val),
                _ => Err(Error::UnsupportedPo2Value {
                    key: key.to_string(),
                    val,
                    range,
                }),
            }
        };

        let segment_po2 = parse_env(
            "ERE_RISC0_SEGMENT_PO2",
            DEFAULT_SEGMENT_PO2,
            SEGMENT_PO2_RANGE,
        )?;
        let keccak_po2 = parse_env("ERE_RISC0_KECCAK_PO2", DEFAULT_KECCAK_PO2, KECCAK_PO2_RANGE)?;

        Ok(Self {
            elf,
            verifier,
            resource,
            segment_po2,
            keccak_po2,
        })
    }
}

impl zkVMProver for Risc0Prover {
    type Verifier = Risc0Verifier;
    type Error = Error;

    fn verifier(&self) -> &Risc0Verifier {
        &self.verifier
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        let env = self.input_to_env(input)?;

        let executor = default_executor();

        let start = Instant::now();
        let session_info = executor.execute(env, &self.elf).map_err(Error::Execute)?;

        Ok((
            session_info.journal.bytes.as_slice().into(),
            ProgramExecutionReport {
                total_num_cycles: session_info.cycles() as u64,
                execution_duration: start.elapsed(),
                ..Default::default()
            },
        ))
    }

    fn prove(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, Risc0Proof, ProgramProvingReport), Error> {
        let env = self.input_to_env(input)?;

        let prover = match self.resource {
            ProverResource::Cpu => Rc::new(ExternalProver::new("ipc", "r0vm")),
            ProverResource::Gpu => {
                if cfg!(feature = "metal") {
                    // When `metal` is enabled, we use the `LocalProver` to do
                    // proving. but it's not public so we use `default_prover`
                    // to instantiate it.
                    default_prover()
                } else {
                    // The `DefaultProver` uses `r0vm-cuda` to spawn multiple
                    // workers to do multi-gpu proving.
                    // It uses env `RISC0_DEFAULT_PROVER_NUM_GPUS` to determine
                    // how many available GPUs there are.
                    Rc::new(DefaultProver::new("r0vm-cuda").map_err(Error::InitializeCudaProver)?)
                }
            }
            _ => {
                return Err(CommonError::unsupported_prover_resource_kind(
                    self.resource.kind(),
                    [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
                ))?;
            }
        };

        let opts = ProverOpts::succinct();

        let now = Instant::now();
        let prove_info = prover
            .prove_with_opts(env, &self.elf, &opts)
            .map_err(Error::Prove)?;
        let proving_time = now.elapsed();

        let public_values = prove_info.receipt.journal.bytes.as_slice().into();
        let proof = Risc0Proof(prove_info.receipt);

        Ok((
            public_values,
            proof,
            ProgramProvingReport::new(proving_time),
        ))
    }
}

impl Risc0Prover {
    fn input_to_env(&self, input: &Input) -> Result<ExecutorEnv<'static>, Error> {
        let mut env = ExecutorEnv::builder();
        env.segment_limit_po2(self.segment_po2 as _)
            .keccak_max_po2(self.keccak_po2 as _)
            .expect("keccak_po2 in valid range");
        env.write_slice(input.stdin());
        if let Some(receipts) = input.proofs() {
            for receipt in receipts.map_err(Error::DeserializeInputProofs)? {
                env.add_assumption(AssumptionReceipt::Proven(receipt));
            }
        }
        env.build().map_err(Error::BuildExecutorEnv)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_risc0::Risc0RustRv32imaCustomized;
    use ere_prover_core::{Input, ProverResource, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgram,
    };

    use crate::prover::Risc0Prover;

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            Risc0RustRv32imaCustomized
                .compile(testing_guest_directory("risc0", "basic"))
                .unwrap()
        })
        .clone()
    }

    #[test]
    fn test_execute() {
        let elf = basic_elf();
        let zkvm = Risc0Prover::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = Risc0Prover::new(elf, ProverResource::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let elf = basic_elf();
        let zkvm = Risc0Prover::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = Risc0Prover::new(elf, ProverResource::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_aligned_allocs() {
        let elf = Risc0RustRv32imaCustomized
            .compile(testing_guest_directory("risc0", "allocs_alignment"))
            .unwrap();

        for i in 1..=16_u32 {
            let zkvm = Risc0Prover::new(elf.clone(), ProverResource::Cpu).unwrap();

            let input = Input::new().with_stdin(i.to_le_bytes().to_vec());

            if i.is_power_of_two() {
                zkvm.execute(&input)
                    .expect("Power of two alignment should execute successfully");
            } else {
                zkvm.execute(&input)
                    .expect_err("Non-power of two aligment is expected to fail");
            }
        }
    }

    #[cfg(any(feature = "cuda", feature = "metal"))]
    #[test]
    fn test_prove_gpu() {
        let elf = basic_elf();
        let zkvm = Risc0Prover::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(any(feature = "cuda", feature = "metal"))]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let elf = basic_elf();
        let zkvm = Risc0Prover::new(elf, ProverResource::Gpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }
}
