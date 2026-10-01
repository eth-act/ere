use std::{array, ops::Range, sync::Arc};

use ere_prover_core::{CostEstimation, CostProfile, PublicValues, SymbolMap, loadable_segments};
use once_cell::sync::OnceCell;
use openvm_circuit::arch::{
    MeteredExecutor, VirtualMachineError, VmExecutionConfig, VmExecutor,
    execution_mode::{MeteredCtx, Segment},
    instructions::exe::{FnBound, VmExe},
    rvr::RvrMeteredInstance,
};
use openvm_sdk::{F, StdIn, keygen::AppProvingKey, prover::AppProver};
use openvm_sdk_config::{SdkVmConfig, SdkVmCpuBuilder};
use openvm_stark_sdk::config::baby_bear_poseidon2::BabyBearPoseidon2CpuEngine;

use crate::{
    cost::profile::{ProfileConfig, Profiler},
    error::Error,
    executor::extract_public_values,
    prover::sdk_vm_config,
};

mod profile;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Component {
    Precompile,
    Rv64,
    System,
}

impl Component {
    const ALL: [Self; 3] = [Self::Precompile, Self::Rv64, Self::System];

    /// An AIR name carries its adapter and core in generics, so each entry is a pattern.
    /// The lookups, the range checkers and the memory argument serve both the precompiles and
    /// plain RISC-V work, so they form their own component.
    fn classify(air_name: &str) -> Self {
        const SYSTEM: &[&str] = &[
            "BitwiseOperationLookupAir",
            "MemoryMerkleAir",
            "PersistentBoundaryAir",
            "Poseidon2PeripheryAir",
            "ProgramAir",
            "RangeTupleCheckerAir",
            "VariableRangeCheckerAir",
            "VmConnectorAir",
        ];
        const PRECOMPILE: &[&str] = &[
            "Keccakf",
            "Rv64IsEqualModU16",
            "Rv64VecHeap",
            "Sha2",
            "Xorin",
        ];
        let matches = |patterns: &[&str]| patterns.iter().any(|pat| air_name.contains(pat));
        if matches(SYSTEM) {
            Self::System
        } else if matches(PRECOMPILE) {
            Self::Precompile
        } else {
            Self::Rv64
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Precompile => "precompile",
            Self::Rv64 => "rv64",
            Self::System => "system",
        }
    }
}

/// `instance` is built on the first estimate, because a second `rvr` shared library in the process
/// crashes it at exit and a caller that only executes or proves never needs one.
pub(crate) struct CostEstimator {
    instance: OnceCell<RvrMeteredInstance<'static>>,
    profile_instance: OnceCell<RvrMeteredInstance<'static>>,
    executor: Box<VmExecutor<F, SdkVmConfig>>,
    profile_executor: Box<VmExecutor<F, ProfileConfig>>,
    app_exe: Arc<VmExe<F>>,
    executor_idx_to_air_idx: Vec<usize>,
    ctx: MeteredCtx,
    /// Width of each AIR per component, 0 for an AIR of another component.
    widths: [Vec<u32>; 3],
    /// Function symbols of the guest, which each profile shares.
    symbol_map: Arc<SymbolMap>,
    /// Address ranges of the loadable segments of the guest ELF.
    loadable_segments: Vec<Range<u64>>,
}

impl CostEstimator {
    pub(crate) fn new(
        elf: &[u8],
        app_exe: &Arc<VmExe<F>>,
        app_pk: &AppProvingKey<SdkVmConfig>,
    ) -> Result<Self, Error> {
        let symbol_map = Arc::new(SymbolMap::from_elf(elf)?);
        let loadable_segments = loadable_segments(elf)?;
        let executor = Box::new(
            VmExecutor::new(sdk_vm_config())
                .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?,
        );
        let profile_executor = Box::new(
            VmExecutor::new(ProfileConfig(sdk_vm_config()))
                .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?,
        );

        let app_prover = AppProver::<BabyBearPoseidon2CpuEngine, SdkVmCpuBuilder>::new(
            SdkVmCpuBuilder,
            &app_pk.app_vm_pk,
            app_exe.clone(),
        )
        .map_err(|err| Error::ProverInit(err.into()))?;
        let vm = app_prover.vm();
        let ctx = vm.build_metered_ctx(app_exe);
        let widths = vm.build_metered_cost_ctx().widths;
        let executor_idx_to_air_idx = vm.executor_idx_to_air_idx();
        let components: Vec<Component> = vm.air_names().map(Component::classify).collect();
        let widths = Component::ALL.map(|component| {
            widths
                .iter()
                .zip(&components)
                .map(|(width, air)| if *air == component { *width as u32 } else { 0 })
                .collect()
        });

        Ok(Self {
            instance: OnceCell::new(),
            profile_instance: OnceCell::new(),
            executor,
            profile_executor,
            app_exe: app_exe.clone(),
            executor_idx_to_air_idx,
            ctx,
            widths,
            symbol_map,
            loadable_segments,
        })
    }

