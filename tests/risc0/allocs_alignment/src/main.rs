use risc0_zkvm::guest::env;

fn main() {
    let alignment = env::read::<usize>();

    let layout = std::alloc::Layout::from_size_align(1, alignment).unwrap();
    let ptr = unsafe { std::alloc::alloc(layout) };
    if ptr.is_null() {
        panic!("allocation failed");
    }
}
