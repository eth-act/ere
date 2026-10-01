use std::{
    any::Any,
    cell::Cell,
    iter,
    ops::Range,
    panic::{self, AssertUnwindSafe},
    ptr::NonNull,
    sync::Arc,
};

use ere_prover_core::{CallTree, CostProfile, PeakMemory, RasAction, StackPointerWrite, SymbolMap};
use openvm_circuit::arch::{
    ExecutorInventory, ExecutorInventoryError, VmExecutionConfig,
    execution_mode::{MeteredCtx, Segment, metered::memory_ctx::PageTouch},
    instructions::{
        LocalOpcode, exe::VmExe, metering::PAGE_MASK_LEAF_BITS, riscv::RV64_REGISTER_NUM_LIMBS,
    },
    interpreter::get_pc_index,
    rvr::metered::{MeteringState, metered_memory_buffer_flush},
};
use openvm_riscv_transpiler::{
    Rv64AuipcOpcode, Rv64JalLuiOpcode, Rv64JalrOpcode, Rv64LoadStoreOpcode,
};
use openvm_sdk::F;
use openvm_sdk_config::SdkVmConfig;
use openvm_stark_sdk::openvm_stark_backend::p3_field::PrimeField32;
use openvm_transpiler::openvm_platform::memory::MEM_SIZE;
use rvr_openvm_ir::LiftedInstr;
use rvr_openvm_lift::{
    ExtensionError, MAIN_MEMORY_PAGE_BYTES, RvrExtension, RvrExtensions, RvrInstruction,
    RvrRuntimeExtension,
};

use crate::cost::{add_cost, rows_cost, run_cost};

/// Events of the hooks in `ere_profile.h`.
const JUMP: u32 = 0;
const STACK_POINTER: u32 = 1;
const EXIT: u32 = 2;
const CHECK_START: u32 = 3;
const CHECK_END: u32 = 4;

/// Root frame of the rows that every segment starts with.
const SEGMENT_BASE_FRAME: &str = "[segment base]";

thread_local! {
    /// Profiler of the run on this thread.
    static PROFILER: Cell<Option<NonNull<Profiler>>> = const { Cell::new(None) };
}

/// Return-address stack action of the instruction at each PC index, and how the instruction
/// writes `x2`.
fn decode(app_exe: &VmExe<F>) -> (Vec<Option<RasAction>>, Vec<StackPointerWrite>) {
    let (jal, jalr) = (
        Rv64JalLuiOpcode::JAL.global_opcode(),
        Rv64JalrOpcode::JALR.global_opcode(),
    );
    let loads = [
        Rv64LoadStoreOpcode::LOADD,
        Rv64LoadStoreOpcode::LOADWU,
        Rv64LoadStoreOpcode::LOADHU,
        Rv64LoadStoreOpcode::LOADBU,
        Rv64LoadStoreOpcode::LOADW,
        Rv64LoadStoreOpcode::LOADH,
        Rv64LoadStoreOpcode::LOADB,
    ]
    .map(|opcode| opcode.global_opcode());
    let upper_immediates = [
        Rv64JalLuiOpcode::LUI.global_opcode(),
        Rv64AuipcOpcode::AUIPC.global_opcode(),
    ];
    let program = &app_exe.program;
    let register = |operand: F| (operand.as_canonical_u32() / RV64_REGISTER_NUM_LIMBS as u32) as u8;
    iter::repeat_n(&None, get_pc_index(program.pc_base))
        .chain(&program.instructions_and_debug_infos)
        .map(|instruction| {
            let Some((instruction, _)) = instruction.as_ref() else {
                return (None, StackPointerWrite::Other);
            };
            let rd = register(instruction.a);
            let action = if instruction.opcode == jal {
                RasAction::from_jump(rd, rd)
            } else if instruction.opcode == jalr {
                RasAction::from_jump(rd, register(instruction.b))
            } else {
                None
            };
            let write = if rd != 2 {
                StackPointerWrite::Other
            } else if loads.contains(&instruction.opcode) {
                StackPointerWrite::Load
            } else if upper_immediates.contains(&instruction.opcode) {
                StackPointerWrite::UpperImmediate
            } else {
                StackPointerWrite::Other
            };
            (action, write)
        })
        .unzip()
}

/// `SdkVmConfig` whose `rvr` libraries report the profile hooks to the [`Profiler`] of the thread.
pub(crate) struct ProfileConfig(pub(crate) SdkVmConfig);

impl VmExecutionConfig<F> for ProfileConfig {
    type Executor = <SdkVmConfig as VmExecutionConfig<F>>::Executor;

    fn create_executors(
        &self,
    ) -> Result<ExecutorInventory<Self::Executor>, ExecutorInventoryError> {
        VmExecutionConfig::<F>::create_executors(&self.0)
    }

