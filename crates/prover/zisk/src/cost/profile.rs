use ere_prover_core::{Frame, PeakMemory, StackPointerWrite};
use zisk_core::{
    InstContext, SRC_IND, SRC_MEM, STORE_IND, STORE_MEM, STORE_REG, ZiskInst,
    zisk_ops::{OpStats, ZiskOp, ops_keccak},
};
use ziskemu::{MAIN_COST, StatsCosts};

use crate::cost::COMPONENTS;

/// Cost that the steps have spent, in the order of `COMPONENTS`. It is the report cost without
/// `BASE_COST` and the ROM and RAM init.
/// https://github.com/0xPolygonHermez/zisk/blob/v1.3.1-alpha/emulator/src/stats/stats.rs#L2840-L2848
pub(crate) fn running_cost(costs: &StatsCosts) -> [u64; COMPONENTS.len()] {
    [
        0,
        costs.precompiled_ops_cost(),
        costs.mops.get_cost(),
        costs.base_ops_cost(),
        costs.steps * MAIN_COST,
    ]
}

/// How the step of `instruction` writes `x2`.
#[inline]
pub(crate) fn stack_pointer_write(instruction: &ZiskInst) -> StackPointerWrite {
    if instruction.store != STORE_REG || instruction.store_offset != 2 {
        StackPointerWrite::Other
    } else if matches!(instruction.b_src, SRC_MEM | SRC_IND) {
        StackPointerWrite::Load
    } else if matches!(instruction.riscv_inst.as_deref(), Some("lui" | "auipc")) {
        StackPointerWrite::UpperImmediate
    } else {
        StackPointerWrite::Other
    }
}

/// Records the memory that the step of `instruction` in `frame` accessed, with `context` as the
/// step left it, which holds the operands that the accesses used.
///
/// The address and width rules are copies of `source_a`, `source_b` and `store_c` of ziskemu.
/// https://github.com/0xPolygonHermez/zisk/blob/v1.3.1-alpha/emulator/src/emu.rs#L180-L197
/// https://github.com/0xPolygonHermez/zisk/blob/v1.3.1-alpha/emulator/src/emu.rs#L502-L533
/// https://github.com/0xPolygonHermez/zisk/blob/v1.3.1-alpha/emulator/src/emu.rs#L1070-L1134
#[inline]
pub(crate) fn record_accesses(
    memory: &mut PeakMemory,
    frame: Frame,
    instruction: &ZiskInst,
    context: &InstContext,
) {
    let mut accesses = Accesses { memory, frame };
    if instruction.a_src == SRC_MEM {
        let mut address = instruction.a_offset_imm0;
        if instruction.a_use_sp_imm1 != 0 {
            address += context.sp;
        }
        accesses.access(address, 8);
    }
    match instruction.b_src {
        SRC_MEM => {
            let mut addr = instruction.b_offset_imm0;
            if instruction.b_use_sp_imm1 != 0 {
                addr += context.sp;
            }
            accesses.access(addr, 8);
        }
        SRC_IND => {
            let mut addr = (context.a as i64 + instruction.b_offset_imm0 as i64) as u64;
            if instruction.b_use_sp_imm1 != 0 {
                addr += context.sp;
            }
            accesses.access(addr, instruction.ind_width);
        }
        _ => {}
    }
    match instruction.store {
        STORE_MEM => {
            let mut addr: i64 = instruction.store_offset;
            if instruction.store_use_sp {
                addr += context.sp as i64;
            }
            accesses.access(addr as u64, 8);
        }
        STORE_IND => {
            let mut addr = instruction.store_offset;
            if instruction.store_use_sp {
                addr += context.sp as i64;
            }
            addr += context.a as i64;
            accesses.access(addr as u64, instruction.ind_width);
        }
        _ => {}
    }
    // Records the parameter reads of `opc_fcall_param`, whose stats hook is `ops_none`.
    // https://github.com/0xPolygonHermez/zisk/blob/v1.3.1-alpha/core/src/zisk_ops.rs#L2054-L2079
    if instruction.op == ZiskOp::FCALL_PARAM && context.a > 1 {
        accesses.access(context.b, 8 * context.a);
    }
    if instruction.input_size > 0 {
        record_precompile_accesses(&mut accesses, instruction, context);
    }
}

/// Records the accesses that the stats hook of a precompile reports, and those of keccak, whose
/// hook is `ops_none`.
/// https://github.com/0xPolygonHermez/zisk/blob/v1.3.1-alpha/core/src/zisk_ops.rs#L565
///
/// It stays out of line so that [`record_accesses`] inlines into the step.
#[inline(never)]
fn record_precompile_accesses(
    accesses: &mut Accesses<'_>,
    instruction: &ZiskInst,
    context: &InstContext,
) {
    let op = ZiskOp::try_from_code(instruction.op).expect("the ROM holds only ZisK opcodes");
    op.call_stats(context, accesses);
    if instruction.op == ZiskOp::KECCAK {
        ops_keccak(context, accesses);
    }
}

/// Records the memory accesses of a step that `frame` ran.
struct Accesses<'a> {
    memory: &'a mut PeakMemory,
    frame: Frame,
}

impl Accesses<'_> {
    fn access(&mut self, address: u64, len: u64) {
        self.memory.access(address, len, self.frame);
    }
}

impl OpStats for Accesses<'_> {
    fn mem_align_read(&mut self, addr: u64, count: usize) {
        self.access(addr, 8 * count as u64);
    }

    fn mem_align_write(&mut self, addr: u64, count: usize) {
        self.access(addr, 8 * count as u64);
    }

    fn set_variable_cost(&mut self, _cost: u64) {}
}

#[cfg(test)]
mod tests {
    use ere_prover_core::StackPointerWrite;
    use zisk_transpiler_riscv::Riscv2zisk;

    use crate::{cost::stack_pointer_write, prover::tests::basic_elf};

    #[test]
    fn stack_pointer_write_finds_the_upper_immediate_of_the_entry_code() {
        // `la sp, _init_stack_top` of the ziskos entry code expands to `auipc sp` and `addi`.
        // https://github.com/0xPolygonHermez/zisk/blob/v1.3.1-alpha/ziskos/entrypoint/src/lib.rs#L374
        let rom = Riscv2zisk::new(&basic_elf()).run().unwrap();
        assert!(rom.insts.values().any(
            |builder| stack_pointer_write(&builder.i) == StackPointerWrite::UpperImmediate
        ));
    }
}