    fn instance(&self) -> Result<&RvrMeteredInstance<'static>, Error> {
        self.instance
            .get_or_try_init(|| self.metered_instance(&self.executor, &self.app_exe))
    }

    /// The profile library reports each edge that enters another function or a gap between
    /// functions, so it compiles a copy of the program whose function bounds start each of them.
    fn profile_instance(&self) -> Result<&RvrMeteredInstance<'static>, Error> {
        self.profile_instance.get_or_try_init(|| {
            let mut app_exe = (*self.app_exe).clone();
            // The code generator reads only the starts, so each bound starts a function or a gap.
            // https://github.com/han0110/openvm/blob/f73d411c192153cc5f4dc0f4794944605ec6dc9e/crates/vm/src/arch/rvr/compile.rs#L595-L599
            app_exe.fn_bounds = self
                .symbol_map
                .starts()
                .map(|start| {
                    let start = start as u32;
                    let bound = FnBound {
                        start,
                        end: start,
                        name: String::new(),
                    };
                    (start, bound)
                })
                .collect();
            self.metered_instance(&self.profile_executor, &app_exe)
        })
    }

    fn metered_instance<VC>(
        &self,
        executor: &VmExecutor<F, VC>,
        app_exe: &VmExe<F>,
    ) -> Result<RvrMeteredInstance<'static>, Error>
    where
        VC: VmExecutionConfig<F>,
        VC::Executor: MeteredExecutor<F>,
    {
        let instance = executor
            .metered_instance(
                app_exe,
                &self.executor_idx_to_air_idx,
                self.ctx.trace_heights.len(),
            )
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;

        // SAFETY: Each caller passes a boxed field of `self` that is declared after the field that
        // stores the instance, so `*executor` outlives every move of `self` and the instance drops
        // first.
        let instance: RvrMeteredInstance<'static> = unsafe { std::mem::transmute(instance) };

        Ok(instance)
    }

    pub(crate) fn estimate(&self, stdin: StdIn) -> Result<(PublicValues, CostEstimation), Error> {
        let (segments, state) = self
            .instance()?
            .execute_metered(stdin, self.ctx.clone())
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;
        let cost = Component::ALL
            .iter()
            .zip(run_cost(&self.widths, &segments))
            .map(|(component, cost)| (component.as_str().to_owned(), cost))
            .collect();
        Ok((extract_public_values(&state), CostEstimation { cost }))
    }

    /// Splits the cost of [`Self::estimate`] over the guest call stacks, and measures the peak
    /// memory use.
    pub(crate) fn profile(&self, stdin: StdIn) -> Result<(PublicValues, CostProfile), Error> {
        let mut profiler = Profiler::new(
            Arc::clone(&self.symbol_map),
            &self.loadable_segments,
            &self.app_exe,
            &Component::ALL.map(|component| component.as_str()),
            self.widths.clone(),
            &self.ctx,
        );
        let instance = self.profile_instance()?;
        let (segments, state) = profiler
            .run(|| instance.execute_metered(stdin, self.ctx.clone()))
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;
        Ok((
            extract_public_values(&state),
            profiler.into_profile(&segments),
        ))
    }
}

/// Cost per component of the rows at `heights`, one height per AIR.
fn rows_cost(widths: &[Vec<u32>; 3], heights: &[u32]) -> [u64; 3] {
    widths.each_ref().map(|widths| {
        heights
            .iter()
            .zip(widths)
            .map(|(height, width)| u64::from(*height) * u64::from(*width))
            .sum()
    })
}

/// Cost per component of a run. Trace heights are per segment, so a run costs their sum.
fn run_cost(widths: &[Vec<u32>; 3], segments: &[Segment]) -> [u64; 3] {
    segments
        .iter()
        .map(|segment| rows_cost(widths, &segment.trace_heights))
        .fold([0; 3], add_cost)
}

/// Cost per component of `left` and `right` together.
fn add_cost(left: [u64; 3], right: [u64; 3]) -> [u64; 3] {
    array::from_fn(|index| left[index] + right[index])
}