    fn create_rvr_extensions(&self, air_idx: Option<&[usize]>) -> RvrExtensions {
        let mut extensions = VmExecutionConfig::<F>::create_rvr_extensions(&self.0, air_idx);
        extensions.register_lifter(ProfileHooks);
        extensions.register_runtime_hook(ProfileHooks);
        extensions
    }
}

/// Defines the profile hooks of the generated code, so that they call [`on_event`].
struct ProfileHooks;

impl RvrExtension for ProfileHooks {
    fn try_lift(&self, _: &RvrInstruction, _: u64) -> Option<LiftedInstr> {
        None
    }

    fn c_headers(&self) -> Vec<(&'static str, &'static str)> {
        vec![("ere_profile.h", include_str!("../../c/ere_profile.h"))]
    }

    fn c_sources(&self) -> Vec<(&'static str, &'static str)> {
        vec![("ere_profile.c", include_str!("../../c/ere_profile.c"))]
    }

    fn max_main_memory_pages_per_instruction(&self) -> usize {
        0
    }
}

impl RvrRuntimeExtension for ProfileHooks {
    unsafe fn register_host_callbacks(
        &self,
        lib: &libloading::Library,
    ) -> Result<(), ExtensionError> {
        type Register =
            unsafe extern "C" fn(unsafe extern "C" fn(*mut MeteringState, *mut u32, u32, u64, u64));
        let register = unsafe { lib.get::<Register>(b"register_ere_profile") }
            .map_err(|err| ExtensionError::HostCallbackRegistration(err.to_string()))?;
        unsafe { register(on_event) };
        Ok(())
    }
}

/// Receives the profile hooks of `ere_profile.h` for the [`Profiler`] of the thread.
unsafe extern "C" fn on_event(
    metering: *mut MeteringState,
    _trace_heights: *mut u32,
    event: u32,
    pc: u64,
    value: u64,
) {
    // SAFETY: `Profiler::run` sets the profiler of the thread for the whole run.
    let profiler = unsafe { PROFILER.get().unwrap().as_mut() };
    // A panic must not unwind into the generated code, so it resumes after the run.
    if profiler.panic.is_none() {
        let result = panic::catch_unwind(AssertUnwindSafe(|| unsafe {
            profiler.on_event(metering, event, pc, value)
        }));
        profiler.panic = result.err();
    }
}

/// Splits the rows of a metered `rvr` run over the guest call frames, and measures its memory use.
pub(crate) struct Profiler {
    /// Return-address stack action of the instruction at each PC index.
    ras_actions: Vec<Option<RasAction>>,
    /// How the instruction at each PC index writes `x2`.
    stack_pointer_writes: Vec<StackPointerWrite>,
    /// Width of each AIR per component, 0 for an AIR of another component.
    widths: [Vec<u32>; 3],
    /// Cost of the closed segments.
    closed_cost: [u64; 3],
    /// Number of the closed segments.
    closed_segments: usize,
    /// PC of the last write to `x2`.
    stack_pointer_pc: u64,
    /// Value of the last write to `x2`.
    stack_pointer: u64,
    call_tree: CallTree,
    memory: PeakMemory,
    /// Touched leaves of each main memory page.
    leaves: Vec<u64>,
    /// Entries of the main memory touches since the last check that the profiler read.
    touches_read: usize,
    panic: Option<Box<dyn Any + Send>>,
}

impl Profiler {
    pub(crate) fn new(
        symbol_map: Arc<SymbolMap>,
        loadable_segments: &[Range<u64>],
        app_exe: &VmExe<F>,
        components: &[&str],
        widths: [Vec<u32>; 3],
        ctx: &MeteredCtx,
    ) -> Self {
        let (ras_actions, stack_pointer_writes) = decode(app_exe);
        let mut profiler = Self {
            ras_actions,
            stack_pointer_writes,
            widths,
            closed_cost: [0; 3],
            closed_segments: 0,
            stack_pointer_pc: u64::MAX,
            stack_pointer: 0,
            call_tree: CallTree::new(symbol_map, u64::from(app_exe.pc_start), components),
            memory: PeakMemory::new(0..MEM_SIZE as u64, loadable_segments),
            leaves: vec![0; MEM_SIZE / MAIN_MEMORY_PAGE_BYTES],
            touches_read: 0,
            panic: None,
        };
        // The rows that the first segment starts with.
        let running = profiler.running_cost(ctx);
        profiler.call_tree.charge_root(SEGMENT_BASE_FRAME, &running);
        profiler
    }

