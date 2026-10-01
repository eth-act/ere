//! Gas of a run per guest call stack, and its peak memory use.

use std::{ops::Range, sync::Arc};

use ere_prover_core::{CallTree, CostProfile, PeakMemory, RasAction, StackPointerWrite, SymbolMap};
use sp1_core_executor::{
    CycleResult, ExecutionError, ExecutionReport, GasEstimatingVM, Instruction, Opcode, RiscvAirId,
    SupervisorMode, SyscallCode,
};
use sp1_primitives::consts::MAX_JIT_LOG_ADDR;

use crate::cost::COMPONENTS;

pub(crate) struct Profiler {
    call_tree: CallTree,
    memory: PeakMemory,
}

impl Profiler {
    pub(crate) fn new(
        symbol_map: Arc<SymbolMap>,
        loadable_segments: &[Range<u64>],
        pc_start: u64,
    ) -> Self {
        Self {
            call_tree: CallTree::new(symbol_map, pc_start, &COMPONENTS),
            memory: PeakMemory::new(0..1 << MAX_JIT_LOG_ADDR, loadable_segments),
        }
    }

    pub(crate) fn into_profile(self) -> CostProfile {
        self.call_tree.into_profile("cost", &self.memory)
    }

    /// Runs the chunk of `vm` to its end, as `GasEstimatingVM::<SupervisorMode>::execute` does. At
    /// each frame change it charges the growth of `running_cost`, the gas per component so far.
    /// https://github.com/succinctlabs/sp1/blob/v6.6.0/crates/core/executor/src/estimating.rs#L37-L51
    pub(crate) fn execute(
        &mut self,
        vm: &mut GasEstimatingVM<'_, SupervisorMode>,
        running_cost: impl Fn(&GasEstimatingVM<'_, SupervisorMode>) -> [u64; 3],
    ) -> Result<ExecutionReport, ExecutionError> {
        if !vm.core.is_done() {
            loop {
                // A fetch in supervisor mode changes no state.
                // https://github.com/succinctlabs/sp1/blob/v6.6.0/crates/core/executor/src/vm.rs#L595-L599
                let instruction = vm.core.fetch();
                let action = ras_action(&instruction);
                // The return address of a `jal` or `jalr` is the address after it.
                let return_address = vm.core.pc() + 4;
                self.record_accesses(&instruction, |register| {
                    vm.core.registers()[register as usize].value
                });
                let result = execute_instruction(vm)?;
                self.memory.stack_pointer(
                    vm.core.registers()[2].value,
                    stack_pointer_write(&instruction),
                );
                if result == CycleResult::Done(true) {
                    break;
                }
                let pc = vm.core.pc();
                if action.is_some() || !self.call_tree.contains(pc) {
                    self.call_tree.charge(&running_cost(vm));
                    self.call_tree.transfer(
                        action,
                        pc,
                        return_address,
                        vm.core.registers()[2].value,
                    );
                }
                if result != CycleResult::Done(false) {
                    break;
                }
            }
        }
        self.call_tree.charge(&running_cost(vm));
        Ok(vm.gas_calculator.generate_report())
    }

    /// Records the memory that `instruction` accesses, from the registers before it runs.
    fn record_accesses(&mut self, instruction: &Instruction, register: impl Fn(u64) -> u64) {
        let width = match instruction.opcode {
            Opcode::LB | Opcode::LBU | Opcode::SB => 1,
            Opcode::LH | Opcode::LHU | Opcode::SH => 2,
            Opcode::LW | Opcode::LWU | Opcode::SW => 4,
            Opcode::LD | Opcode::SD => 8,
            Opcode::ECALL => return self.record_syscall_accesses(register),
            _ => return,
        };
        // A load or store keeps its base register in `op_b`.
        let address = register(instruction.op_b).wrapping_add(instruction.op_c);
        self.memory.access(address, width, self.call_tree.frame());
    }

    /// Records the memory that the syscall in `t0` accesses, from the registers before it runs.
    fn record_syscall_accesses(&mut self, register: impl Fn(u64) -> u64) {
        let code = SyscallCode::from_u32(register(5) as u32);
        let frame = self.call_tree.frame();
        let mut access = |address, len| self.memory.access(address, len, frame);
        match code {
            // The doublewords that `hint_read` writes.
            // https://github.com/succinctlabs/sp1/blob/v6.6.0/crates/core/executor/src/minimal/hint.rs#L19-L38
            SyscallCode::HINT_READ => access(register(10), 8 * (register(11) / 8 + 1)),
            SyscallCode::WRITE => access(register(11), register(12)),
            SyscallCode::INSERT_PROFILER_SYMBOLS | SyscallCode::DELETE_PROFILER_SYMBOLS => {
                access(register(10), register(11))
            }
            _ => {}
        }
        for (argument, doublewords) in syscall_doublewords(code) {
            access(register(*argument), 8 * doublewords);
        }
    }
}

/// Doublewords that a syscall reads or writes at the address in each argument register. They sum to
/// `SyscallCode::touched_addresses`.
fn syscall_doublewords(code: SyscallCode) -> &'static [(u64, u64)] {
    match code {
        SyscallCode::SHA_EXTEND => &[(10, 64)],
        SyscallCode::SHA_COMPRESS => &[(10, 64), (11, 8)],
        SyscallCode::KECCAK_PERMUTE => &[(10, 25)],
        SyscallCode::POSEIDON2 => &[(10, 8)],
        SyscallCode::ED_ADD
        | SyscallCode::SECP256K1_ADD
        | SyscallCode::SECP256R1_ADD
        | SyscallCode::BN254_ADD
        | SyscallCode::BN254_FP2_ADD
        | SyscallCode::BN254_FP2_SUB
        | SyscallCode::BN254_FP2_MUL => &[(10, 8), (11, 8)],
        SyscallCode::BLS12381_ADD
        | SyscallCode::BLS12381_FP2_ADD
        | SyscallCode::BLS12381_FP2_SUB
        | SyscallCode::BLS12381_FP2_MUL => &[(10, 12), (11, 12)],
        SyscallCode::ED_DECOMPRESS
        | SyscallCode::SECP256K1_DECOMPRESS
        | SyscallCode::SECP256R1_DECOMPRESS
        | SyscallCode::SECP256K1_DOUBLE
        | SyscallCode::SECP256R1_DOUBLE
        | SyscallCode::BN254_DOUBLE => &[(10, 8)],
        SyscallCode::BLS12381_DECOMPRESS | SyscallCode::BLS12381_DOUBLE => &[(10, 12)],
        SyscallCode::BN254_FP_ADD | SyscallCode::BN254_FP_SUB | SyscallCode::BN254_FP_MUL => {
            &[(10, 4), (11, 4)]
        }
        SyscallCode::BLS12381_FP_ADD
        | SyscallCode::BLS12381_FP_SUB
        | SyscallCode::BLS12381_FP_MUL => &[(10, 6), (11, 6)],
        SyscallCode::UINT256_MUL => &[(10, 4), (11, 8)],
        SyscallCode::UINT256_ADD_CARRY | SyscallCode::UINT256_MUL_CARRY => {
            &[(10, 4), (11, 4), (12, 4), (13, 4), (14, 4)]
        }
        SyscallCode::U256XU2048_MUL => &[(10, 4), (11, 32), (12, 32), (13, 4)],
        SyscallCode::HALT
        | SyscallCode::WRITE
        | SyscallCode::ENTER_UNCONSTRAINED
        | SyscallCode::EXIT_UNCONSTRAINED
        | SyscallCode::COMMIT
        | SyscallCode::COMMIT_DEFERRED_PROOFS
        | SyscallCode::VERIFY_SP1_PROOF
        | SyscallCode::HINT_LEN
        | SyscallCode::HINT_READ
        | SyscallCode::MPROTECT
        | SyscallCode::SIG_RETURN
        | SyscallCode::HINT_MPROTECT_FLUSH
        | SyscallCode::DUMP_ELF
        | SyscallCode::INSERT_PROFILER_SYMBOLS
        | SyscallCode::DELETE_PROFILER_SYMBOLS => &[],
    }
}

