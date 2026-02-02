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
use std::env;

#[derive(CanonicalSerialize, CanonicalDeserialize)]
pub struct JoltProof {
    proof: RV64IMACProof,
    outputs: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct JoltConfig {
    max_input_size: u64,
    max_trusted_advice_size: u64,
    max_untrusted_advice_size: u64,
    max_output_size: u64,
    stack_size: u64,
    memory_size: u64,
    max_trace_length: u64,
}

impl JoltConfig {
    pub fn from_env() -> Self {
        #[rustfmt::skip]
        let envs = [
            ("JOLT_MAX_INPUT_SIZE",            DEFAULT_MAX_INPUT_SIZE),
            ("JOLT_MAX_TRUSTED_ADVICE_SIZE",   DEFAULT_MAX_TRUSTED_ADVICE_SIZE),
            ("JOLT_MAX_UNTRUSTED_ADVICE_SIZE", DEFAULT_MAX_UNTRUSTED_ADVICE_SIZE),
            ("JOLT_MAX_OUTPUT_SIZE",           DEFAULT_MAX_OUTPUT_SIZE),
            ("JOLT_STACK_SIZE",                DEFAULT_STACK_SIZE),
            ("JOLT_MEMORY_SIZE",               DEFAULT_MEMORY_SIZE),
            ("JOLT_MAX_TRACE_LENGTH",          DEFAULT_MAX_TRACE_LENGTH),
        ];
        let [
            max_input_size,
            max_trusted_advice_size,
            max_untrusted_advice_size,
            max_output_size,
            stack_size,
            memory_size,
            max_trace_length,
        ] = envs.map(|(key, default)| {
            env::var(key)
                .ok()
                .and_then(|val| val.parse().ok())
                .unwrap_or(default)
        });
        Self {
            max_input_size,
            max_trusted_advice_size,
            max_untrusted_advice_size,
            max_output_size,
            stack_size,
            memory_size,
            max_trace_length,
        }
    }
}

pub struct JoltSdk {
    elf: Vec<u8>,
    memory_config: MemoryConfig,
    pk: JoltProverPreprocessing<F, PCS>,
    vk: JoltVerifierPreprocessing<F, PCS>,
}

impl JoltSdk {
    pub fn new(elf: &[u8], config: JoltConfig) -> Self {
        let (bytecode, memory_init, program_size) = decode(elf);
        let memory_config = MemoryConfig {
            max_input_size: config.max_input_size,
            max_trusted_advice_size: config.max_trusted_advice_size,
            max_untrusted_advice_size: config.max_untrusted_advice_size,
            max_output_size: config.max_output_size,
            stack_size: config.stack_size,
            memory_size: config.memory_size,
            program_size: Some(program_size),
        };
        let memory_layout = MemoryLayout::new(&memory_config);

        // FIXME: Use public trusted setup or switch to other transparent PCS.
        let shared = JoltSharedPreprocessing::new(
            bytecode,
            memory_layout,
            memory_init,
            config.max_trace_length as usize,
        );
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
        // Use untrusted advice (aka private input) instead of input of Jolt device,
        // which is public to verifier.
        let untrusted_advice = input;
        let (trace_iter, materialized_trace, _memory, io) = trace(
            &self.elf,
            None,
            &[],
            untrusted_advice,
            &[],
            &self.memory_config,
        );
        if io.panic {
            return Err(Error::ExecutionPanic);
        }
        let num_cycles = materialized_trace.len() + trace_iter.count();
        let public_values = extract_public_values(&io.outputs)?;
        Ok((public_values, num_cycles as _))
    }

    pub fn prove(&self, input: &[u8]) -> Result<(PublicValues, JoltProof), Error> {
        // Use untrusted advice (aka private input) instead of input of Jolt device,
        // which is public to verifier.
        let untrusted_advice = input;
        let prover = RV64IMACProver::gen_from_elf(
            &self.pk,
            &self.elf,
            &[],
            untrusted_advice,
            &[],
            None,
            None,
        );
        let io = prover.program_io.clone();
        if io.panic {
            return Err(Error::ExecutionPanic);
        }
        let (proof, _debug_info) = prover.prove();

        let public_values = extract_public_values(&io.outputs)?;
        let proof = JoltProof {
            proof,
            outputs: io.outputs,
        };
        Ok((public_values, proof))
    }

    pub fn verify(&self, proof: JoltProof) -> Result<PublicValues, Error> {
        let io_device = JoltDevice {
            outputs: proof.outputs.clone(),
            panic: false,
            memory_layout: MemoryLayout::new(&self.memory_config),
            ..Default::default()
        };

        let verifier = RV64IMACVerifier::new(&self.vk, proof.proof, io_device, None, None)
            .map_err(Error::VerifierInitFailed)?;
        verifier.verify().map_err(Error::VerifyFailed)?;

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
