use crate::{EreJoltProof, error::VerifyError};
use common::constants::{DEFAULT_MAX_BYTECODE_SIZE, DEFAULT_MAX_TRACE_LENGTH, DEFAULT_MEMORY_SIZE};
use jolt::{
    Jolt, JoltHyperKZGProof, JoltProverPreprocessing, JoltVerifierPreprocessing, MemoryConfig,
    MemoryLayout, RV32IJoltVM, tracer::JoltDevice,
};
use zkvm_interface::Input;

pub fn preprocess_prover(
    program: &jolt::host::Program,
) -> JoltProverPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript> {
    let (bytecode, memory_init) = program.decode();
    let memory_layout = MemoryLayout::new(&MemoryConfig::default());
    let preprocessing: JoltProverPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript> =
        RV32IJoltVM::prover_preprocess(
            bytecode,
            memory_layout,
            memory_init,
            DEFAULT_MAX_BYTECODE_SIZE as usize,
            DEFAULT_MEMORY_SIZE as usize,
            DEFAULT_MAX_TRACE_LENGTH as usize,
        );
    preprocessing
}

pub fn preprocess_verifier(
    program: &jolt::host::Program,
) -> JoltVerifierPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript> {
    let (bytecode, memory_init) = program.decode();
    let memory_layout = MemoryLayout::new(&MemoryConfig::default());
    let preprocessing: JoltVerifierPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript> =
        RV32IJoltVM::verifier_preprocess(
            bytecode,
            memory_layout,
            memory_init,
            DEFAULT_MAX_BYTECODE_SIZE as usize,
            DEFAULT_MEMORY_SIZE as usize,
            DEFAULT_MAX_TRACE_LENGTH as usize,
        );
    preprocessing
}

pub fn prove_generic(
    program: &jolt::host::Program,
    preprocessing: JoltProverPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript>,
    _inputs: &Input,
) -> EreJoltProof {
    let mut program = program.clone();

    // TODO: Check how to pass private input to jolt, issue for tracking:
    //       https://github.com/a16z/jolt/issues/371.
    let input_bytes = Vec::new();

    let (io_device, trace) = program.trace(&input_bytes);

    let (jolt_proof, jolt_commitments, io_device, _) =
        RV32IJoltVM::prove(io_device, trace, preprocessing);

    EreJoltProof {
        proof: JoltHyperKZGProof {
            proof: jolt_proof,
            commitments: jolt_commitments,
        },
        public_outputs: io_device.outputs,
    }
}

pub fn verify_generic(
    proof: EreJoltProof,
    preprocessing: JoltVerifierPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript>,
) -> Result<(), VerifyError> {
    let mut io_device = JoltDevice::new(&MemoryConfig {
        max_input_size: preprocessing.memory_layout.max_input_size,
        max_output_size: preprocessing.memory_layout.max_output_size,
        stack_size: preprocessing.memory_layout.stack_size,
        memory_size: preprocessing.memory_layout.memory_size,
    });
    io_device.outputs = proof.public_outputs;

    RV32IJoltVM::verify(
        preprocessing,
        proof.proof.proof,
        proof.proof.commitments,
        io_device,
        None,
    )?;

    Ok(())
}
