use std::borrow::BorrowMut;

use openvm_circuit::{
    arch::instructions::{
        LocalOpcode, SystemOpcode, VM_DIGEST_WIDTH,
        exe::VmExe,
        instruction::Instruction,
        program::{DEFAULT_PC_STEP, Program},
    },
    system::program::{ProgramExecutionCols, trace::compute_exe_commit_from_mem_config},
};
use openvm_sdk::{DeferralSetup, F, SC, keygen::AppProvingKey, prover::AggProver};
use openvm_sdk_config::SdkVmConfig;
use openvm_stark_sdk::openvm_stark_backend::{
    StarkEngine,
    p3_field::Field,
    p3_matrix::dense::RowMajorMatrix,
    p3_maybe_rayon::prelude::*,
    prover::{ColMajorMatrix, DeviceDataTransporter, TraceCommitter},
};
use openvm_verify_stark_host::vk::VerificationBaseline;

/// Returns the baseline of a program with commit `app_exe_commit`, as
/// `StarkProver::generate_baseline` does without a prover.
pub(crate) fn baseline(
    app_pk: &AppProvingKey<SdkVmConfig>,
    agg_prover: &AggProver,
    app_exe_commit: [F; VM_DIGEST_WIDTH],
) -> VerificationBaseline {
    let system_config = app_pk.app_vm_pk.vm_config.as_ref();
    VerificationBaseline {
        app_exe_commit,
        memory_dimensions: system_config.memory_config.memory_dimensions(),
        num_user_pvs: system_config.num_public_values,
        app_vk_commit: agg_prover.leaf_prover.get_vk_commit(false),
        leaf_vk_commit: agg_prover.internal_for_leaf_prover.get_vk_commit(false),
        internal_for_leaf_vk_commit: agg_prover.internal_recursive_prover.get_vk_commit(false),
        internal_recursive_vk_commit: agg_prover.internal_recursive_prover.get_vk_commit(true),
        expected_def_hook_commit: DeferralSetup::Disabled.hook_commit(),
    }
}

/// Returns the commit of `app_exe`, as `VirtualMachine::commit_program_on_device` and
/// `AppProver::app_exe_commit` do without the proving key.
pub(crate) fn app_exe_commit<E: StarkEngine<SC = SC>>(
    engine: &E,
    app_pk: &AppProvingKey<SdkVmConfig>,
    app_exe: &VmExe<F>,
) -> [F; VM_DIGEST_WIDTH] {
    let trace = ColMajorMatrix::from_row_major(&generate_cached_trace(&app_exe.program));
    let trace = engine.device().transport_matrix_to_device(&trace);
    let (program_commit, _) = engine
        .device()
        .commit(&[&trace])
        .expect("the program trace commits");
    compute_exe_commit_from_mem_config(
        &program_commit,
        app_exe,
        &app_pk.app_vm_pk.vm_config.as_ref().memory_config,
    )
}

// Copied from https://github.com/openvm-org/openvm/blob/v2.1.0-preview/crates/vm/src/system/program/trace.rs,
// with `collect::<Vec<_>>` for `collect_vec` and the value of `EXIT_CODE_FAIL`.

fn generate_cached_trace<F: Field>(program: &Program<F>) -> RowMajorMatrix<F> {
    let width = ProgramExecutionCols::<F>::width();
    let mut instructions = program
        .enumerate_by_pc()
        .into_iter()
        .map(|(pc, instruction, _)| (pc, instruction))
        .collect::<Vec<_>>();

    let padding = padding_instruction();
    while !instructions.len().is_power_of_two() {
        instructions.push((
            program.pc_base + instructions.len() as u32 * DEFAULT_PC_STEP,
            padding.clone(),
        ));
    }

    let mut rows = F::zero_vec(instructions.len() * width);
    rows.par_chunks_mut(width)
        .zip(instructions)
        .for_each(|(row, (pc, instruction))| {
            let row: &mut ProgramExecutionCols<F> = row.borrow_mut();
            *row = ProgramExecutionCols {
                pc: F::from_u32(pc),
                opcode: instruction.opcode.to_field(),
                a: instruction.a,
                b: instruction.b,
                c: instruction.c,
                d: instruction.d,
                e: instruction.e,
                f: instruction.f,
                g: instruction.g,
            };
        });

    RowMajorMatrix::new(rows, width)
}

fn padding_instruction<F: Field>() -> Instruction<F> {
    Instruction::from_usize(SystemOpcode::TERMINATE.global_opcode(), [0, 0, 1])
}

#[cfg(test)]
mod tests {
    use openvm_stark_sdk::{
        config::baby_bear_poseidon2::BabyBearPoseidon2CpuEngine, openvm_stark_backend::StarkEngine,
    };

    use crate::{
        baseline::{app_exe_commit, baseline},
        prover::{cpu_sdk, tests::basic_elf, transpile},
    };

    #[test]
    fn test_baseline_matches_sdk() {
        let app_exe = transpile(&basic_elf()).unwrap();
        let sdk = cpu_sdk(None, None).unwrap();
        let engine = <BabyBearPoseidon2CpuEngine>::new(sdk.app_pk().app_vm_pk.get_params());
        let app_exe_commit = app_exe_commit(&engine, sdk.app_pk(), &app_exe);
        let baseline = baseline(sdk.app_pk(), &sdk.agg_prover(), app_exe_commit);
        let expected = sdk.prover(app_exe).unwrap().generate_baseline();
        assert_eq!(format!("{baseline:?}"), format!("{expected:?}"));
    }
}
