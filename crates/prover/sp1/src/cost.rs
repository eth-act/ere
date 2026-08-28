use std::{collections::BTreeMap, env, sync::Arc};

use ere_prover_core::{
    CostEstimation, ERE_COST_ESTIMATION_HEAP_START, PublicValues, symbol_address,
};
use sp1_core_executor::{
    GasEstimatingVMEnum, Program, RiscvAirId, SP1CoreOpts, TraceChunkRaw, get_complexity_mapping,
    rv64im_costs,
};

use crate::error::{Error, EstimateCostError};

#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
mod jit;
#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
use crate::cost::jit::Executor;

#[cfg(not(all(target_arch = "x86_64", target_os = "linux")))]
mod portable;
#[cfg(not(all(target_arch = "x86_64", target_os = "linux")))]
use crate::cost::portable::Executor;

const DEFAULT_HEAP_START: &str = "_end";

const TRACE_AREA_WEIGHT: u64 = 3;

#[derive(Default)]
struct Charges {
    cost: u64,
    syscall: u64,
    system: u64,
    exit_code: u64,
}

pub(crate) struct SP1CostEstimator {
    program: Arc<Program>,
    weights: Vec<u64>,
    executor: Executor,
}

impl SP1CostEstimator {
    pub(crate) fn new(program: Arc<Program>, elf: &[u8]) -> Self {
        let start = env::var(ERE_COST_ESTIMATION_HEAP_START)
            .unwrap_or_else(|_| DEFAULT_HEAP_START.to_owned());
        Self {
            executor: Executor::new(Arc::clone(&program), symbol_address(elf, &start)),
            program,
            weights: weights(),
        }
    }

    pub(crate) fn estimate(&self, input: &[u8]) -> Result<(PublicValues, CostEstimation), Error> {
        let mut charges = Charges::default();
        let (peak_heap_bytes, public_values) = self
            .executor
            .execute(input, |chunk| self.charge(chunk, &mut charges))?;

        if charges.exit_code != 0 {
            return Err(Error::ExecutionFailed(charges.exit_code as u32));
        }

        let opcode = charges
            .cost
            .checked_sub(charges.syscall + charges.system)
            .ok_or(EstimateCostError::Mismatch(charges.cost))?;

        Ok((
            public_values.as_slice().into(),
            CostEstimation {
                cost: BTreeMap::from([
                    ("opcode".to_owned(), opcode),
                    ("syscall".to_owned(), charges.syscall),
                    ("system".to_owned(), charges.system),
                ]),
                peak_heap_bytes,
            },
        ))
    }

    fn charge(&self, chunk: &TraceChunkRaw, charges: &mut Charges) -> Result<(), Error> {
        let mut vm = GasEstimatingVMEnum::new(
            chunk,
            Arc::clone(&self.program),
            Default::default(),
            SP1CoreOpts::default(),
        );
        let report = vm
            .execute()
            .map_err(|err| EstimateCostError::Gas(err.to_string()))?;
        let counts = match &vm {
            GasEstimatingVMEnum::Supervisor(vm) => &vm.gas_calculator,
            GasEstimatingVMEnum::User(vm) => &vm.gas_calculator,
        };
        let cost_of_rows = |rows: &mut dyn Iterator<Item = (RiscvAirId, u64)>| -> u64 {
            rows.map(|(air, count)| self.weights[air as usize] * count)
                .sum()
        };

        let (complexity, trace_area) = vm.costs();
        charges.cost += TRACE_AREA_WEIGHT * trace_area + complexity;

        let untrusted = self.program.enable_untrusted_programs;
        charges.syscall += cost_of_rows(
            &mut counts
                .syscall_counts
                .iter()
                .chain(counts.deferred_syscall_counts.iter())
                .filter_map(|(code, count)| Some((code.as_air_id_flag(untrusted)?, *count))),
        );
        charges.system += cost_of_rows(
            &mut counts
                .system_chips_counts
                .iter()
                .map(|(air, count)| (air, *count)),
        );
        charges.exit_code |= report.exit_code;
        Ok(())
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
