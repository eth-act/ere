use ere_platform_risc0::risc0_zkvm::guest::env::read_slice;

fn main() {
    let mut alignment = [0; 4];
    read_slice(&mut alignment);
    let alignment = u32::from_le_bytes(alignment) as usize;

    let layout = std::alloc::Layout::from_size_align(1, alignment).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        panic!("allocation failed");
    }
}
