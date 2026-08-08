//! OpenVM cost estimation.
//!
//! OpenVM prices an execution from trace geometry rather than from opcode counts. Metered cost
//! execution charges every instruction the rows it adds times the width of the AIR that proves it,
//! so the total is the trace cells the app VM has to commit to. It carries no memory model and no
//! segmentation, which makes the number independent of the machine and of the prover backend.
//!
//! The execution reports that total as a single number, so the kinds come from running it once per
//! kind against an artifact priced with only that kind's AIR widths. Metered cost is a weighted sum
//! over the AIRs an execution charges and the weights are the caller's, so zeroing the rest leaves
//! exactly the cells one kind contributes.

use std::{collections::BTreeMap, mem, sync::Arc};

use openvm_circuit::arch::{
    VirtualMachineError, VmExecutor, execution_mode::MeteredCostCtx, instructions::exe::VmExe,
};
use openvm_sdk::{F, StdIn, compiled::MeteredCostInstance};
use openvm_sdk_config::SdkVmConfig;

use crate::error::Error;

/// Cost kind holding the whole.
pub const COST: &str = "cost";

/// Cost kind covering the work a guest buys by reaching for an accelerator.
pub const PRECOMPILE: &str = "precompile";

/// Cost kind covering the work a guest does in the base instruction set.
pub const RV64: &str = "rv64";

/// Cost kinds that partition an OpenVM total, in stack order.
///
/// The split is between the work a guest buys by reaching for an accelerator and the work it does
/// in the base instruction set, which is the comparison worth drawing across guests running the
/// same block. Loads and stores stay under `rv64` because a guest cannot choose them away.
const KINDS: [&str; 2] = [PRECOMPILE, RV64];

/// AIR name fragments the accelerator extensions own, covering Keccak, SHA-2, 256-bit integers and
/// the field expression AIRs the modular, complex and elliptic curve extensions share. Pairing
/// registers no AIR of its own and is charged through those.
const PRECOMPILE_AIRS: [&str; 5] = [
    "Keccakf",
    "Rv64IsEqualModU16",
    "Rv64VecHeap",
    "Sha2",
    "Xorin",
];

/// AIR names no instruction can add a row to, which is why they belong to no kind.
///
/// No executor is bound to any of them. The memory argument reaches its two AIRs through the page
/// hooks metered cost stubs out, the periphery hasher only through the memory argument, and the
/// lookup tables are filled from counters once the execution is over.
const UNCHARGED_AIRS: [&str; 8] = [
    "BitwiseOperationLookupAir",
    "MemoryMerkleAir",
    "PersistentBoundaryAir",
    "Poseidon2PeripheryAir",
    "ProgramAir",
    "RangeTupleCheckerAir",
    "VariableRangeCheckerAir",
    "VmConnectorAir",
];

/// One metered cost artifact per cost kind, plus one priced with every AIR width.
///
/// Each artifact is a C translation of the guest built into a shared library, which is the
/// expensive half of estimating a cost and is paid once per guest here.
///
/// The instances borrow the `SystemConfig` owned by `*executor`, making this self-referential the
/// same way [`crate::executor::Executor`] is. The `'static` lifetime is sound because that config
/// lives behind an `Arc` whose allocation outlives every move of the returned `CostEstimator`, and
/// declaring the instances first drops the borrows before their referent.
pub(crate) struct CostEstimator {
    /// Priced with every AIR width, which is the cost OpenVM's own metered cost mode reports.
    whole: MeteredCostInstance<'static>,
    /// One artifact per kind, each priced with only that kind's AIR widths.
    priced: Vec<(&'static str, MeteredCostInstance<'static>)>,
    // Never read directly. Owned only to keep `*executor` alive for the instances.
    #[allow(dead_code)]
    executor: Box<VmExecutor<F, SdkVmConfig>>,
    /// Shared across the artifacts, since each carries its own widths and the context only
    /// collects what a run accumulated.
    ctx: MeteredCostCtx,
}

impl CostEstimator {
    /// Builds one artifact per cost kind for `app_exe`, where `ctx`, `executor_idx_to_air_idx` and
    /// `air_names` come from the virtual machine the guest is proven by.
    pub(crate) fn new(
        config: SdkVmConfig,
        app_exe: &Arc<VmExe<F>>,
        ctx: MeteredCostCtx,
        executor_idx_to_air_idx: &[usize],
        air_names: &[&str],
    ) -> Result<Self, Error> {
        let kinds: Vec<Option<&'static str>> = air_names.iter().copied().map(kind).collect();

        let masked: Vec<(&'static str, Vec<usize>)> = KINDS
            .iter()
            .map(|&name| {
                let mask = ctx
                    .widths
                    .iter()
                    .zip(&kinds)
                    .map(|(&width, &kind)| if kind == Some(name) { width } else { 0 })
                    .collect();
                (name, mask)
            })
            .collect();
        // Every charged AIR lands in one kind and every uncharged one in none, which is what the
        // per estimate sum against the whole cost then holds the uncharged ones to.
        if !kinds.iter().enumerate().all(|(air, kind)| {
            let priced: usize = masked.iter().map(|(_, mask)| mask[air]).sum();
            priced == kind.map_or(0, |_| ctx.widths[air])
        }) {
            return Err(Error::CostKindsDoNotPartitionAirWidths);
        }

        let executor = Box::new(
            VmExecutor::new(config)
                .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?,
        );

        let compile = |widths: &[usize]| -> Result<MeteredCostInstance<'static>, Error> {
            let instance = executor
                .metered_cost_instance_with_debug_map(
                    app_exe,
                    executor_idx_to_air_idx,
                    widths,
                    None,
                )
                .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;

            // SAFETY: the instance borrows only the `SystemConfig` held in `*executor`, which lives
            // in an `Arc` allocation that stays put while `*executor` is alive, and the `executor`
            // field is dropped after the instances by field order. The borrow therefore never
            // dangles and never outlives its referent, so extending its lifetime to `'static` for
            // co-storage is sound.
            Ok(unsafe {
                mem::transmute::<MeteredCostInstance<'_>, MeteredCostInstance<'static>>(instance)
            })
        };
        let priced = masked
            .iter()
            .map(|(kind, mask)| Ok((*kind, compile(mask)?)))
            .collect::<Result<Vec<_>, Error>>()?;
        let whole = compile(&ctx.widths)?;

        Ok(Self {
            whole,
            priced,
            executor,
            ctx,
        })
    }

