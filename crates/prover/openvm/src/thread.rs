use std::{
    env,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    thread::{self, JoinHandle},
};

use ere_prover_core::ProverResource;
use openvm_circuit::arch::{
    ContinuationProverBuilder, VirtualMachineError,
    instructions::{VM_DIGEST_WIDTH, exe::VmExe},
};
use openvm_sdk::{
    DeferralSetup, F, SC, StdIn,
    keygen::AppProvingKey,
    prover::{AggProver, StarkProver},
};
use openvm_sdk_config::{SdkVmConfig, SdkVmCpuBuilder};
use openvm_stark_sdk::{
    config::baby_bear_poseidon2::BabyBearPoseidon2CpuEngine, openvm_stark_backend::StarkEngine,
};
use openvm_verify_stark_host::VmStarkProof;

use crate::{baseline::app_exe_commit, error::Error};

pub(crate) enum Request {
    Commit(
        Arc<VmExe<F>>,
        SyncSender<Result<[F; VM_DIGEST_WIDTH], Error>>,
    ),
    Setup(Arc<VmExe<F>>, SyncSender<Result<(), Error>>),
    Reset(SyncSender<Result<(), Error>>),
    Prove(
        Arc<VmExe<F>>,
        StdIn,
        SyncSender<Result<VmStarkProof, Error>>,
    ),
}

/// Owns the `StarkProver` of the current program, which is not `Send`, on one thread.
pub(crate) struct ProverThread {
    requests: Option<Sender<Request>>,
    handle: Option<JoinHandle<()>>,
}

impl ProverThread {
    pub(crate) fn spawn(
        resource: &ProverResource,
        app_pk: AppProvingKey<SdkVmConfig>,
        agg_prover: Arc<AggProver>,
    ) -> Self {
        let (requests, receiver) = mpsc::channel();
        let handle = match resource {
            #[cfg(feature = "cuda")]
            ProverResource::Gpu => thread::spawn(move || {
                serve::<openvm_sdk::DefaultStarkEngine, openvm_sdk_config::SdkVmGpuBuilder>(
                    app_pk, agg_prover, receiver, true,
                )
            }),
            _ => thread::spawn(move || {
                serve::<BabyBearPoseidon2CpuEngine, SdkVmCpuBuilder>(
                    app_pk, agg_prover, receiver, false,
                )
            }),
        };
        Self {
            requests: Some(requests),
            handle: Some(handle),
        }
    }

    pub(crate) fn request<T>(
        &self,
        request: impl FnOnce(SyncSender<Result<T, Error>>) -> Request,
    ) -> Result<T, Error> {
        let (reply, response) = mpsc::sync_channel(1);
        self.requests
            .as_ref()
            .expect("set until drop")
            .send(request(reply))
            .map_err(|_| Error::ProverThreadPanicked)?;
        response.recv().map_err(|_| Error::ProverThreadPanicked)?
    }
}

