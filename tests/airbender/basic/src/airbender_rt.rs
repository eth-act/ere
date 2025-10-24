use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::addr_of_mut,
};

core::arch::global_asm!(include_str!("./asm_reduced.S"));

unsafe extern "C" {
    // Boundaries of the heap
    static mut _sheap: usize;
    static mut _eheap: usize;

    // Boundaries of the .data section (and it's part in ROM)
    static mut _sidata: usize;
    static mut _sdata: usize;
    static mut _edata: usize;

    // Boundaries of the .rodata section
    static mut _sirodata: usize;
    static mut _srodata: usize;
    static mut _erodata: usize;
}

unsafe fn load_to_ram(src: *const u8, dst_start: *mut u8, dst_end: *mut u8) {
    let offset = dst_end.addr() - dst_start.addr();

    unsafe { core::ptr::copy_nonoverlapping(src, dst_start, offset) };
}

#[unsafe(link_section = ".init.rust")]
#[unsafe(export_name = "_start_rust")]
unsafe extern "C" fn start_rust() -> ! {
    unsafe {
        load_to_ram(
            addr_of_mut!(_sirodata) as *const u8,
            addr_of_mut!(_srodata) as *mut u8,
            addr_of_mut!(_erodata) as *mut u8,
        );
        load_to_ram(
            addr_of_mut!(_sidata) as *const u8,
            addr_of_mut!(_sdata) as *mut u8,
            addr_of_mut!(_edata) as *mut u8,
        );
    };

    crate::main();

    unsafe { core::hint::unreachable_unchecked() }
}

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
    let mut heap_pos = unsafe { HEAP_POS };

    if heap_pos == 0 {
        heap_pos = addr_of_mut!(_sheap) as *const u8 as usize;
    }

    let offset = heap_pos & (align - 1);
    if offset != 0 {
        heap_pos += align - offset;
    }

    let ptr = heap_pos as *mut u8;
    let (heap_pos, overflowed) = heap_pos.overflowing_add(bytes);

    let eheap = addr_of_mut!(_eheap) as *const u8 as usize;
    if overflowed || heap_pos > eheap {
        panic!("heap exhausted");
    }

    unsafe { HEAP_POS = heap_pos };
    ptr
}

#[cfg(all(target_arch = "riscv32", target_feature = "a"))]
#[unsafe(no_mangle)]
fn _critical_section_1_0_acquire() -> u32 {
    0
}

#[cfg(all(target_arch = "riscv32", target_feature = "a"))]
#[unsafe(no_mangle)]
fn _critical_section_1_0_release(_: u32) {}
