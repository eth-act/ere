use core::alloc::{GlobalAlloc, Layout};
// Import user `main` function
use crate::main;

// Call __start function defined below. The executor already sets the stack pointer.
// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/executor/src/vm/registers.rs#L3-L16
core::arch::global_asm!(
    r#"
.section .text._start;
.globl _start;
_start:
    call __start;
"#
);

// 1. Call `main` user function
// 2. Call system halt environment function.
#[unsafe(no_mangle)]
fn __start(_argc: isize, _argv: *const *const u8) -> isize {
    main();

    terminate();

    unreachable!()
}

// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/syscalls/src/syscalls.rs#L141-L153
#[inline(always)]
fn terminate() {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") 0usize, // Exit code 0, the HALT AIR requires it
            in("a7") 93usize, // Halt
        )
    }
}

// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/syscalls/src/syscalls.rs#L21-L26
#[panic_handler]
fn panic_impl(_panic_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a0") 0usize, // Empty message
            in("a1") 0usize,
            in("a7") 2usize, // Panic
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

// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/syscalls/src/allocator.rs#L3
pub const MAX_MEMORY: usize = 0xC000_0000;
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
        panic!("Memory limit exceeded (0xC0000000)");
    }

    unsafe { HEAP_POS = heap_pos };
    ptr
}