/// Return-address stack action of `instruction`.
fn ras_action(instruction: &Instruction) -> Option<RasAction> {
    match instruction.opcode {
        Opcode::JAL => {
            let rd = instruction.j_type().0 as u8;
            RasAction::from_jump(rd, rd)
        }
        Opcode::JALR => {
            let (rd, rs1, _) = instruction.i_type();
            RasAction::from_jump(rd as u8, rs1 as u8)
        }
        _ => None,
    }
}

/// How `instruction` writes `x2`.
fn stack_pointer_write(instruction: &Instruction) -> StackPointerWrite {
    if instruction.op_a != 2 {
        StackPointerWrite::Other
    } else if instruction.is_memory_load_instruction() {
        StackPointerWrite::Load
    } else if matches!(instruction.opcode, Opcode::LUI | Opcode::AUIPC) {
        StackPointerWrite::UpperImmediate
    } else {
        StackPointerWrite::Other
    }
}

/// Copy of the private `GasEstimatingVM::<SupervisorMode>::execute_instruction` of
/// sp1-core-executor v6.6.0, with `self` as the argument `vm`.
/// https://github.com/succinctlabs/sp1/blob/v6.6.0/crates/core/executor/src/estimating.rs#L53-L115
#[rustfmt::skip]
fn execute_instruction(
    vm: &mut GasEstimatingVM<'_, SupervisorMode>,
) -> Result<CycleResult, ExecutionError> {
    let instruction = vm.core.fetch();

    match &instruction.opcode {
        Opcode::ADD
        | Opcode::ADDI
        | Opcode::SUB
        | Opcode::XOR
        | Opcode::OR
        | Opcode::AND
        | Opcode::SLL
        | Opcode::SLLW
        | Opcode::SRL
        | Opcode::SRA
        | Opcode::SRLW
        | Opcode::SRAW
        | Opcode::SLT
        | Opcode::SLTU
        | Opcode::MUL
        | Opcode::MULHU
        | Opcode::MULHSU
        | Opcode::MULH
        | Opcode::MULW
        | Opcode::DIVU
        | Opcode::REMU
        | Opcode::DIV
        | Opcode::REM
        | Opcode::DIVW
        | Opcode::ADDW
        | Opcode::SUBW
        | Opcode::DIVUW
        | Opcode::REMUW
        | Opcode::REMW => {
            vm.execute_alu(&instruction);
        }
        Opcode::LB
        | Opcode::LBU
        | Opcode::LH
        | Opcode::LHU
        | Opcode::LW
        | Opcode::LWU
        | Opcode::LD => vm.execute_load(&instruction)?,
        Opcode::SB | Opcode::SH | Opcode::SW | Opcode::SD => {
            vm.execute_store(&instruction)?;
        }
        Opcode::JAL | Opcode::JALR => {
            vm.execute_jump(&instruction);
        }
        Opcode::BEQ | Opcode::BNE | Opcode::BLT | Opcode::BGE | Opcode::BLTU | Opcode::BGEU => {
            vm.execute_branch(&instruction);
        }
        Opcode::LUI | Opcode::AUIPC => {
            vm.execute_utype(&instruction);
        }
        Opcode::ECALL => vm.execute_ecall(&instruction)?,
        Opcode::EBREAK | Opcode::UNIMP => {
            unreachable!("Invalid opcode for `execute_instruction`: {:?}", instruction.opcode)
        }
    }

    Ok(vm.core.advance())
}

