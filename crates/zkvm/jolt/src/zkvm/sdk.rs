use crate::zkvm::Error;
use core::cmp::min;
use ere_zkvm_interface::zkvm::{CommonError, PublicValues};
use jolt_ark_serialize::{self as ark_serialize, CanonicalDeserialize, CanonicalSerialize};
use jolt_common::constants::{
    DEFAULT_MAX_INPUT_SIZE, DEFAULT_MAX_OUTPUT_SIZE, DEFAULT_MAX_TRACE_LENGTH, DEFAULT_MEMORY_SIZE,
    DEFAULT_STACK_SIZE,
};
use jolt_core::{
    poly::commitment::commitment_scheme::CommitmentScheme, transcripts::Blake2bTranscript as FS,
    utils::math::Math, zkvm::witness::DTH_ROOT_OF_K,
};
use jolt_sdk::{
    F, Jolt, JoltDevice, JoltProverPreprocessing, JoltRV64IMAC, JoltVerifierPreprocessing,
    MemoryConfig, MemoryLayout, PCS,
    guest::program::{decode, trace},
    postcard,
};

#[derive(CanonicalSerialize, CanonicalDeserialize)]
pub struct JoltProof {
    proof: jolt_sdk::JoltProof<F, PCS, FS>,
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
            max_output_size: DEFAULT_MAX_OUTPUT_SIZE,
            stack_size: DEFAULT_STACK_SIZE,
            memory_size: DEFAULT_MEMORY_SIZE,
            program_size: Some(program_size),
        };
        let memory_layout = MemoryLayout::new(&memory_config);
        let max_trace_length = DEFAULT_MAX_TRACE_LENGTH as usize;
        let pk = {
            // FIXME: Use public trusted setup or switch to other transparent PCS.
            let max_trace_length = max_trace_length.next_power_of_two();
            let generators = PCS::setup_prover(DTH_ROOT_OF_K.log_2() + max_trace_length.log_2());

            let shared = JoltRV64IMAC::shared_preprocess(bytecode, memory_layout, memory_init);

            JoltProverPreprocessing { generators, shared }
        };
        let vk = JoltVerifierPreprocessing::from(&pk);
        Self {
            elf: elf.to_vec(),
            memory_config,
            pk,
            vk,
        }
    }

    pub fn execute(&self, input: &[u8]) -> Result<(PublicValues, u64), Error> {
        let (cycles, _, io) = trace(
            &self.elf,
            None,
            &serialize_input(input)?,
            &self.memory_config,
        );
        if io.panic {
            return Err(Error::ExecutionPanic);
        }
        let public_values = deserialize_output(&io.outputs)?;
        Ok((public_values, cycles.len() as _))
    }

    pub fn prove(&self, input: &[u8]) -> Result<(PublicValues, JoltProof), Error> {
        let (proof, io, _) = JoltRV64IMAC::prove(&self.pk, &self.elf, &serialize_input(input)?);
        if io.panic {
            return Err(Error::ExecutionPanic);
        }
        let public_values = deserialize_output(&io.outputs)?;
        let proof = JoltProof {
            proof,
            inputs: io.inputs,
            outputs: io.outputs,
        };
        Ok((public_values, proof))
    }

    pub fn verify(&self, proof: JoltProof) -> Result<PublicValues, Error> {
        JoltRV64IMAC::verify(
            &self.vk,
            proof.proof,
            JoltDevice {
                inputs: proof.inputs.clone(),
                outputs: proof.outputs.clone(),
                panic: false,
                memory_layout: MemoryLayout::new(&self.memory_config),
            },
            None,
        )?;
        let public_values = deserialize_output(&proof.outputs)?;
        Ok(public_values)
    }
}

fn serialize_input(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    Ok(postcard::to_stdvec(bytes)
        .map_err(|err| CommonError::serialize("input", "postcard", err))?)
}

fn deserialize_output(output: &[u8]) -> Result<Vec<u8>, Error> {
    Ok(if output.is_empty() {
        Vec::new()
    } else {
        let (len, bytes) = postcard::take_from_bytes::<u64>(output)
            .map_err(|err| CommonError::deserialize("output", "postcard", err))?;
        let mut output = vec![0; len as usize];
        // For execute the bytes are padded to size of multiple of 8, but for
        // prove the bytes are truncated if there are trailing zeros, so here we
        // take the min.
        let len = min(bytes.len(), output.len());
        output[..len].copy_from_slice(&bytes[..len]);
        output
    })
}
