use crate::{Risc0Program, serialize_inputs};
use risc0_zkvm::{ExecutorEnv, ProverOpts, Receipt, default_prover};
use std::time::Duration;
use zkvm_interface::{Input, zkVMError};

pub fn prove(program: &Risc0Program, inputs: &Input) -> Result<(Receipt, Duration), zkVMError> {
    let prover = default_prover();
    let mut env = ExecutorEnv::builder();
    serialize_inputs(&mut env, inputs).map_err(|err| zkVMError::Other(err.into()))?;
    let env = env.build().map_err(|err| zkVMError::Other(err.into()))?;

    let now = std::time::Instant::now();
    let prove_info = prover
        .prove_with_opts(env, &program.elf, &ProverOpts::succinct())
        .map_err(|err| zkVMError::Other(err.into()))?;
    let proving_time = now.elapsed();

    Ok((prove_info.receipt, proving_time))
}
