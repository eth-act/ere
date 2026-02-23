use core::alloc::{GlobalAlloc, Layout};

#[no_mangle]
unsafe extern "C" fn _zisk_main() {
    crate::main();
}

// According to https://github.com/0xPolygonHermez/zisk/blob/pre-develop-0.16.0/ziskos/entrypoint/src/lib.rs#L116
core::arch::global_asm!(
    r#"
.section .text.init
.globl _start
_start:
    .option push
    .option norelax
    la gp, _global_pointer
    .option pop

    la sp, _init_stack_top

    call _zisk_main

    li a7, 93
    ecall

    j .
"#,
);

// According to https://github.com/0xPolygonHermez/rust/blob/zisk/library/std/src/sys/pal/zisk/mod.rs#L48
#[panic_handler]
fn panic_impl(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::arch::asm!("unimp", options(noreturn)) }
}

struct SimpleAlloc;

unsafe impl GlobalAlloc for SimpleAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        sys_alloc_aligned(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static HEAP: SimpleAlloc = SimpleAlloc;

// According to https://github.com/0xPolygonHermez/zisk/blob/pre-develop-0.16.0/ziskos/entrypoint/src/lib.rs#L241
#[no_mangle]
pub unsafe extern "C" fn sys_alloc_aligned(bytes: usize, align: usize) -> *mut u8 {
    use core::arch::asm;
    let heap_bottom: usize;
    // UNSAFE: This is fine, just loading some constants.
    unsafe {
        // using inline assembly is easier to access linker constants
        asm!(
          "la {heap_bottom}, _kernel_heap_bottom",
          heap_bottom = out(reg) heap_bottom,
          options(nomem)
        )
    };

    // Pointer to next heap address to use, or 0 if the heap has not yet been
    // initialized.
    static mut HEAP_POS: usize = 0;

    // SAFETY: Single threaded, so nothing else can touch this while we're working.
    let mut heap_pos = unsafe { HEAP_POS };

    if heap_pos == 0 {
        heap_pos = heap_bottom;
    }

    let offset = heap_pos & (align - 1);
    if offset != 0 {
        heap_pos += align - offset;
    }

    let ptr = heap_pos as *mut u8;
    heap_pos += bytes;

    // Check to make sure heap doesn't collide with SYSTEM memory.
    //if SYSTEM_START < heap_pos {
    //    panic!();
    // }

    unsafe { HEAP_POS = heap_pos };

    ptr
}

core::arch::global_asm!(include_str!("memcpy.s"));
core::arch::global_asm!(include_str!("memmove.s"));
