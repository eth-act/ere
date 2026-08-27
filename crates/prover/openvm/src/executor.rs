//! OpenVM execution instance.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use ere_prover_core::PublicValues;
use ere_verifier_openvm::NUM_PUBLIC_VALUES_BYTES;
use openvm_circuit::{
    arch::{
        VirtualMachineError, VmExecutor, VmState, instructions::exe::VmExe, rvr::RvrPureInstance,
    },
    system::memory::{merkle::public_values, online::GuestMemory},
};
use openvm_sdk::{F, StdIn};
use openvm_sdk_config::SdkVmConfig;

use crate::{error::Error, prover::sdk_vm_config};

/// A precomputed execution instance with the executor it borrows from.
///
/// `instance` borrows the `SystemConfig` owned by `*executor`, making this
/// self-referential. The `'static` lifetime is sound because that config lives
/// behind an `Arc` whose allocation outlives every move of the returned
/// `Executor`, and declaring `instance` first drops the borrow before its
/// referent.
pub(crate) struct Executor {
    instance: RvrPureInstance<'static>,
    // Never read directly. Owned only to keep `*executor` alive for `instance`.
    #[allow(dead_code)]
    executor: Box<VmExecutor<F, SdkVmConfig>>,
}

impl Executor {
    pub(crate) fn new(app_exe: &Arc<VmExe<F>>) -> Result<Self, Error> {
        let executor = Box::new(
            VmExecutor::new(sdk_vm_config())
                .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?,
        );

        let instance = executor
            .instance(app_exe)
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;

        // SAFETY: `*executor` outlives every move of this struct, and `instance` drops first.
        let instance: RvrPureInstance<'static> = unsafe { std::mem::transmute(instance) };

        Ok(Self { instance, executor })
    }

    /// Runs `stdin` on the instance.
    pub(crate) fn execute(&self, stdin: StdIn) -> Result<(PublicValues, Duration), Error> {
        let start = Instant::now();
        let state = self
            .instance
            .execute(stdin)
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;
        let execution_duration = start.elapsed();

        Ok((extract_public_values(&state), execution_duration))
    }
}

pub(crate) fn extract_public_values(state: &VmState<GuestMemory>) -> PublicValues {
    public_values::extract_public_values(NUM_PUBLIC_VALUES_BYTES, &state.memory.memory).into()
}
