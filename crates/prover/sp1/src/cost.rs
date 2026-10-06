use std::{array, ops::Range, sync::Arc};

use ere_prover_core::{CostEstimation, CostProfile, PublicValues, SymbolMap};
use sp1_core_executor::{
    ExecutionMode, GasEstimatingVM, GasEstimatingVMEnum, Program, RiscvAirId, SP1CoreOpts,
    SupervisorMode, TraceChunkRaw, get_complexity_mapping, rv64im_costs,
};

use crate::{
    cost::profile::{Profiler, riscv_air_id_from_opcode},
    error::{Error, EstimateCostError},
};

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
mod jit;
#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
use crate::cost::jit::Executor;

#[cfg(not(all(target_arch = "x86_64", target_os = "linux")))]
mod portable;
#[cfg(not(all(target_arch = "x86_64", target_os = "linux")))]
use crate::cost::portable::Executor;

mod profile;

const TRACE_AREA_WEIGHT: u64 = 3;

const COMPONENTS: [&str; 3] = ["opcode", "syscall", "system"];

#[derive(Default)]
struct Charges {
    cost: u64,
    syscall: u64,
    system: u64,
    exit_code: u64,
}

impl Charges {
    /// Gas per component, in the order of `COMPONENTS`.
    fn components(&self) -> Result<[u64; 3], EstimateCostError> {
        let opcode = self
            .cost
            .checked_sub(self.syscall + self.system)
            .ok_or(EstimateCostError::Mismatch(self.cost))?;
        Ok([opcode, self.syscall, self.system])
    }

    fn cost_estimation(&self) -> Result<CostEstimation, EstimateCostError> {
        let cost = COMPONENTS
            .iter()
            .zip(self.components()?)
            .map(|(component, gas)| ((*component).to_owned(), gas))
            .collect();
        Ok(CostEstimation { cost })
    }
}

pub(crate) struct SP1CostEstimator {
    program: Arc<Program>,
    /// Function symbols of the guest, which each profile shares.
    symbol_map: Arc<SymbolMap>,
    /// Address ranges of the loadable segments of the guest ELF.
    loadable_segments: Vec<Range<u64>>,
    weights: Vec<u64>,
    executor: Executor,
}

impl SP1CostEstimator {
    pub(crate) fn new(
        program: Arc<Program>,
        symbol_map: Arc<SymbolMap>,
        loadable_segments: Vec<Range<u64>>,
    ) -> Self {
        Self {
            executor: Executor::new(Arc::clone(&program)),
            program,
            symbol_map,
            loadable_segments,
            weights: weights(),
        }
    }

    pub(crate) fn estimate(&self, input: &[u8]) -> Result<(PublicValues, CostEstimation), Error> {
        let (public_values, charges) = self.run(input, None)?;
        Ok((public_values, charges.cost_estimation()?))
    }

    /// Gas of the run per guest call stack, and the peak memory use.
    pub(crate) fn profile(&self, input: &[u8]) -> Result<(PublicValues, CostProfile), Error> {
        let mut profiler = Profiler::new(
            Arc::clone(&self.symbol_map),
            &self.loadable_segments,
            self.program.pc_start_abs,
        );
        let (public_values, charges) = self.run(input, Some(&mut profiler))?;
        let profile = profiler.into_profile();
        assert_eq!(
            profile.cost_estimation(),
            charges.cost_estimation()?,
            "profiled gas must split the estimated gas"
        );
        Ok((public_values, profile))
    }

    /// Runs `input` and charges its gas, per guest call stack too when `profiler` is set.
    fn run(
        &self,
        input: &[u8],
        mut profiler: Option<&mut Profiler>,
    ) -> Result<(PublicValues, Charges), Error> {
        let mut charges = Charges::default();
        let public_values = self.executor.execute(input, |chunk| {
            self.charge(chunk, &mut charges, profiler.as_deref_mut())
        })?;

        if charges.exit_code != 0 {
            return Err(Error::ExecutionFailed(charges.exit_code as u32));
        }

        Ok((public_values.as_slice().into(), charges))
    }

    fn charge(
        &self,
        chunk: &TraceChunkRaw,
        charges: &mut Charges,
        profiler: Option<&mut Profiler>,
    ) -> Result<(), Error> {
        let mut vm = GasEstimatingVMEnum::new(
            chunk,
            Arc::clone(&self.program),
            Default::default(),
            SP1CoreOpts::default(),
        );
        let untrusted = self.program.enable_untrusted_programs;
        let report = match (&mut vm, profiler) {
            (GasEstimatingVMEnum::Supervisor(vm), Some(profiler)) => {
                // `split` covers only this chunk, and `earlier` is the gas before it.
                let earlier = charges.components()?;
                profiler.execute(vm, |vm| {
                    let split = self.split(vm);
                    array::from_fn(|component| earlier[component] + split[component])
                })
            }
            (GasEstimatingVMEnum::User(_), Some(_)) => {
                unreachable!("SP1Prover::profile rejects untrusted programs")
            }
            (vm, None) => vm.execute(),
        }
        .map_err(|err| EstimateCostError::Gas(err.to_string()))?;

        let (complexity, trace_area) = vm.costs();
        charges.cost += TRACE_AREA_WEIGHT * trace_area + complexity;

        let [syscall, system] = match &vm {
            GasEstimatingVMEnum::Supervisor(vm) => self.syscall_and_system(vm, untrusted),
            GasEstimatingVMEnum::User(vm) => self.syscall_and_system(vm, untrusted),
        };
        charges.syscall += syscall;
        charges.system += system;
        charges.exit_code |= report.exit_code;
        Ok(())
    }

    /// Gas per component of the counts in `vm`, for a program without untrusted programs. Their sum
    /// is the gas that `charge` computes from `GasEstimatingVMEnum::costs`.
    fn split(&self, vm: &GasEstimatingVM<'_, SupervisorMode>) -> [u64; 3] {
        let opcode = self.cost_of_rows(
            vm.gas_calculator
                .opcode_counts
                .iter()
                .filter(|(_, count)| **count > 0)
                .map(|(opcode, count)| (riscv_air_id_from_opcode(opcode), *count)),
        );
        let [syscall, system] = self.syscall_and_system(vm, false);
        [opcode, syscall, system]
    }

    /// Gas of the syscall rows and of the system rows of the counts in `vm`.
    fn syscall_and_system<M: ExecutionMode>(
        &self,
        vm: &GasEstimatingVM<'_, M>,
        untrusted: bool,
    ) -> [u64; 2] {
        let counts = &vm.gas_calculator;
        [
            self.cost_of_rows(
                counts
                    .syscall_counts
                    .iter()
                    .chain(counts.deferred_syscall_counts.iter())
                    .filter(|(_, count)| **count > 0)
                    .filter_map(|(code, count)| Some((code.as_air_id_flag(untrusted)?, *count))),
            ),
            self.cost_of_rows(
                counts
                    .system_chips_counts
                    .iter()
                    .map(|(air, count)| (air, *count)),
            ),
        ]
    }

    fn cost_of_rows(&self, rows: impl Iterator<Item = (RiscvAirId, u64)>) -> u64 {
        rows.map(|(air, count)| self.weights[air as usize] * count)
            .sum()
    }
}

fn weights() -> Vec<u64> {
    let cells = rv64im_costs();
    let mut weights = Vec::new();
    for (air, complexity) in get_complexity_mapping() {
        let index = air as usize;
        if index >= weights.len() {
            weights.resize(index + 1, 0);
        }
        weights[index] =
            TRACE_AREA_WEIGHT * cells.get(&air).copied().unwrap_or(0) as u64 + complexity;
    }
    weights
}
