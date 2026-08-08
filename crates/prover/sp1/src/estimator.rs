//! SP1 cost estimation.
//!
//! SP1 prices an execution as gas, which weighs the trace cells the AIRs an execution touches have
//! to commit to against the constraints those AIRs evaluate. A minimal trace of the run is charged
//! one chunk at a time, the same path the SDK takes when it reports gas. Gas divides that weighted
//! sum by 191 with a rounding step per chunk, so the estimate records the sum, which stays exact.
//!
//! The sum splits by the accumulators SP1 charges through rather than by the two weightings, since
//! every AIR carries both weightings and only the accumulators track what the program did.

use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use enum_map::EnumMap;
use sp1_core_executor::{
    GAS_TRACE_CHUNK_THRESHOLD, GasEstimatingVMEnum, MinimalExecutorEnum, Program, RiscvAirId,
    SP1CoreOpts, get_complexity_mapping,
};
use sp1_core_machine::riscv::RiscvAir;
use sp1_primitives::SP1Field;

use crate::error::Error;

/// Cost kind holding the whole.
pub const COST: &str = "cost";

/// Cost kind covering the instructions the program runs.
pub const OPCODE: &str = "opcode";

/// Cost kind covering the precompiles the program calls, charged in place or deferred to a later
/// shard.
pub const SYSCALL: &str = "syscall";

/// Cost kind covering the chips the machine runs on the program's behalf rather than at its
/// request.
pub const SYSTEM: &str = "system";

/// Weight gas gives a trace cell relative to a constraint.
const TRACE_AREA_WEIGHT: u64 = 3;

/// Nonce the execution commits to, which reaches the public values rather than the cost.
const PROOF_NONCE: [u32; 4] = [0; 4];

pub(crate) struct CostEstimator {
    program: Arc<Program>,
    opts: SP1CoreOpts,
    /// What one row of each AIR costs, which is the gas formula's two weightings already blended.
    weights: EnumMap<RiscvAirId, u64>,
}

impl CostEstimator {
    pub(crate) fn new(program: Arc<Program>) -> Self {
        Self {
            program,
            opts: SP1CoreOpts::default(),
            weights: weights(),
        }
    }

    /// Charges a minimal trace of `stdin` one chunk at a time.
    pub(crate) fn estimate_cost(&self, stdin: &[u8]) -> Result<BTreeMap<String, u64>, Error> {
        // Gas accounts a chunk boundary as a shard boundary, so it grows as chunks get smaller. The
        // cadence gas was calibrated against is pinned here rather than the smaller one that only
        // bounds the executor's own memory.
        let mut executor = MinimalExecutorEnum::new(
            Arc::clone(&self.program),
            false,
            Some(GAS_TRACE_CHUNK_THRESHOLD),
        );
        // SP1 hands the guest one input entry per read, so the stdin goes in whole and carries no
        // framing.
        executor.with_input(stdin);

        let untrusted = self.program.enable_untrusted_programs;
        let (mut cost, mut syscall, mut system, mut exit_code) = (0, 0, 0, 0);
        while let Some(chunk) = executor
            .try_execute_chunk()
            .map_err(|err| Error::Execute(err.into()))?
        {
            let mut vm = GasEstimatingVMEnum::new(
                &chunk,
                Arc::clone(&self.program),
                PROOF_NONCE,
                self.opts.clone(),
            );
            let report = vm.execute().map_err(|err| Error::Execute(err.into()))?;
            let counts = match &vm {
                GasEstimatingVMEnum::Supervisor(vm) => &vm.gas_calculator,
                GasEstimatingVMEnum::User(vm) => &vm.gas_calculator,
            };

            let (complexity, trace_area) = vm.costs();
            cost += TRACE_AREA_WEIGHT * trace_area + complexity;
            // Whether a precompile is charged in place or deferred to a later shard follows the
            // retained event presets, which moves rows between the two maps without repricing them.
            syscall += self.charge(
                counts
                    .syscall_counts
                    .iter()
                    .chain(counts.deferred_syscall_counts.iter())
                    .filter_map(|(code, count)| Some((code.as_air_id_flag(untrusted)?, *count))),
            );
            system += self.charge(
                counts
                    .system_chips_counts
                    .iter()
                    .map(|(air, count)| (air, *count)),
            );
            // Only the chunk the guest halts in carries an exit code, which is how SP1 folds them.
            exit_code |= report.exit_code;
        }
        if exit_code != 0 {
            return Err(Error::ExecutionFailed(exit_code));
        }

        // Instructions are what the total has left once the precompiles and the machine's own chips
        // are taken out, which keeps the parts summing to SP1's own figure rather than to a second
        // reading of the same weights.
        let opcode = cost
            .checked_sub(syscall + system)
            .ok_or(Error::UnexpectedCostKindsSum {
                summed: syscall + system,
                total: cost,
            })?;

        Ok(BTreeMap::from([
            (OPCODE.to_owned(), opcode),
            (SYSCALL.to_owned(), syscall),
            (SYSTEM.to_owned(), system),
            (COST.to_owned(), cost),
        ]))
    }

    /// Charges every row at the weight of the AIR that proves it.
    fn charge(&self, rows: impl Iterator<Item = (RiscvAirId, u64)>) -> u64 {
        rows.map(|(air, count)| self.weights[air] * count).sum()
    }
}

/// What one row of each AIR costs, which is its trace cells weighed against its constraints.
///
/// SP1 keeps the two tables apart because gas blends them only at the end. Blending them per AIR
/// instead leaves the same total while letting it split by what the program did.
fn weights() -> EnumMap<RiscvAirId, u64> {
    let complexity = get_complexity_mapping();
    let mut cells: EnumMap<RiscvAirId, u64> = EnumMap::default();
    for (name, cost) in RiscvAir::<SP1Field>::costs() {
        cells[RiscvAirId::from_str(&name).expect("SP1 names its own AIRs")] = cost;
    }
    EnumMap::from_fn(|air| TRACE_AREA_WEIGHT * cells[air] + complexity[air])
}