impl Drop for ProverThread {
    // The prover unloads its rvr libraries before the process exits.
    fn drop(&mut self) {
        drop(self.requests.take());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn serve<E, VB>(
    app_pk: AppProvingKey<SdkVmConfig>,
    agg_prover: Arc<AggProver>,
    requests: Receiver<Request>,
    gpu: bool,
) where
    E: StarkEngine<SC = SC>,
    VB: Default + ContinuationProverBuilder<E, VmConfig = SdkVmConfig>,
{
    let engine = E::new(app_pk.app_vm_pk.get_params());
    let mut prover: Option<StarkProver<E, VB>> = None;
    for request in requests {
        match request {
            Request::Commit(app_exe, reply) => {
                let _ = reply.send(
                    catch_unwind(AssertUnwindSafe(|| {
                        app_exe_commit(&engine, &app_pk, &app_exe)
                    }))
                    .map_err(|_| Error::ProverThreadPanicked),
                );
            }
            Request::Setup(app_exe, reply) => {
                let _ = reply.send(with_prover(
                    &mut prover,
                    &app_pk,
                    &agg_prover,
                    &app_exe,
                    |prover| build_rvr_libraries(prover, &app_exe, gpu),
                ));
            }
            Request::Reset(reply) => {
                prover = None;
                let _ = reply.send(Ok(()));
            }
            Request::Prove(app_exe, stdin, reply) => {
                let _ = reply.send(with_prover(
                    &mut prover,
                    &app_pk,
                    &agg_prover,
                    &app_exe,
                    |prover| {
                        let (proof, _) = prover
                            .prove(stdin, &[])
                            .map_err(|err| Error::Prove(err.into()))?;
                        Ok(proof)
                    },
                ));
            }
        }
    }
}

/// Runs `f` on the prover of `app_exe`, which replaces the prover of another program. An error or
/// a panic drops the prover, because it can leave the prover without its execution state.
fn with_prover<E, VB, T>(
    prover: &mut Option<StarkProver<E, VB>>,
    app_pk: &AppProvingKey<SdkVmConfig>,
    agg_prover: &Arc<AggProver>,
    app_exe: &Arc<VmExe<F>>,
    f: impl FnOnce(&mut StarkProver<E, VB>) -> Result<T, Error>,
) -> Result<T, Error>
where
    E: StarkEngine<SC = SC>,
    VB: Default + ContinuationProverBuilder<E, VmConfig = SdkVmConfig>,
{
    let result = catch_unwind(AssertUnwindSafe(|| {
        if prover
            .as_ref()
            .is_none_or(|prover| !Arc::ptr_eq(prover.app_prover.instance().exe(), app_exe))
        {
            // Drops the old prover first, so its rvr libraries unload before new ones load.
            *prover = None;
            *prover = Some(stark_prover(app_pk, agg_prover, app_exe)?);
        }
        f(prover.as_mut().unwrap())
    }))
    .unwrap_or_else(|_| Err(Error::ProverThreadPanicked));
    if result.is_err() {
        *prover = None;
    }
    result
}

/// Builds a prover of `app_exe` on the shared aggregation prover.
fn stark_prover<E, VB>(
    app_pk: &AppProvingKey<SdkVmConfig>,
    agg_prover: &Arc<AggProver>,
    app_exe: &Arc<VmExe<F>>,
) -> Result<StarkProver<E, VB>, Error>
where
    E: StarkEngine<SC = SC>,
    VB: Default + ContinuationProverBuilder<E, VmConfig = SdkVmConfig>,
{
    StarkProver::new(
        VB::default(),
        &app_pk.app_vm_pk,
        app_exe.clone(),
        agg_prover.clone(),
        DeferralSetup::Disabled,
    )
    .map_err(|err| Error::ProverInit(err.into()))
}

/// Builds the rvr libraries that a prove uses in parallel, so the first prove loads them from the
/// cache. The GPU prove builds its own preflight library, while the CPU one interprets preflight.
fn build_rvr_libraries<E, VB>(
    prover: &StarkProver<E, VB>,
    app_exe: &VmExe<F>,
    gpu: bool,
) -> Result<(), Error>
where
    E: StarkEngine<SC = SC>,
    VB: ContinuationProverBuilder<E, VmConfig = SdkVmConfig>,
{
    if env::var_os("OPENVM_RVR_NATIVE_CACHE_DIR").is_none() {
        return Ok(());
    }
    let vm = &prover.app_prover.instance().vm;
    let (executor, executor_idx_to_air_idx) = (vm.executor(), vm.executor_idx_to_air_idx());
    thread::scope(|scope| {
        let preflight = gpu.then(|| scope.spawn(|| executor.preflight_instance(app_exe).map(drop)));
        executor.metered_instance(app_exe, &executor_idx_to_air_idx, vm.num_airs())?;
        preflight.map_or(Ok(()), |preflight| preflight.join().unwrap())
    })
    .map_err(|err| Error::ProverInit(VirtualMachineError::from(err).into()))
}
