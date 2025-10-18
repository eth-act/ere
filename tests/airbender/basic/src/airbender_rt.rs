use core::alloc::{GlobalAlloc, Layout};

core::arch::global_asm!(include_str!("./asm_reduced.S"));

#[link_section = ".init.rust"]
#[export_name = "_start_rust"]
unsafe extern "C" fn start_rust() -> ! {
    crate::main();
    unsafe { core::hint::unreachable_unchecked() }
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

static mut HEAP_POS: usize = 0;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sys_alloc_aligned(bytes: usize, align: usize) -> *mut u8 {
    unsafe extern "C" {
        // start/end of heap defined in `link.x`.
        unsafe static _sheap: u8;
        unsafe static _eheap: u8;
    }

    // SAFETY: Single threaded, so nothing else can touch this while we're working.
    let mut heap_pos = unsafe { HEAP_POS };

    if heap_pos == 0 {
        heap_pos = unsafe { (&_sheap) as *const u8 as usize };
    }

    let offset = heap_pos & (align - 1);
    if offset != 0 {
        heap_pos += align - offset;
    }

    let ptr = heap_pos as *mut u8;
    let (heap_pos, overflowed) = heap_pos.overflowing_add(bytes);

    let eheap = unsafe { (&_eheap) as *const u8 as usize };
    if overflowed || heap_pos > eheap {
        panic!("Heap exhausted");
    }

    unsafe { HEAP_POS = heap_pos };
    ptr
}