    /// Calls `run`, which runs a profile library on this thread, with the hooks going to `self`.
    pub(crate) fn run<T>(&mut self, run: impl FnOnce() -> T) -> T {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                PROFILER.set(None);
            }
        }
        PROFILER.set(Some(NonNull::from(&mut *self)));
        let result = {
            let _reset = Reset;
            run()
        };
        if let Some(payload) = self.panic.take() {
            panic::resume_unwind(payload);
        }
        result
    }

    /// The profile of a run that ended with `segments`.
    pub(crate) fn into_profile(mut self, segments: &[Segment]) -> CostProfile {
        self.call_tree.charge(&run_cost(&self.widths, segments));
        self.keep_stack_pointer();
        self.call_tree.into_profile("cells", &self.memory)
    }

    unsafe fn on_event(&mut self, metering: *mut MeteringState, event: u32, pc: u64, value: u64) {
        // The stack pointer hook runs before the block flushes its page locals, so it must not
        // touch the metering state.
        if event == STACK_POINTER {
            // An instruction between the last write and this one keeps the last value.
            if self.stack_pointer_pc.wrapping_add(4) != pc {
                self.keep_stack_pointer();
            }
            let write = self.stack_pointer_writes[get_pc_index(pc as u32)];
            self.memory.stack_pointer(value, write);
            (self.stack_pointer_pc, self.stack_pointer) = (pc, value);
            return;
        }
        let action = self.ras_actions[get_pc_index(pc as u32)];
        // Inside the function that runs, only a call or a return can change the frame.
        if event == JUMP
            && action.is_none()
            && self.call_tree.contains(pc)
            && self.call_tree.contains(value)
        {
            return;
        }
        // SAFETY: a hook runs between instructions, where nothing borrows the metering state.
        let segmentation = unsafe {
            metered_memory_buffer_flush(metering);
            &mut *(*metering).seg_state
        };
        // As a check does, the memory rows of the main memory accesses so far enter the heights.
        // https://github.com/openvm-org/openvm/blob/v2.1.0-preview/crates/vm/src/arch/rvr/metered.rs#L270-L313
        let ctx = &mut segmentation.ctx;
        ctx.memory_ctx.apply_height_updates(&mut ctx.trace_heights);
        let cut = ctx.segmentation_ctx.segments.len() > self.closed_segments;
        let running = self.running_cost(ctx);
        match event {
            JUMP => {
                self.call_tree.charge(&running);
                self.read_touches(&segmentation.drained_mem_page_touches);
                // The return address of a `jal` or `jalr` is the address after it.
                self.call_tree
                    .transfer(action, value, pc + 4, self.stack_pointer);
            }
            CHECK_END => {
                // The check clears the touches.
                self.touches_read = 0;
                if cut {
                    self.call_tree.charge_root(SEGMENT_BASE_FRAME, &running);
                }
            }
            EXIT | CHECK_START => {
                self.call_tree.charge(&running);
                self.read_touches(&segmentation.drained_mem_page_touches);
            }
            _ => unreachable!("`ere_profile.h` sends no event {event}"),
        }
    }

    /// Cost per component of the rows so far.
    fn running_cost(&mut self, ctx: &MeteredCtx) -> [u64; 3] {
        let segments = &ctx.segmentation_ctx.segments;
        let closed = run_cost(&self.widths, &segments[self.closed_segments..]);
        self.closed_cost = add_cost(self.closed_cost, closed);
        self.closed_segments = segments.len();
        add_cost(
            self.closed_cost,
            rows_cost(&self.widths, &ctx.trace_heights),
        )
    }

    /// Records the new leaves of the `touches` since the last check, which the frame that runs
    /// touched.
    fn read_touches(&mut self, touches: &[PageTouch]) {
        let leaf_bytes = (MAIN_MEMORY_PAGE_BYTES >> PAGE_MASK_LEAF_BITS) as u64;
        let frame = self.call_tree.frame();
        // A touch of the page of the last entry joins that entry, so the profiler reads it again.
        // https://github.com/openvm-org/openvm/blob/v2.1.0-preview/crates/vm/src/arch/rvr/metered.rs#L219-L235
        for touch in &touches[self.touches_read.saturating_sub(1)..] {
            let page_leaves = &mut self.leaves[touch.page_id as usize];
            let mut mask = touch.leaf_mask & !*page_leaves;
            *page_leaves |= touch.leaf_mask;
            // One access per run of new leaves.
            while mask != 0 {
                let start = mask.trailing_zeros();
                let leaves = (mask >> start).trailing_ones();
                let address = u64::from(touch.page_id) * MAIN_MEMORY_PAGE_BYTES as u64
                    + u64::from(start) * leaf_bytes;
                self.memory
                    .access(address, u64::from(leaves) * leaf_bytes, frame);
                mask &= mask.wrapping_add(1 << start);
            }
        }
        self.touches_read = touches.len();
    }

    /// Records that a step kept the last value of `x2`.
    fn keep_stack_pointer(&mut self) {
        self.memory
            .stack_pointer(self.stack_pointer, StackPointerWrite::Other);
    }
}
