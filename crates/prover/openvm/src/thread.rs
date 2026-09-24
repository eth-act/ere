use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender, SyncSender},
    },
    thread::{self, JoinHandle},
};

use ere_prover_core::ProverResource;
use openvm_circuit::arch::{ContinuationProverBuilder, instructions::exe::VmExe};
use openvm_sdk::{
    DeferralSetup, F, SC, StdIn,
    keygen::AppProvingKey,
    prover::{AggProver, StarkProver},
};
use openvm_sdk_config::{SdkVmConfig, SdkVmCpuBuilder};
use openvm_stark_sdk::{
    config::baby_bear_poseidon2::BabyBearPoseidon2CpuEngine, openvm_stark_backend::StarkEngine,
};
use openvm_verify_stark_host::{VmStarkProof, vk::VerificationBaseline};

use crate::error::Error;

pub(crate) enum Request {
    Setup(
        Arc<VmExe<F>>,
        SyncSender<Result<VerificationBaseline, Error>>,
    ),
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
                    app_pk, agg_prover, receiver,
                )
            }),
            _ => thread::spawn(move || {
                serve::<BabyBearPoseidon2CpuEngine, SdkVmCpuBuilder>(app_pk, agg_prover, receiver)
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
) where
    E: StarkEngine<SC = SC>,
    VB: Default + ContinuationProverBuilder<E, VmConfig = SdkVmConfig>,
{
    let mut prover: Option<StarkProver<E, VB>> = None;
    for request in requests {
        match request {
            Request::Setup(app_exe, reply) => {
                let _ = reply.send(with_prover(
                    &mut prover,
                    &app_pk,
                    &agg_prover,
                    &app_exe,
                    |prover| Ok(prover.generate_baseline()),
                ));
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

/// Runs `f` on the prover of `app_exe`, which replaces the prover of another program. A panic drops
/// the prover.
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
    catch_unwind(AssertUnwindSafe(|| {
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
    .unwrap_or_else(|_| {
        *prover = None;
        Err(Error::ProverThreadPanicked)
    })
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
