use core::alloc::{GlobalAlloc, Layout};
// Import user `main` function
use crate::main;

// Pico starts with no stack pointer set, so set `gp` and `sp` before calling `__start`.
// According to https://github.com/brevis-network/pico/blob/613d6aeb64350d73faf24c17e2e3e3cee5b763bc/sdk/sdk/src/lib.rs#L96-L111
core::arch::global_asm!(
    r#"
.section .text._start;
.globl _start;
_start:
    .option push;
    .option norelax;
    la gp, __global_pointer$;
    .option pop;
    li sp, 0x78000000;
    call __start;
"#
);

// 1. Call `main` user function
// 2. Commit the public values digest and halt.
#[unsafe(no_mangle)]
fn __start(_argc: isize, _argv: *const *const u8) -> isize {
    main();

    terminate();

    unreachable!()
}

// SHA-256 of the empty public values stream, as 8 little-endian words.
const EMPTY_PV_DIGEST: [u32; 8] = [
    0x42c4b0e3, 0x141cfc98, 0xc8f4fb9a, 0x24b96f99, 0xe441ae27, 0x4c939b64, 0x1b9995a4, 0x55b85278,
];

// According to https://github.com/brevis-network/pico/blob/613d6aeb64350d73faf24c17e2e3e3cee5b763bc/sdk/sdk/src/riscv_ecalls/halt.rs
#[inline(always)]
fn terminate() {
    unsafe {
        for (i, word) in EMPTY_PV_DIGEST.iter().enumerate() {
            core::arch::asm!("ecall", in("t0") 0x10usize, in("a0") i, in("a1") *word); // Commit
        }
        for i in 0..8usize {
            core::arch::asm!("ecall", in("t0") 0x1Ausize, in("a0") i, in("a1") 0usize); // Commit deferred proofs
        }
        core::arch::asm!("ecall", in("t0") 0usize, in("a0") 0usize); // Halt with exit code 0
    }
}

#[panic_handler]
fn panic_impl(_panic_info: &core::panic::PanicInfo) -> ! {
    // Halt with a non-zero exit code.
    unsafe { core::arch::asm!("ecall", in("t0") 0usize, in("a0") 1usize) }

    unreachable!()
}

/// A simple heap allocator.
///
/// Allocates memory from left to right, without any deallocation.
struct SimpleAlloc;

unsafe impl GlobalAlloc for SimpleAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { sys_alloc_aligned(layout.size(), layout.align()) }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static HEAP: SimpleAlloc = SimpleAlloc;

// According to https://github.com/brevis-network/pico/blob/613d6aeb64350d73faf24c17e2e3e3cee5b763bc/sdk/sdk/src/riscv_ecalls/memory.rs
pub const MAX_MEMORY: usize = 0x7800_0000;
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
