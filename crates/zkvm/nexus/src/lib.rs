#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    compiler::NexusProgram,
    error::{NexusError, ProveError, VerifyError},
};
use ere_zkvm_interface::{
    Input, InputItem, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMError,
};
use nexus_core::nvm::{self, ElfFile};
use nexus_sdk::{
    KnownExitCodes, Prover, Verifiable, Viewable,
    stwo::seq::{Proof as NexusProof, Stwo},
};
use nexus_vm::trace::Trace;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{io::Read, time::Instant};
use tracing::info;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub mod compiler;
pub mod error;

#[derive(Serialize, Deserialize)]
pub struct NexusProofBundle {
    proof: NexusProof,
    public_values: Vec<u8>,
}

pub struct EreNexus {
    elf: NexusProgram,
}

impl EreNexus {
    pub fn new(elf: NexusProgram, _resource_type: ProverResourceType) -> Self {
        Self { elf }
    }
}

impl zkVM for EreNexus {
    fn execute(&self, inputs: &Input) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let elf = ElfFile::from_bytes(&self.elf)
            .map_err(|e| NexusError::Prove(ProveError::Client(e.into())))?;

        let input_bytes = serialize_inputs(inputs)?;

        // Nexus sdk does not provide a trace, so we need to use core `nvm`
        // Encoding is copied directly from `prove_with_input`
        let mut private_encoded = if input_bytes.is_empty() {
            Vec::new()
        } else {
            postcard::to_stdvec_cobs(&input_bytes)
                .map_err(|e| NexusError::Prove(ProveError::Postcard(e.to_string())))?
        };

        if !private_encoded.is_empty() {
            let private_padded_len = (private_encoded.len() + 3) & !3;
            assert!(private_padded_len >= private_encoded.len());
            private_encoded.resize(private_padded_len, 0x00);
        }

        let start = Instant::now();
        let (view, trace) = nvm::k_trace(elf, &[], &[], private_encoded.as_slice(), 1)
            .map_err(|e| NexusError::Prove(ProveError::Client(e.into())))?;

        let public_values = view
            .public_output::<Vec<u8>>()
            .map_err(|e| NexusError::Prove(ProveError::Client(e.into())))?;

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles: trace.get_num_steps() as u64,
                region_cycles: Default::default(), // not available
                execution_duration: start.elapsed(),
            },
        ))
    }

    fn prove(
        &self,
        inputs: &Input,
        proof_kind: ProofKind,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        if proof_kind != ProofKind::Compressed {
            panic!("Only Compressed proof kind is supported.");
        }

        let elf = ElfFile::from_bytes(&self.elf)
            .map_err(|e| NexusError::Prove(ProveError::Client(e.into())))?;

        let prover =
            Stwo::new(&elf).map_err(|e| NexusError::Prove(ProveError::Client(e.into())))?;

        let input_bytes = serialize_inputs(inputs)?;

        let start = Instant::now();
        let (view, proof) = prover
            .prove_with_input::<Vec<u8>, ()>(&input_bytes, &())
            .map_err(|e| NexusError::Prove(ProveError::Client(e.into())))?;

        let public_values = view
            .public_output::<Vec<u8>>()
            .map_err(|e| NexusError::Prove(ProveError::Client(e.into())))?;

        let proof_bundle = NexusProofBundle {
            proof,
            public_values: public_values.clone(),
        };

        let proof_bytes = bincode::serialize(&proof_bundle)
            .map_err(|err| NexusError::Prove(ProveError::Bincode(err)))?;

        Ok((
            public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(start.elapsed()),
        ))
    }

    fn verify(&self, proof: &Proof) -> Result<PublicValues, zkVMError> {
        let Proof::Compressed(proof) = proof else {
            return Err(zkVMError::other("Only Compressed proof kind is supported."));
        };

        info!("Verifying proof...");

        let proof_bundle = bincode::deserialize::<NexusProofBundle>(proof)
            .map_err(|err| NexusError::Verify(VerifyError::Bincode(err)))?;

        proof_bundle
            .proof
            .verify_expected_from_program_bytes::<(), Vec<u8>>(
                &(),
                KnownExitCodes::ExitSuccess as u32,
                &proof_bundle.public_values,
                &self.elf,
                &[],
            )
            .map_err(|e| NexusError::Verify(VerifyError::Client(e.into())))?;

        info!("Verify Succeeded!");

        Ok(proof_bundle.public_values)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }

    fn deserialize_from<R: Read, T: DeserializeOwned>(&self, reader: R) -> Result<T, zkVMError> {
        let mut buf = vec![0; 1 << 20]; // allocate 1MiB as buffer.
        let (value, _) = postcard::from_io((reader, &mut buf)).map_err(zkVMError::other)?;
        Ok(value)
    }
}

