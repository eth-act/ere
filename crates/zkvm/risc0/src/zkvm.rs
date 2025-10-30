use crate::program::Risc0Program;
use anyhow::bail;
use ere_zkvm_interface::zkvm::{
    CommonError, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMProgramDigest,
};
use risc0_zkvm::{
    DEFAULT_MAX_PO2, DefaultProver, Digest, ExecutorEnv, ExternalProver, InnerReceipt, ProverOpts,
    Receipt, default_executor, default_prover,
};
use std::{env, ops::RangeInclusive, rc::Rc, time::Instant};

mod error;

pub use error::Error;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

/// Default logarithmic segment size from [`DEFAULT_SEGMENT_LIMIT_PO2`].
///
/// [`DEFAULT_SEGMENT_LIMIT_PO2`]: https://github.com/risc0/risc0/blob/v3.0.3/risc0/circuit/rv32im/src/execute/mod.rs#L39.
const DEFAULT_SEGMENT_PO2: usize = 20;

/// Supported range of logarithmic segment size.
///
/// The minimum is by [`MIN_LIFT_PO2`] to be lifted.
///
/// The maximum is by [`DEFAULT_MAX_PO2`], although the real maximum is `24`,
/// but it requires us to set the `control_ids` manually in the `ProverOpts`.
///
/// [`MIN_LIFT_PO2`]: https://github.com/risc0/risc0/blob/v3.0.3/risc0/circuit/recursion/src/control_id.rs#L19
/// [`DEFAULT_MAX_PO2`]: https://github.com/risc0/risc0/blob/v3.0.3/risc0/zkvm/src/receipt.rs#L884
const SEGMENT_PO2_RANGE: RangeInclusive<usize> = 14..=DEFAULT_MAX_PO2;

/// Default logarithmic keccak size from [`KECCAK_DEFAULT_PO2`].
///
/// [`KECCAK_DEFAULT_PO2`]: https://github.com/risc0/risc0/blob/v3.0.3/risc0/circuit/keccak/src/lib.rs#L27.
const DEFAULT_KECCAK_PO2: usize = 17;

/// Supported range of logarithmic keccak size from [`KECCAK_PO2_RANGE`].
///
/// [`KECCAK_PO2_RANGE`]: https://github.com/risc0/risc0/blob/v3.0.3/risc0/circuit/keccak/src/lib.rs#L29.
const KECCAK_PO2_RANGE: RangeInclusive<usize> = 14..=18;

pub struct EreRisc0 {
    program: Risc0Program,
    resource: ProverResourceType,
    segment_po2: usize,
    keccak_po2: usize,
}

impl EreRisc0 {
    pub fn new(program: Risc0Program, resource: ProverResourceType) -> Result<Self, Error> {
        if matches!(resource, ProverResourceType::Network(_)) {
            panic!(
                "Network proving not yet implemented for RISC Zero. Use CPU or GPU resource type."
            );
        }

        let [segment_po2, keccak_po2] = [
            ("RISC0_SEGMENT_PO2", DEFAULT_SEGMENT_PO2, SEGMENT_PO2_RANGE),
            ("RISC0_KECCAK_PO2", DEFAULT_KECCAK_PO2, KECCAK_PO2_RANGE),
        ]
        .map(|(key, default, range)| {
            let val = env::var(key)
                .ok()
                .and_then(|po2| po2.parse::<usize>().ok())
                .unwrap_or(default);
            if !range.contains(&val) {
                panic!("Unsupported po2 value {val} of {key}, expected in range {range:?}")
            }
            val
        });

        Ok(Self {
            program,
            resource,
            segment_po2,
            keccak_po2,
        })
    }
}

