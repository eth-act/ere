use core::{
    alloc::{GlobalAlloc, Layout},
    panic::PanicInfo,
};

// According to https://github.com/a16z/jolt/blob/6dcd401/common/src/jolt_device.rs#L189
const DEFAULT_TERMINATION_ADDR: usize = 0x7FFFC008;
const DEFAULT_PANIC_ADDR: usize = 0x7FFFC000;

// According to https://github.com/a16z/jolt/blob/6dcd401/jolt-sdk/macros/src/lib.rs#L808
core::arch::global_asm!(
    r#"
.global _start
.extern _STACK_PTR
.section .text.boot
_start:
    la sp, _STACK_PTR
    call main
    j .
"#
);

#[no_mangle]
pub extern "C" fn main() {
    crate::main();
    unsafe { core::ptr::write_volatile(DEFAULT_TERMINATION_ADDR as *mut u8, 1) };
}

// According to https://github.com/a16z/jolt/blob/6dcd401/jolt-sdk/macros/src/lib.rs
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unsafe { core::ptr::write_volatile(DEFAULT_PANIC_ADDR as *mut u8, 1) };
    loop {}
}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator;

// According to https://github.com/a16z/jolt/blob/6dcd401/jolt-platform/src/alloc.rs
pub struct BumpAllocator;

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        sys_alloc(layout.size(), layout.align())
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

extern "C" {
    static _HEAP_PTR: u8;
}

static mut ALLOC_NEXT: usize = 0;

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "C" fn sys_alloc(size: usize, align: usize) -> *mut u8 {
    let mut next = unsafe { ALLOC_NEXT };

    if next == 0 {
        next = unsafe { (&_HEAP_PTR) as *const u8 as usize };
    }

    next = align_up(next, align);

    let ptr = next as *mut u8;
    next += size;

    unsafe { ALLOC_NEXT = next };
    ptr
}

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