/// Serializes nexus program inputs
pub fn serialize_inputs(inputs: &Input) -> Result<Vec<u8>, NexusError> {
    inputs
        .iter()
        .try_fold(Vec::new(), |mut acc, item| -> Result<Vec<u8>, NexusError> {
            match item {
                InputItem::Object(obj) => {
                    let buffer = postcard::to_allocvec(obj.as_ref())
                        .map_err(|e| NexusError::Prove(ProveError::Postcard(e.to_string())))?;
                    acc.extend_from_slice(&buffer);
                    Ok(acc)
                }
                InputItem::SerializedObject(bytes) => {
                    acc.extend_from_slice(bytes);
                    Ok(acc)
                }
                InputItem::Bytes(bytes) => {
                    let buffer = postcard::to_allocvec(bytes)
                        .map_err(|e| NexusError::Prove(ProveError::Postcard(e.to_string())))?;
                    acc.extend_from_slice(&buffer);
                    Ok(acc)
                }
            }
        })
}

#[cfg(test)]
mod tests {
    use crate::{EreNexus, compiler::RustRv32i};
    use ere_test_utils::host::{
        BasicProgramIo, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };
    use ere_zkvm_interface::{Compiler, Input, ProofKind, ProverResourceType, zkVM};
    use serde::{Deserialize, Serialize};
    use std::sync::OnceLock;

    static BASIC_PROGRAM: OnceLock<Vec<u8>> = OnceLock::new();
    static FIB_PROGRAM: OnceLock<Vec<u8>> = OnceLock::new();

    fn basic_program() -> Vec<u8> {
        BASIC_PROGRAM
            .get_or_init(|| {
                RustRv32i
                    .compile(&testing_guest_directory("nexus", "basic"))
                    .unwrap()
            })
            .clone()
    }

    fn fib_program() -> Vec<u8> {
        FIB_PROGRAM
            .get_or_init(|| {
                RustRv32i
                    .compile(&testing_guest_directory("nexus", "fib"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu);

        let io = BasicProgramIo::valid();
        run_zkvm_execute(&zkvm, &io);
    }

    #[test]
    fn test_execute_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu);

        for inputs in [
            BasicProgramIo::empty(),
            BasicProgramIo::invalid_type(),
            BasicProgramIo::invalid_data(),
        ] {
            zkvm.execute(&inputs).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu);

        let io = BasicProgramIo::valid();
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn test_prove_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu);

        for inputs in [
            BasicProgramIo::empty(),
            BasicProgramIo::invalid_type(),
            BasicProgramIo::invalid_data(),
        ] {
            zkvm.prove(&inputs, ProofKind::default()).unwrap_err();
        }
    }

    #[test]
    fn test_fibonacci() {
        #[derive(Serialize, Deserialize)]
        struct FibInput {
            n: u32,
        }

        let program = fib_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu);

        let mut input = Input::new();
        input.write(FibInput { n: 10 });

        let (public_values, _report) = zkvm.execute(&input).expect("Execution failed");

        let result: u32 = zkvm
            .deserialize_from(&public_values[..])
            .expect("Failed to deserialize output");
        assert_eq!(result, 55, "fib(10) should be 55");

        let mut input = Input::new();
        input.write(FibInput { n: 0 });

        let (public_values, _report) = zkvm.execute(&input).expect("Execution failed");
        let result: u32 = zkvm
            .deserialize_from(&public_values[..])
            .expect("Failed to deserialize output");
        assert_eq!(result, 0, "fib(0) should be 0");

        let mut input = Input::new();
        input.write(FibInput { n: 1 });

        let (public_values, _report) = zkvm.execute(&input).expect("Execution failed");
        let result: u32 = zkvm
            .deserialize_from(&public_values[..])
            .expect("Failed to deserialize output");
        assert_eq!(result, 1, "fib(1) should be 1");
    }
}
