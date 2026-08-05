//! OpenVM execution instance.

use std::{sync::Arc, time::Instant};

use ere_prover_core::{ProgramExecutionReport, PublicValues};
use openvm_circuit::{
    arch::{
        U16_CELL_SIZE, VirtualMachineError, VmExecutor, instructions::exe::VmExe,
        rvr::RvrPureInstance,
    },
    system::memory::merkle::public_values::extract_public_values,
};
use openvm_sdk::{F, StdIn};
use openvm_sdk_config::SdkVmConfig;

use crate::error::Error;

/// A precomputed execution instance with the executor it borrows from.
///
/// `instance` holds borrows (and raw precompute pointers) into `*executor`, making
/// this self-referential. The `'static` lifetime is sound because boxing `executor`
/// fixes its heap address across moves and declaring `instance` first drops the
/// borrow before its referent.
pub(crate) struct Executor {
    instance: RvrPureInstance<'static>,
    // Never read directly. Owned only to keep `*executor` alive for `instance`.
    #[allow(dead_code)]
    executor: Box<VmExecutor<F, SdkVmConfig>>,
    num_public_values_bytes: usize,
}

impl Executor {
    pub(crate) fn new(config: SdkVmConfig, app_exe: &Arc<VmExe<F>>) -> Result<Self, Error> {
        let executor = Box::new(
            VmExecutor::new(config)
                .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?,
        );
        let num_public_values_bytes = executor.config.as_ref().num_public_values * U16_CELL_SIZE;

        let instance = executor
            .instance(app_exe)
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;

        // SAFETY: `instance` borrows only into `*executor`. The executor is boxed,
        // so its heap storage stays at a fixed address when the returned `Executor`
        // is moved, and the `executor` field is dropped after `instance` by field
        // order. The borrow therefore never dangles and never outlives its referent,
        // so extending its lifetime to `'static` for co-storage is sound.
        let instance: RvrPureInstance<'static> = unsafe { std::mem::transmute(instance) };

        Ok(Self {
            instance,
            executor,
            num_public_values_bytes,
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
            .execute(stdin)
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?
            .memory;
        let execution_duration = start.elapsed();
        let public_values =
            extract_public_values(self.num_public_values_bytes, &final_memory.memory);
        Ok((
            public_values.into(),
            ProgramExecutionReport {
                execution_duration,
                ..Default::default()
            },
        ))
    }
}