/// Copy of the crate-private `riscv_air_id_from_opcode` of sp1-core-executor v6.6.0.
/// https://github.com/succinctlabs/sp1/blob/v6.6.0/crates/core/executor/src/vm/shapes.rs#L350-L393
#[rustfmt::skip]
#[inline]
pub fn riscv_air_id_from_opcode(opcode: Opcode) -> RiscvAirId {
    match opcode {
        Opcode::ADD => RiscvAirId::Add,
        Opcode::ADDI => RiscvAirId::Addi,
        Opcode::ADDW => RiscvAirId::Addw,
        Opcode::SUB => RiscvAirId::Sub,
        Opcode::SUBW => RiscvAirId::Subw,
        Opcode::XOR | Opcode::OR | Opcode::AND => RiscvAirId::Bitwise,
        Opcode::SLT | Opcode::SLTU => RiscvAirId::Lt,
        Opcode::MUL | Opcode::MULH | Opcode::MULHU | Opcode::MULHSU | Opcode::MULW => {
            RiscvAirId::Mul
        }
        Opcode::DIV
        | Opcode::DIVU
        | Opcode::REM
        | Opcode::REMU
        | Opcode::DIVW
        | Opcode::DIVUW
        | Opcode::REMW
        | Opcode::REMUW => RiscvAirId::DivRem,
        Opcode::SLL | Opcode::SLLW => RiscvAirId::ShiftLeft,
        Opcode::SRLW | Opcode::SRAW | Opcode::SRL | Opcode::SRA => RiscvAirId::ShiftRight,
        Opcode::LB | Opcode::LBU => RiscvAirId::LoadByte,
        Opcode::LH | Opcode::LHU => RiscvAirId::LoadHalf,
        Opcode::LW | Opcode::LWU => RiscvAirId::LoadWord,
        Opcode::LD => RiscvAirId::LoadDouble,
        Opcode::SB => RiscvAirId::StoreByte,
        Opcode::SH => RiscvAirId::StoreHalf,
        Opcode::SW => RiscvAirId::StoreWord,
        Opcode::SD => RiscvAirId::StoreDouble,
        Opcode::BEQ | Opcode::BNE | Opcode::BLT | Opcode::BGE | Opcode::BLTU | Opcode::BGEU => {
            RiscvAirId::Branch
        }
        Opcode::AUIPC | Opcode::LUI => RiscvAirId::UType,
        Opcode::JAL => RiscvAirId::Jal,
        Opcode::JALR => RiscvAirId::Jalr,
        Opcode::ECALL => RiscvAirId::SyscallInstrs,
        _ => {
            eprintln!("Unknown opcode: {opcode:?}");
            unreachable!()
        }
    }
}

#[cfg(test)]
mod tests {
    use ere_prover_core::StackPointerWrite;
    use sp1_core_executor::{Program, SyscallCode};
    use strum::IntoEnumIterator;

    use crate::{
        cost::profile::{stack_pointer_write, syscall_doublewords},
        prover::tests::basic_elf,
    };

    #[test]
    fn stack_pointer_write_finds_the_upper_immediate_of_the_entry_code() {
        // `la sp, _STACK_TOP` of the sp1-zkvm entry code expands to `auipc sp` and `addi`.
        // https://github.com/succinctlabs/sp1/blob/v6.6.0/crates/zkvm/entrypoint/src/lib.rs#L269
        let program = Program::from(&basic_elf()).unwrap();
        assert!(program.instructions.iter().any(
            |instruction| stack_pointer_write(instruction) == StackPointerWrite::UpperImmediate
        ));
    }

    #[test]
    fn syscall_doublewords_match_touched_addresses() {
        for code in SyscallCode::iter() {
            let doublewords: u64 = syscall_doublewords(code)
                .iter()
                .map(|(_, doublewords)| doublewords)
                .sum();
            assert_eq!(doublewords as usize, code.touched_addresses(), "{code:?}");
        }
    }
}
