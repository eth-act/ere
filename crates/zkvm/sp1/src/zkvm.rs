use crate::{program::SP1Program, zkvm::sdk::SP1Sdk};
use anyhow::bail;
use ere_zkvm_interface::zkvm::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResource, PublicValues, zkVM, zkVMProgramDigest,
};
use sp1_sdk::{SP1ProofMode, SP1ProofWithPublicValues, SP1Stdin, SP1VerifyingKey};
use std::{future::Future, sync::OnceLock, time::Instant};
use tracing::info;

mod error;
mod sdk;

pub use error::Error;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub struct EreSP1 {
    sdk: SP1Sdk,
}

impl EreSP1 {
    pub fn new(program: SP1Program, resource: ProverResource) -> Result<Self, Error> {
        let sdk = block_on(SP1Sdk::new(program.elf, &resource))?;
        Ok(Self { sdk })
    }
}

impl zkVM for EreSP1 {
    fn execute(&self, input: &Input) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let stdin = input_to_stdin(input)?;

        let start = Instant::now();
        let (public_values, exec_report) = block_on(self.sdk.execute(stdin))?;
        let execution_duration = start.elapsed();

        Ok((
            public_values.to_vec(),
            ProgramExecutionReport {
                total_num_cycles: exec_report.total_instruction_count(),
                region_cycles: exec_report.cycle_tracker.into_iter().collect(),
                execution_duration,
            },
        ))
    }

    fn prove(
        &self,
        input: &Input,
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        info!("Generating proof...");

        let stdin = input_to_stdin(input)?;

        let mode = match proof_kind {
            ProofKind::Compressed => SP1ProofMode::Compressed,
            ProofKind::Groth16 => SP1ProofMode::Groth16,
        };

        let start = Instant::now();
        let proof = block_on(self.sdk.prove(stdin, mode))?;
        let proving_time = start.elapsed();

        let public_values = proof.public_values.to_vec();
        let proof = Proof::new(
            proof_kind,
            bincode::serde::encode_to_vec(&proof, bincode::config::legacy())
                .map_err(|err| CommonError::serialize("proof", "bincode", err))?,
        );

        Ok((
            public_values,
            proof,
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        info!("Verifying proof...");

        let proof_kind = proof.kind();

        let (proof, _): (SP1ProofWithPublicValues, _) =
            bincode::serde::decode_from_slice(proof.as_bytes(), bincode::config::legacy())
                .map_err(|err| CommonError::deserialize("proof", "bincode", err))?;
        let inner_proof_kind = SP1ProofMode::from(&proof.proof);

        if !matches!(
            (proof_kind, inner_proof_kind),
            (ProofKind::Compressed, SP1ProofMode::Compressed)
                | (ProofKind::Groth16, SP1ProofMode::Groth16)
        ) {
            bail!(Error::InvalidProofKind(proof_kind, inner_proof_kind));
        }

        self.sdk.verify(&proof)?;

        let public_values_bytes = proof.public_values.as_slice().to_vec();

        Ok(public_values_bytes)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

impl zkVMProgramDigest for EreSP1 {
    type ProgramDigest = SP1VerifyingKey;

    fn program_digest(&self) -> anyhow::Result<Self::ProgramDigest> {
        Ok(self.sdk.verifying_key().clone())
    }
}

fn input_to_stdin(input: &Input) -> Result<SP1Stdin, Error> {
    let mut stdin = SP1Stdin::new();
    stdin.write_slice(input.stdin());
    if let Some(proofs) = input.proofs() {
        for (proof, vk) in proofs.map_err(Error::DeserializeInputProofs)? {
            stdin.write_proof(proof, vk);
        }
    }
    Ok(stdin)
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => {
            static FALLBACK_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
            FALLBACK_RT
                .get_or_init(|| tokio::runtime::Runtime::new().expect("Failed to create runtime"))
                .block_on(future)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv32imaCustomized, program::SP1Program, zkvm::EreSP1};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        io::serde::bincode::BincodeLegacy,
        program::basic::BasicProgram,
    };
    use ere_zkvm_interface::{
        Input,
        compiler::Compiler,
        zkvm::{ProofKind, ProverResource, RemoteProverConfig, zkVM},
    };
    use std::sync::OnceLock;

    fn basic_program() -> SP1Program {
        static PROGRAM: OnceLock<SP1Program> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                RustRv32imaCustomized
                    .compile(&testing_guest_directory("sp1", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResource::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResource::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }

    #[test]
    #[ignore = "Requires NETWORK_PRIVATE_KEY environment variable to be set"]
    fn test_prove_sp1_network() {
        // Check if we have the required environment variable
        if std::env::var("NETWORK_PRIVATE_KEY").is_err() {
            eprintln!("Skipping network test: NETWORK_PRIVATE_KEY not set");
            return;
        }

        // Create a remote prover configuration
        let config = RemoteProverConfig {
            endpoint: std::env::var("NETWORK_RPC_URL").unwrap_or_default(),
            api_key: std::env::var("NETWORK_PRIVATE_KEY").ok(),
        };
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResource::Network(config)).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }
}
