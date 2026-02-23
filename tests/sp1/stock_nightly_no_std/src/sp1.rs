use core::alloc::{GlobalAlloc, Layout};

#[no_mangle]
unsafe extern "C" fn __start() {
    crate::main();

    halt(0);
}

core::arch::global_asm!(include_str!("memcpy.s"));

// Alias the stack top to a static we can load easily.
static STACK_TOP: u64 = 0x78000000;

// 1. Init global pointer (GP). It's used to optimize jumps by linker. Linker can change jumping from PC(Program Counter) based to GP based.
// 2. Init stack pointer to the value STACK_TOP. It's stored in sp register.
// 3. Call __start function defined below.
// `__global_pointer$` is set by the linker. Its value depends on linker optimization. https://www.programmersought.com/article/77722901592/
core::arch::global_asm!(
    r#"
.section .text._start;
.globl _start;
_start:
    .option push;
    .option norelax;
    la gp, __global_pointer$;
    .option pop;
    la sp, {0}
    ld sp, 0(sp)
    call __start;
"#,
    sym STACK_TOP
);

/// According to https://github.com/succinctlabs/sp1/blob/v6.0.1/crates/zkvm/entrypoint/src/syscalls/sys.rs#L40.
#[panic_handler]
fn panic_impl(_panic_info: &core::panic::PanicInfo) -> ! {
    halt(1);
}

/// According to https://github.com/succinctlabs/sp1/blob/v6.0.1/crates/zkvm/entrypoint/src/syscalls/halt.rs#L58-L63.
fn halt(exit_code: u32) -> ! {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("t0") 0x00_00_00_00,
            in("a0") exit_code
        )
    };
    unreachable!()
}

/// A simple heap allocator.
///
/// Allocates memory from left to right, without any deallocation.
struct SimpleAlloc;

unsafe impl GlobalAlloc for SimpleAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        sys_alloc_aligned(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static HEAP: SimpleAlloc = SimpleAlloc;

// According to https://github.com/succinctlabs/sp1/blob/v6.0.1/crates/primitives/src/consts.rs#L4.
pub const MAXIMUM_MEMORY_SIZE: u64 = (1u64 << 48) - 1;

#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sys_alloc_aligned(bytes: usize, align: usize) -> *mut u8 {
    // Pointer to next heap address to use, or 0 if the heap has not yet been
    // initialized.
    static mut HEAP_POS: usize = 0;

    extern "C" {
        // https://lld.llvm.org/ELF/linker_script.html#sections-command
        static _end: u8;
    }

    // SAFETY: Single threaded, so nothing else can touch this while we're working.
    let mut heap_pos = unsafe { HEAP_POS };

    if heap_pos == 0 {
        heap_pos = unsafe { (&_end) as *const u8 as usize };
    }

    let offset = heap_pos & (align - 1);
    if offset != 0 {
        heap_pos += align - offset;
    }

    let ptr = heap_pos as *mut u8;
    let (heap_pos, overflowed) = heap_pos.overflowing_add(bytes);

    if overflowed || MAXIMUM_MEMORY_SIZE < heap_pos as u64 {
        panic!("Memory limit exceeded");
    }

    unsafe { HEAP_POS = heap_pos };

    ptr
}

// Assume single-threaded.
#[cfg(all(target_arch = "riscv32", target_feature = "a"))]
#[unsafe(no_mangle)]
fn _critical_section_1_0_acquire() -> u32 {
    return 0;
}

#[cfg(all(target_arch = "riscv32", target_feature = "a"))]
#[unsafe(no_mangle)]
fn _critical_section_1_0_release(_: u32) {}

// Assume single-threaded.
#[cfg(all(target_arch = "riscv64", target_feature = "a"))]
#[unsafe(no_mangle)]
fn _critical_section_1_0_acquire() -> u64 {
    return 0;
}

#[cfg(all(target_arch = "riscv64", target_feature = "a"))]
#[unsafe(no_mangle)]
fn _critical_section_1_0_release(_: u64) {}
