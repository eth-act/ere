use core::alloc::{GlobalAlloc, Layout};
// Import user `main` function
use crate::main;

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
    lw sp, 0(sp)
    call __start;
"#,
    sym STACK_TOP
);

static STACK_TOP: u32 = 0x0020_0400;

// 1. Call `main` user function
// 2. Call system halt environment function. It's defined by sp1 vm.
#[unsafe(no_mangle)]
fn __start(_argc: isize, _argv: *const *const u8) -> isize {
    main();

    terminate();

    unreachable!()
}

// According to https://github.com/openvm-org/openvm/blob/v1.4.2-rc.1/crates/toolchain/openvm/src/process.rs
#[inline(always)]
fn terminate() {
    unsafe {
        core::arch::asm!(
            ".insn 4, 0x000b"
        )
    }
}

// According to https://github.com/openvm-org/openvm/blob/v1.4.2-rc.1/crates/toolchain/openvm/src/process.rs
#[panic_handler]
fn panic_impl(_panic_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        core::arch::asm!(
            ".insn 4, 0x10000b"
        )
    }

    unreachable!()
}

/// A simple heap allocator.
///
/// Allocates memory from left to right, without any deallocation.
struct SimpleAlloc;

unsafe impl GlobalAlloc for SimpleAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            sys_alloc_aligned(layout.size(), layout.align())
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static HEAP: SimpleAlloc = SimpleAlloc;

pub const MAX_MEMORY: usize = 0x78000000;
static mut HEAP_POS: usize = 0;
#[allow(clippy::missing_safety_doc)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sys_alloc_aligned(bytes: usize, align: usize) -> *mut u8 {
    unsafe extern "C" {
        // https://lld.llvm.org/ELF/linker_script.html#sections-command
        // `_end` is the last global variable defined by the linker. Its address is the beginning of heap data.
        unsafe static _end: u8;
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

    if overflowed || MAX_MEMORY < heap_pos {
        panic!("Memory limit exceeded (0x78000000)");
    }

    unsafe { HEAP_POS = heap_pos };
    ptr
}

// Assume single-threaded.
#[cfg(all(target_arch = "riscv32", target_feature = "a"))]
#[unsafe(no_mangle)]
fn _critical_section_1_0_acquire() -> u32
{
    return 0;
}

#[cfg(all(target_arch = "riscv32", target_feature = "a"))]
#[unsafe(no_mangle)]
fn _critical_section_1_0_release(_: u32)
{}

// Assume single-threaded.
#[cfg(all(target_arch = "riscv64", target_feature = "a"))]
#[unsafe(no_mangle)]
fn _critical_section_1_0_acquire() -> u64
{
    return 0;
}

#[cfg(all(target_arch = "riscv64", target_feature = "a"))]
#[unsafe(no_mangle)]
fn _critical_section_1_0_release(_: u64)
{}
