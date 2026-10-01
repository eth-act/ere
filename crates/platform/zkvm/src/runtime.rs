//! The guest's runtime: a heap on the static library's `_heap_start`/`_heap_end`, and a panic
//! handler on [`Platform::abort`].

use core::alloc::{GlobalAlloc, Layout};

use ere_platform_core::Platform;

use crate::ZkvmPlatform;

unsafe extern "C" {
    /// First byte of the guest's heap, defined by the static library's linker script.
    static _heap_start: u8;
    /// One past the last byte of the guest's heap.
    static _heap_end: u8;
}

// The linker writes both addresses into the statics' initial values, so they need no startup code,
// and the heap may lie anywhere, even beyond the ±2 GiB that code reaches PC-relatively (SP1's).
/// Next free byte of the heap.
static mut NEXT: *mut u8 = &raw const _heap_start as *mut u8;
/// End of the heap, read volatile so the optimizer cannot fold it back into a PC-relative
/// `_heap_end`.
static mut END: *const u8 = &raw const _heap_end;

/// A bump allocator: the zkVM runs one program to completion, so it never frees.
struct Heap;

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Address arithmetic that would overflow fails rather than wraps.
        let mask = layout.align() - 1;
        unsafe {
            let Some(start) = NEXT.addr().checked_add(mask).map(|next| next & !mask) else {
                return core::ptr::null_mut();
            };
            match start.checked_add(layout.size()) {
                Some(end) if end <= (&raw const END).read_volatile().addr() => {
                    NEXT = NEXT.with_addr(end);
                    NEXT.with_addr(start)
                }
                _ => core::ptr::null_mut(),
            }
        }
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[global_allocator]
static HEAP: Heap = Heap;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    ZkvmPlatform::abort()
}
