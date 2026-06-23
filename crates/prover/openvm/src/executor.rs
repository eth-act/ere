//! OpenVM execution instance.

use std::{sync::Arc, time::Instant};

use ere_prover_core::{ProgramExecutionReport, PublicValues};
use openvm_circuit::{
    arch::{
        VirtualMachineError, VmExecutor, execution_mode::ExecutionCtx, instructions::exe::VmExe,
    },
    system::memory::merkle::public_values::extract_public_values,
};
use openvm_sdk::{F, StdIn, config::SdkVmConfig};

use crate::error::Error;

#[cfg(target_arch = "x86_64")]
type ExecutorInstance = openvm_circuit::arch::AotInstance<F, ExecutionCtx>;
#[cfg(not(target_arch = "x86_64"))]
type ExecutorInstance = openvm_circuit::arch::InterpretedInstance<F, ExecutionCtx>;

/// A execution instance and the executor it borrows from.
pub(crate) struct Executor {
    // Borrows from `executor`, so it is dropped first.
    instance: ExecutorInstance,
    // Kept alive to back borrow of `instance`.
    #[allow(dead_code)]
    executor: VmExecutor<F, SdkVmConfig>,
    num_public_values: usize,
}

impl Executor {
    pub(crate) fn new(config: SdkVmConfig, app_exe: &Arc<VmExe<F>>) -> Result<Self, Error> {
        let executor = VmExecutor::new(config)
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;
        let instance = executor
            .instance(app_exe)
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;
        let num_public_values = executor.config.as_ref().num_public_values;
        Ok(Self {
            instance,
            executor,
            num_public_values,
        })
    }

    /// Runs `stdin` on the instance.
    pub(crate) fn execute(
        &self,
        stdin: StdIn,
    ) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        let start = Instant::now();
        let final_memory = self
            .instance
            .execute(stdin, None)
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?
            .memory;
        let execution_duration = start.elapsed();
        let public_values = extract_public_values(self.num_public_values, &final_memory.memory);
        Ok((
            public_values.into(),
            ProgramExecutionReport {
                execution_duration,
                ..Default::default()
            },
        ))
    }
}
