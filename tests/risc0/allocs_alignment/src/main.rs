use risc0_zkvm::guest::env;

fn main() {
    let alignment = {
        let mut buf = [0; 4];
        env::read_slice(&mut buf);
        u32::from_le_bytes(buf) as usize
    };

    let layout = std::alloc::Layout::from_size_align(1, alignment).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        panic!("allocation failed");
    }
}
