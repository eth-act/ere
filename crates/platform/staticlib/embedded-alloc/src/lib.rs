//! The zkVM staticlibs' stand-in for `embedded-alloc` 0.6, whose heaps the vendors' runtimes
//! register as their global allocator: SP1's `TlsfHeap`, OpenVM's `LlffHeap` and, with
//! `zisk-embedded-tlfs-alloc`, ZisK's `TlsfHeap`. The workspace patches it in for the staticlib
//! builds, so each vendor allocates from its own region, apart from the guest's heap.
//!
//! The region runs from `_end`, the end of the program's static data, to `_heap_start`, where the
//! static library's linker script starts the guest's heap (`_heap_start` to `_heap_end`), so the
//! linker script alone sets its size. The range a vendor passes to [`Heap::init`], which it derives
//! from its own memory layout, is ignored.
//!
//! It is a stack of blocks that frees in any order. Freeing the most recent block pops it, then
//! every block directly beneath it that was freed before; freeing an older block only marks it. An
//! accelerator call drops everything it allocates before it returns, in whatever order, so the
//! stack is back where it was when the call began: no wrapper needs to tell the allocator where a
//! call starts or ends. What outlives a call, such as the input a vendor reads once and keeps,
//! stays beneath what later calls allocate.
//!
//! Each block has a header of two words: where the stack's top was before it (to pop it), and the
//! block beneath it, whose lowest bit marks this block freed. Blocks are 8-byte aligned at least,
//! as the vendors' own allocators give (OpenVM reads its input into a `Vec<u8>` word by word).

#![cfg_attr(not(test), no_std)]

use core::{
    alloc::{GlobalAlloc, Layout},
    cell::Cell,
    ptr,
};

/// Alignment of every block.
const MIN_ALIGN: usize = 8;
/// Bytes of the header before every block.
const HEADER: usize = 2 * size_of::<usize>();
/// The lowest bit of a header's second word: the block is freed.
const FREED: usize = 1;

#[cfg(not(test))]
unsafe extern "C" {
    /// End of the program's static data: the first byte of the vendor's region.
    static _end: u8;
    /// First byte of the guest's heap: one past the last byte of the vendor's region.
    static _heap_start: u8;
}

// The linker writes both addresses into the statics' initial values, so code reaches them wherever
// the region is. They are read volatile so the optimizer cannot fold them back into PC-relative
// references, which reach only ±2 GiB.
#[cfg(not(test))]
static mut REGION_START: *const u8 = &raw const _end;
#[cfg(not(test))]
static mut REGION_END: *const u8 = &raw const _heap_start;

/// A stack allocator over the vendor's region that frees in any order.
pub struct Heap {
    /// The first free byte.
    next: Cell<usize>,
    /// The most recent block, or 0 if there is none.
    top: Cell<usize>,
    /// One past the last byte of the region.
    end: Cell<usize>,
}

// zkVM guests run on one thread.
unsafe impl Sync for Heap {}

/// `embedded-alloc`'s linked-list heap, as OpenVM names it.
pub type LlffHeap = Heap;
/// `embedded-alloc`'s TLSF heap, as SP1 and ZisK name it.
pub type TlsfHeap = Heap;

impl Heap {
    /// A heap with no memory, until [`Heap::init`].
    pub const fn empty() -> Self {
        Self {
            next: Cell::new(0),
            top: Cell::new(0),
            end: Cell::new(0),
        }
    }

    /// Gives the heap the vendor's region, `_end` to `_heap_start`. `embedded-alloc`'s
    /// arguments, the range the vendor derives from its own layout, are ignored.
    ///
    /// # Safety
    ///
    /// The heap must not be in use.
    #[cfg(not(test))]
    pub unsafe fn init(&self, _start_addr: usize, _size: usize) {
        unsafe {
            let start = (&raw const REGION_START).read_volatile().addr();
            let end = (&raw const REGION_END).read_volatile().addr();
            self.init_region(start, end);
        }
    }

    /// Gives the heap the region `start` to `end`.
    fn init_region(&self, start: usize, end: usize) {
        self.next.set(start);
        self.top.set(0);
        self.end.set(end);
    }