    /// Runs `stdin` once per kind and once against the whole cost.
    pub(crate) fn estimate_cost(&self, stdin: StdIn) -> Result<BTreeMap<String, u64>, Error> {
        let mut cost = BTreeMap::new();
        for (kind, instance) in &self.priced {
            cost.insert((*kind).to_owned(), self.execute(instance, &stdin)?);
        }

        // The kinds leave out the AIRs no instruction can charge, so kinds that fall short of the
        // whole cost mean one of them took a row after all.
        let total = self.execute(&self.whole, &stdin)?;
        let summed: u64 = cost.values().sum();
        if summed != total {
            return Err(Error::UnexpectedCostKindsSum { summed, total });
        }
        cost.insert(COST.to_owned(), total);

        Ok(cost)
    }

    fn execute(
        &self,
        instance: &MeteredCostInstance<'static>,
        stdin: &StdIn,
    ) -> Result<u64, Error> {
        // `Sdk::execute_metered_cost` wraps this call to also read the guest's public values, which
        // a cost estimate does not report.
        let (ctx, _) = instance
            .execute_metered_cost(stdin.clone(), self.ctx.clone())
            .map_err(|err| Error::Execute(VirtualMachineError::from(err).into()))?;
        Ok(ctx.cost)
    }
}

/// The kind an AIR is priced under, from the name the proving key records it as, or `None` for the
/// AIRs metered cost never charges.
///
/// Whatever an accelerator does not claim is base instruction execution, which is where the loads,
/// the stores and the hint stream land.
fn kind(air_name: &str) -> Option<&'static str> {
    if UNCHARGED_AIRS
        .iter()
        .any(|fragment| air_name.contains(fragment))
    {
        None
    } else if PRECOMPILE_AIRS
        .iter()
        .any(|fragment| air_name.contains(fragment))
    {
        Some(PRECOMPILE)
    } else {
        Some(RV64)
    }
}

#[cfg(test)]
mod tests {
    use crate::estimator::{KINDS, PRECOMPILE, RV64, kind};

    /// The AIR names the app config keygens to, one per line, as the proving key records them.
    /// Regenerated whenever the pinned OpenVM changes the AIR set.
    const AIR_NAMES: &str = include_str!("testdata/openvm-air-names.txt");

    /// A fragment that stops matching moves real cost between kinds without failing anything else,
    /// so the split each fragment set makes is pinned here. The last count is the AIRs left to no
    /// kind, which an estimated cost then holds to zero.
    #[test]
    fn the_kinds_split_the_air_set_as_expected() {
        let counts = AIR_NAMES.lines().fold([0; 3], |mut counts, air_name| {
            let index = match kind(air_name) {
                Some(kind) => KINDS
                    .iter()
                    .position(|name| *name == kind)
                    .expect("every kind is listed"),
                None => KINDS.len(),
            };
            counts[index] += 1;
            counts
        });
        assert_eq!(counts, [51, 39, 8]);
    }

    #[test]
    fn kinds_follow_the_extension_an_air_belongs_to() {
        assert_eq!(kind("KeccakfPermAir"), Some(PRECOMPILE));
        assert_eq!(kind("Sha2BlockHasherVmAir<Sha256Config>"), Some(PRECOMPILE));
        assert_eq!(
            kind("VmAirWrapper<Rv64VecHeapAdapterAir<1, 12, 12>, FieldExpressionCoreAir>"),
            Some(PRECOMPILE)
        );
        // A 256-bit add reads its operands from the heap, where the base one reads registers.
        assert_eq!(
            kind(
                "VmAirWrapper<Rv64VecHeapU16AdapterAir<2, 4, 4>, 2, 4, 4, 4, 16, 16>, AddSubCoreAir<16, 16, true>"
            ),
            Some(PRECOMPILE)
        );
        assert_eq!(
            kind("VmAirWrapper<Rv64BaseAluRegU16AdapterAir, AddSubCoreAir<4, 16, true>"),
            Some(RV64)
        );
        assert_eq!(
            kind("VmAirWrapper<Rv64LoadMultiByteAdapterAir, LoadCoreAir<8, 5>"),
            Some(RV64)
        );
        assert_eq!(kind("Rv64HintStoreAir"), Some(RV64));
        assert_eq!(kind("PhantomAir"), Some(RV64));
        assert_eq!(kind("MemoryMerkleAir<8>"), None);
        assert_eq!(kind("Poseidon2PeripheryAir<BabyBearParameters>, 1>"), None);
        assert_eq!(kind("VariableRangeCheckerAir"), None);
    }
}
