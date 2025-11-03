use ere_platform_risc0::{Platform, Risc0Platform};

fn main() {
    let alignment =
        u32::from_le_bytes(Risc0Platform::read_whole_input().try_into().unwrap()) as usize;

    let layout = std::alloc::Layout::from_size_align(1, alignment).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        panic!("allocation failed");
    }
}
