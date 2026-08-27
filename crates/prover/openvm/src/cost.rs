use std::{collections::BTreeMap, env, ops::Range, sync::Arc};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CostEstimation, ERE_COST_ESTIMATION_HEAP_START, PublicValues, symbol_address,
};
use once_cell::sync::OnceCell;
use openvm_circuit::arch::{
    VirtualMachineError, VmExecutor,
    execution_mode::MeteredCtx,
    instructions::{exe::VmExe, riscv::RV64_MEMORY_AS},
    rvr::RvrMeteredInstance,
};
use openvm_sdk::{CpuSdk, F, StdIn};
use openvm_sdk_config::SdkVmConfig;
use openvm_transpiler::openvm_platform::memory::MEM_SIZE;

use crate::{error::Error, executor::extract_public_values, prover::sdk_vm_config};

const DEFAULT_HEAP_START: &str = "_end";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Component {
    Precompile,
    Rv64,
    System,
}

impl Component {
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
    // Never read directly. Owned only to keep `*executor` alive for `instance`.
    #[allow(dead_code)]
    executor: Box<VmExecutor<F, SdkVmConfig>>,
    app_exe: Arc<VmExe<F>>,
    executor_idx_to_air_idx: Vec<usize>,
    ctx: MeteredCtx,
    widths: Vec<usize>,
    components: Vec<Component>,
    heap_range: Option<Range<u64>>,
}

impl CostEstimator {
    pub(crate) fn new(elf: &Elf, app_exe: &Arc<VmExe<F>>, sdk: &CpuSdk) -> Result<Self, Error> {
        let executor = Box::new(
            VmExecutor::new(sdk_vm_config())
                .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?,
        );

        let app_prover = sdk.app_prover(app_exe.clone()).map_err(Error::ProverInit)?;
        let vm = app_prover.vm();
        let ctx = vm.build_metered_ctx(app_exe);
        let widths = vm.build_metered_cost_ctx().widths;
        let executor_idx_to_air_idx = vm.executor_idx_to_air_idx();
        let components = vm.air_names().map(Component::classify).collect();

        let start = env::var(ERE_COST_ESTIMATION_HEAP_START)
            .unwrap_or_else(|_| DEFAULT_HEAP_START.to_owned());
        let heap_range = symbol_address(&elf.0, &start)
            .filter(|start| *start < MEM_SIZE as u64)
            .map(|start| start..MEM_SIZE as u64);

        Ok(Self {
            instance: OnceCell::new(),
            executor,
            app_exe: app_exe.clone(),
            executor_idx_to_air_idx,
            ctx,
            widths,
            components,
            heap_range,
        })
    }

    fn instance(&self) -> Result<&RvrMeteredInstance<'static>, Error> {
        self.instance.get_or_try_init(|| {
            let instance = self
                .executor
                .metered_instance(
                    &self.app_exe,
                    &self.executor_idx_to_air_idx,
                    self.widths.len(),
                )
                .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;

            // SAFETY: `*executor` outlives every move of this struct, and `instance` drops first.
            let instance: RvrMeteredInstance<'static> = unsafe { std::mem::transmute(instance) };

            Ok(instance)
        })
    }

    pub(crate) fn estimate(&self, stdin: StdIn) -> Result<(PublicValues, CostEstimation), Error> {
        let (segments, state) = self
            .instance()?
            .execute_metered(stdin, self.ctx.clone())
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;

        // Trace heights are per segment, so a run costs their sum.
        let mut rows = vec![0u64; self.widths.len()];
        for segment in &segments {
            for (air, height) in segment.trace_heights.iter().enumerate() {
                rows[air] += u64::from(*height);
            }
        }

        let mut cost = BTreeMap::new();
        for (air, component) in self.components.iter().enumerate() {
            *cost.entry(component.as_str().to_owned()).or_insert(0) +=
                rows[air] * self.widths[air] as u64;
        }

        let peak_heap_bytes = self.heap_range.as_ref().and_then(|range| {
            let heap = state
                .memory
                .checked_u8_slice(RV64_MEMORY_AS, range.start, range.end - range.start)
                .ok()?;
            peak_heap_bytes(heap)
        });

        Ok((
            extract_public_values(&state),
            CostEstimation {
                cost,
                peak_heap_bytes,
            },
        ))
    }
}

fn peak_heap_bytes(bytes: &[u8]) -> Option<u64> {
    let highest = bytes.iter().rposition(|byte| *byte != 0)?;
    let lowest = bytes
        .iter()
        .position(|byte| *byte != 0)
        .expect("a heap holding a highest non-zero byte holds a lowest one");
    Some((highest - lowest + 1) as u64)
}