impl zkVM for EreRisc0 {
    fn execute(&self, input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let executor = default_executor();
        let env = ExecutorEnv::builder()
            .write_slice(input)
            .build()
            .map_err(Error::BuildExecutorEnv)?;

        let start = Instant::now();
        let session_info = executor
            .execute(env, &self.program.elf)
            .map_err(Error::Execute)?;

        let public_values = session_info.journal.bytes.clone();

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles: session_info.cycles() as u64,
                execution_duration: start.elapsed(),
                ..Default::default()
            },
        ))
    }

    fn prove(
        &self,
        input: &[u8],
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        let prover = match self.resource {
            ProverResourceType::Cpu => Rc::new(ExternalProver::new("ipc", "r0vm")),
            ProverResourceType::Gpu => {
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
            ProverResourceType::Network(_) => {
                panic!(
                    "Network proving not yet implemented for RISC Zero. Use CPU or GPU resource type."
                );
            }
        };

        let env = ExecutorEnv::builder()
            .write_slice(input)
            .segment_limit_po2(self.segment_po2 as _)
            .keccak_max_po2(self.keccak_po2 as _)
            .and_then(|builder| builder.build())
            .map_err(Error::BuildExecutorEnv)?;

        let opts = match proof_kind {
            ProofKind::Compressed => ProverOpts::succinct(),
            ProofKind::Groth16 => ProverOpts::groth16(),
        };

        let now = Instant::now();
        let prove_info = prover
            .prove_with_opts(env, &self.program.elf, &opts)
            .map_err(Error::Prove)?;
        let proving_time = now.elapsed();

        let public_values = prove_info.receipt.journal.bytes.clone();
        let proof = Proof::new(
            proof_kind,
            borsh::to_vec(&prove_info.receipt)
                .map_err(|err| CommonError::serialize("proof", "borsh", err))?,
        );

        Ok((
            public_values,
            proof,
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        let proof_kind = proof.kind();

        let receipt: Receipt = borsh::from_slice(proof.as_bytes())
            .map_err(|err| CommonError::deserialize("proof", "borsh", err))?;

        if !matches!(
            (proof_kind, &receipt.inner),
            (ProofKind::Compressed, InnerReceipt::Succinct(_))
                | (ProofKind::Groth16, InnerReceipt::Groth16(_))
        ) {
            let got = match &receipt.inner {
                InnerReceipt::Composite(_) => "Composite",
                InnerReceipt::Succinct(_) => "Succinct",
                InnerReceipt::Groth16(_) => "Groth16",
                InnerReceipt::Fake(_) => "Fake",
                _ => "Unknown",
            };
            bail!(Error::InvalidProofKind(proof_kind, got.to_string()));
        }

        receipt
            .verify(self.program.image_id)
            .map_err(Error::Verify)?;

        let public_values = receipt.journal.bytes.clone();

        Ok(public_values)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

impl zkVMProgramDigest for EreRisc0 {
    type ProgramDigest = Digest;

    fn program_digest(&self) -> anyhow::Result<Self::ProgramDigest> {
        Ok(self.program.image_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv32imaCustomized, program::Risc0Program, zkvm::EreRisc0};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgramInput,
    };
    use ere_zkvm_interface::{
        compiler::Compiler,
        zkvm::{ProofKind, ProverResourceType, zkVM},
    };
    use std::sync::OnceLock;

    fn basic_program() -> Risc0Program {
        static PROGRAM: OnceLock<Risc0Program> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                RustRv32imaCustomized
                    .compile(&testing_guest_directory("risc0", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgramInput::valid();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_input() {
        let program = basic_program();
        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgramInput::valid();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_input() {
        let program = basic_program();
        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }

    #[test]
    fn test_aligned_allocs() {
        let program = RustRv32imaCustomized
            .compile(&testing_guest_directory("risc0", "allocs_alignment"))
            .unwrap();

        for i in 1..=16_u32 {
            let zkvm = EreRisc0::new(program.clone(), ProverResourceType::Cpu).unwrap();

            let input = i.to_le_bytes();

            if i.is_power_of_two() {
                zkvm.execute(&input)
                    .expect("Power of two alignment should execute successfully");
            } else {
                zkvm.execute(&input)
                    .expect_err("Non-power of two aligment is expected to fail");
            }
        }
    }
}
