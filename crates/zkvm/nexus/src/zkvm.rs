use crate::program::NexusProgram;
use anyhow::bail;
use ere_zkvm_interface::zkvm::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM,
};
use nexus_core::nvm::{self, ElfFile, internals::LinearMemoryLayout};
use nexus_sdk::{CheckedView, KnownExitCodes, Viewable};
use nexus_vm::{emulator::InternalView, trace::Trace};
use nexus_vm_prover::{
    Proof as RawProof,
    machine::{BaseComponent, Machine},
};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::info;

pub use nexus_vm_prover::extensions::ExtensionComponent as NexusExtension;

mod error;

pub use error::Error;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

#[derive(Serialize, Deserialize)]
struct NexusProof {
    proof: RawProof,
    memory_layout: LinearMemoryLayout,
}

#[derive(Serialize, Deserialize)]
struct NexusProofBundle {
    proof: NexusProof,
    raw_output: Vec<u8>,
    exit_code: u32,
}

pub struct EreNexus {
    program: NexusProgram,
    extensions: Vec<NexusExtension>,
}

impl EreNexus {
    pub fn new(program: NexusProgram, resource: ProverResourceType) -> Result<Self, Error> {
        Self::with_extensions(program, resource, vec![])
    }

    pub fn with_extensions(
        program: NexusProgram,
        resource: ProverResourceType,
        extensions: Vec<NexusExtension>,
    ) -> Result<Self, Error> {
        if !matches!(resource, ProverResourceType::Cpu) {
            panic!("Network or GPU proving not yet implemented for Nexus. Use CPU resource type.");
        }

        Ok(Self {
            program,
            extensions,
        })
    }
}

impl zkVM for EreNexus {
    fn execute(&self, input: &Input) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        if input.proofs.is_some() {
            bail!(CommonError::unsupported_input("no dedicated proofs stream"))
        }

        let elf = ElfFile::from_bytes(self.program.elf()).map_err(Error::ParseElf)?;

        let private_encoded = encode_private_input(input.stdin())?;

        let start = Instant::now();
        let (view, trace) =
            nvm::k_trace(elf, &[], &[], private_encoded.as_slice(), 1).map_err(Error::Execute)?;
        let execution_duration = start.elapsed();

        let exit_code = view
            .exit_code()
            .unwrap_or(KnownExitCodes::ExitSuccess as u32);
        if exit_code != KnownExitCodes::ExitSuccess as u32 {
            bail!(Error::GuestPanic(exit_code));
        }

        let public_values = decode_public_output(view.view_public_output());

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles: trace.get_num_steps() as u64,
                execution_duration,
                ..Default::default()
            },
        ))
    }

    fn prove(
        &self,
        input: &Input,
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        if input.proofs.is_some() {
            bail!(CommonError::unsupported_input("no dedicated proofs stream"))
        }
        if proof_kind != ProofKind::Compressed {
            bail!(CommonError::unsupported_proof_kind(
                proof_kind,
                [ProofKind::Compressed]
            ))
        }

        let elf = ElfFile::from_bytes(self.program.elf()).map_err(Error::ParseElf)?;

        let private_encoded = encode_private_input(input.stdin())?;

        let start = Instant::now();
        let (view, trace) =
            nvm::k_trace(elf, &[], &[], private_encoded.as_slice(), 1).map_err(Error::Execute)?;

        let exit_code = view
            .exit_code()
            .unwrap_or(KnownExitCodes::ExitSuccess as u32);
        if exit_code != KnownExitCodes::ExitSuccess as u32 {
            bail!(Error::GuestPanic(exit_code));
        }

        let proof =
            Machine::<BaseComponent>::prove_with_extensions(&self.extensions, &trace, &view)
                .map_err(Error::Prove)?;
        let proving_time = start.elapsed();

        let raw_output = view.view_public_output().unwrap_or_default();
        let public_values = decode_public_output(view.view_public_output());

        let proof_bundle = NexusProofBundle {
            proof: NexusProof {
                proof,
                memory_layout: trace.memory_layout,
            },
            raw_output,
            exit_code,
        };

        let proof_bytes = bincode::serde::encode_to_vec(&proof_bundle, bincode::config::legacy())
            .map_err(|err| CommonError::serialize("proof", "bincode", err))?;

        Ok((
            public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        let Proof::Compressed(proof) = proof else {
            bail!(CommonError::unsupported_proof_kind(
                proof.kind(),
                [ProofKind::Compressed]
            ))
        };

        info!("Verifying proof...");

        let (proof_bundle, _): (NexusProofBundle, _) =
            bincode::serde::decode_from_slice(proof, bincode::config::legacy())
                .map_err(|err| CommonError::deserialize("proof", "bincode", err))?;

        let elf = ElfFile::from_bytes(self.program.elf()).map_err(Error::ParseElf)?;
        let layout = proof_bundle.proof.memory_layout;

        let view = nvm::View::new_from_expected(
            &layout,
            &[],
            &proof_bundle.exit_code.to_le_bytes(),
            &proof_bundle.raw_output,
            &elf,
            &[],
        );

        let init_memory: Vec<_> = [
            view.get_ro_initial_memory(),
            view.get_rw_initial_memory(),
            view.get_public_input(),
        ]
        .concat();

        Machine::<BaseComponent>::verify_with_extensions(
            &self.extensions,
            proof_bundle.proof.proof,
            view.get_program_memory(),
            view.view_associated_data().as_deref().unwrap_or_default(),
            &init_memory,
            view.get_exit_code(),
            view.get_public_output(),
        )
        .map_err(Error::Verify)?;

        info!("Verify Succeeded!");

        let public_values = decode_public_output(view.view_public_output());

        Ok(public_values)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

fn encode_private_input(stdin: &[u8]) -> Result<Vec<u8>, CommonError> {
    if stdin.is_empty() {
        return Ok(Vec::new());
    }

    let mut encoded = postcard::to_stdvec_cobs(&stdin)
        .map_err(|err| CommonError::serialize("input", "postcard", err))?;

    let padded_len = (encoded.len() + 3) & !3;
    encoded.resize(padded_len, 0x00);

    Ok(encoded)
}

fn decode_public_output(public_outputs: Option<Vec<u8>>) -> PublicValues {
    public_outputs
        .and_then(|mut raw| postcard::from_bytes_cobs::<Vec<u8>>(&mut raw).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv32i, program::NexusProgram, zkvm::EreNexus};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        io::serde::bincode::BincodeLegacy,
        program::basic::BasicProgram,
    };
    use ere_zkvm_interface::{
        Input,
        compiler::Compiler,
        zkvm::{ProofKind, ProverResourceType, zkVM},
    };
    use std::sync::OnceLock;

    fn basic_program() -> NexusProgram {
        static PROGRAM: OnceLock<NexusProgram> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                RustRv32i
                    .compile(&testing_guest_directory("nexus", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu).unwrap();

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
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }
}