    /// The two words of `block`'s header.
    fn header(block: usize) -> *mut [usize; 2] {
        (block - HEADER) as *mut [usize; 2]
    }
}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Address arithmetic that would overflow fails rather than wraps.
        let mask = layout.align().max(MIN_ALIGN) - 1;
        let Some(block) = self
            .next
            .get()
            .checked_add(HEADER + mask)
            .map(|a| a & !mask)
        else {
            return ptr::null_mut();
        };
        match block.checked_add(layout.size()) {
            Some(end) if end <= self.end.get() => {
                unsafe { Self::header(block).write([self.next.get(), self.top.get()]) };
                self.top.set(block);
                self.next.set(end);
                block as *mut u8
            }
            _ => ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _: Layout) {
        let block = ptr as usize;
        if block != self.top.get() {
            unsafe { (*Self::header(block))[1] |= FREED };
            return;
        }
        // Pop the block, then every freed block directly beneath it.
        loop {
            let [next, below] = unsafe { Self::header(self.top.get()).read() };
            self.next.set(next);
            self.top.set(below & !FREED);
            let top = self.top.get();
            if top == 0 || unsafe { (*Self::header(top))[1] } & FREED == 0 {
                break;
            }
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // The most recent block grows or shrinks in place.
        let block = ptr as usize;
        if block == self.top.get() {
            match block.checked_add(new_size) {
                Some(end) if end <= self.end.get() => {
                    self.next.set(end);
                    return ptr;
                }
                _ => return ptr::null_mut(),
            }
        }
        let new =
            unsafe { self.alloc(Layout::from_size_align_unchecked(new_size, layout.align())) };
        if !new.is_null() {
            unsafe {
                ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
        }
        new
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A heap over a region of `size` bytes, with the region kept alive.
    fn heap(size: usize) -> (Heap, Vec<u64>) {
        let mut region = vec![0u64; size / 8];
        let start = region.as_mut_ptr() as usize;
        let heap = Heap::empty();
        heap.init_region(start, start + size);
        (heap, region)
    }

    fn layout(size: usize, align: usize) -> Layout {
        Layout::from_size_align(size, align).unwrap()
    }

    #[test]
    fn frees_in_any_order_back_to_the_start() {
        let (heap, _region) = heap(4096);
        let start = heap.next.get();
        unsafe {
            let a = heap.alloc(layout(24, 8));
            let b = heap.alloc(layout(100, 16));
            let c = heap.alloc(layout(8, 8));
            heap.dealloc(b, layout(100, 16));
            heap.dealloc(a, layout(24, 8));
            assert_ne!(heap.next.get(), start, "older blocks are only marked");
            heap.dealloc(c, layout(8, 8));
        }
        assert_eq!(heap.next.get(), start);
        assert_eq!(heap.top.get(), 0);
    }

    #[test]
    fn a_live_block_beneath_keeps_its_place() {
        let (heap, _region) = heap(4096);
        unsafe {
            let input = heap.alloc(layout(64, 8));
            let after_input = heap.next.get();
            for _ in 0..3 {
                let x = heap.alloc(layout(32, 8));
                let y = heap.alloc(layout(48, 8));
                heap.dealloc(x, layout(32, 8));
                heap.dealloc(y, layout(48, 8));
                assert_eq!(heap.next.get(), after_input);
            }
            assert_eq!(heap.top.get(), input as usize);
        }
    }

    #[test]
    fn aligns_blocks_and_their_headers() {
        let (heap, _region) = heap(4096);
        unsafe {
            for align in [1, 2, 8, 16, 64, 256] {
                let p = heap.alloc(layout(3, align)) as usize;
                assert_eq!(p % align.max(MIN_ALIGN), 0);
                assert_eq!((p - HEADER) % size_of::<usize>(), 0);
            }
        }
    }

    #[test]
    fn realloc_grows_the_top_in_place_and_moves_others() {
        let (heap, _region) = heap(4096);
        unsafe {
            let a = heap.alloc(layout(16, 8));
            a.write_bytes(7, 16);
            let grown = heap.realloc(a, layout(16, 8), 64);
            assert_eq!(grown, a);
            let b = heap.alloc(layout(8, 8));
            let moved = heap.realloc(a, layout(64, 8), 128);
            assert_ne!(moved, a);
            assert_eq!(*moved.add(15), 7);
            heap.dealloc(moved, layout(128, 8));
            heap.dealloc(b, layout(8, 8));
            assert_eq!(
                heap.top.get(),
                0,
                "the old copy of `a` was freed when it moved"
            );
        }
    }

    #[test]
    fn returns_null_when_the_region_is_full() {
        let (heap, _region) = heap(256);
        unsafe {
            assert!(heap.alloc(layout(1024, 8)).is_null());
            assert!(heap.alloc(layout(isize::MAX as usize - 64, 8)).is_null());
            let a = heap.alloc(layout(200, 8));
            assert!(!a.is_null());
            assert!(heap.realloc(a, layout(200, 8), 4096).is_null());
        }
    }
}
