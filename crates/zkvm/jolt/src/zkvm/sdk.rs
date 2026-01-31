use crate::zkvm::Error;
use core::{array::from_fn, cmp::min};
use ere_zkvm_interface::zkvm::PublicValues;
use jolt_ark_serialize::{self as ark_serialize, CanonicalDeserialize, CanonicalSerialize};
use jolt_common::constants::{
    DEFAULT_MAX_INPUT_SIZE, DEFAULT_MAX_OUTPUT_SIZE, DEFAULT_MAX_TRACE_LENGTH,
    DEFAULT_MAX_TRUSTED_ADVICE_SIZE, DEFAULT_MAX_UNTRUSTED_ADVICE_SIZE, DEFAULT_MEMORY_SIZE,
    DEFAULT_STACK_SIZE,
};
use jolt_sdk::{
    F, JoltDevice, JoltProverPreprocessing, JoltSharedPreprocessing, JoltVerifierPreprocessing,
    MemoryConfig, MemoryLayout, PCS, RV64IMACProof, RV64IMACProver, RV64IMACVerifier,
    guest::program::{decode, trace},
};

#[derive(CanonicalSerialize, CanonicalDeserialize)]
pub struct JoltProof {
    proof: RV64IMACProof,
    // FIXME: Remove `inputs` when Jolt supports proving with private input.
    //        Issue for tracking: https://github.com/eth-act/ere/issues/4.
    inputs: Vec<u8>,
    outputs: Vec<u8>,
}

pub struct JoltSdk {
    elf: Vec<u8>,
    memory_config: MemoryConfig,
    pk: JoltProverPreprocessing<F, PCS>,
    vk: JoltVerifierPreprocessing<F, PCS>,
}

impl JoltSdk {
    pub fn new(elf: &[u8]) -> Self {
        let (bytecode, memory_init, program_size) = decode(elf);
        let memory_config = MemoryConfig {
            max_input_size: DEFAULT_MAX_INPUT_SIZE,
            max_trusted_advice_size: DEFAULT_MAX_TRUSTED_ADVICE_SIZE,
            max_untrusted_advice_size: DEFAULT_MAX_UNTRUSTED_ADVICE_SIZE,
            max_output_size: DEFAULT_MAX_OUTPUT_SIZE,
            stack_size: DEFAULT_STACK_SIZE,
            memory_size: DEFAULT_MEMORY_SIZE,
            program_size: Some(program_size),
        };
        let memory_layout = MemoryLayout::new(&memory_config);
        let max_trace_length = DEFAULT_MAX_TRACE_LENGTH as usize;

        // FIXME: Use public trusted setup or switch to other transparent PCS.
        let shared =
            JoltSharedPreprocessing::new(bytecode, memory_layout, memory_init, max_trace_length);
        let pk = JoltProverPreprocessing::new(shared);
        let vk = JoltVerifierPreprocessing::from(&pk);

        Self {
            elf: elf.to_vec(),
            memory_config,
            pk,
            vk,
        }
    }

    pub fn execute(&self, input: &[u8]) -> Result<(PublicValues, u64), Error> {
        let (trace_iter, materialized_trace, _memory, io) =
            trace(&self.elf, None, input, &[], &[], &self.memory_config);
        if io.panic {
            return Err(Error::ExecutionPanic);
        }
        let num_cycles = materialized_trace.len() + trace_iter.count();
        let public_values = extract_public_values(&io.outputs)?;
        Ok((public_values, num_cycles as _))
    }

    pub fn prove(&self, input: &[u8]) -> Result<(PublicValues, JoltProof), Error> {
        let prover: RV64IMACProver =
            RV64IMACProver::gen_from_elf(&self.pk, &self.elf, input, &[], &[], None, None);
        let io = prover.program_io.clone();
        if io.panic {
            return Err(Error::ExecutionPanic);
        }
        let (proof, _debug_info) = prover.prove();

        let public_values = extract_public_values(&io.outputs)?;
        let proof = JoltProof {
            proof,
            inputs: io.inputs,
            outputs: io.outputs,
        };
        Ok((public_values, proof))
    }

    pub fn verify(&self, proof: JoltProof) -> Result<PublicValues, Error> {
        let io_device = JoltDevice {
            inputs: proof.inputs.clone(),
            trusted_advice: Vec::new(),
            untrusted_advice: Vec::new(),
            outputs: proof.outputs.clone(),
            panic: false,
            memory_layout: MemoryLayout::new(&self.memory_config),
        };

        let verifier: RV64IMACVerifier =
            RV64IMACVerifier::new(&self.vk, proof.proof, io_device, None, None)
                .map_err(Error::VerifyProofFailed)?;
        verifier
            .verify()
            .map_err(|e| Error::VerifyFailed(e.to_string()))?;

        let public_values = extract_public_values(&proof.outputs)?;
        Ok(public_values)
    }
}

// Note taht for execute the bytes are padded to size of multiple of 8, but for
// prove the bytes are truncated.
fn extract_public_values(output: &[u8]) -> Result<Vec<u8>, Error> {
    Ok(if output.is_empty() {
        Vec::new()
    } else {
        let len = u32::from_le_bytes(from_fn(|i| output.get(i).copied().unwrap_or(0))) as usize;
        if output.len() > (len + 4).next_multiple_of(8) {
            return Err(Error::InvalidOutput);
        }
        let mut public_values = vec![0; len];
        if let Some((_, output)) = output.split_at_checked(4) {
            let len = min(len, output.len());
            public_values[..len].copy_from_slice(&output[..len]);
        }
        public_values
    })
}
