use ere_platform_core::Platform;

/// Maximum bytes of output the guest may reveal.
pub const MAX_OUTPUT_BYTES: usize = 256;

/// OpenVM [`Platform`] implementation.
///
/// Note that the maximum output size is 256 bytes, and output less than 256
/// bytes will be padded to 256 bytes.
pub struct OpenVMPlatform;

/// `read_input`, `write_output` and `abort` are the trait's defaults, on the [zkvm-standards] I/O
/// interface (`zkvm_io.h`) exported below.
///
/// [zkvm-standards]: https://github.com/eth-act/zkvm-standards
impl Platform for OpenVMPlatform {
    fn print(message: &str) {
        openvm::io::print(message)
    }
}

/// The input, read on the first call to [`read_input`].
static mut INPUT: Option<&'static [u8]> = None;

/// The output written so far, of which [`OUTPUT_LEN`] bytes are revealed.
static mut OUTPUT: [u8; MAX_OUTPUT_BYTES] = [0; MAX_OUTPUT_BYTES];
static mut OUTPUT_LEN: usize = 0;

/// The whole input. The first call reads it, and every call returns the same buffer.
#[unsafe(no_mangle)]
unsafe extern "C" fn read_input(buf_ptr: *mut *const u8, buf_size: *mut usize) {
    // zkVM guests run on one thread.
    let input = unsafe { &mut *(&raw mut INPUT) }
        .get_or_insert_with(|| alloc::vec::Vec::leak(openvm::io::read_vec()));
    unsafe {
        *buf_ptr = input.as_ptr();
        *buf_size = input.len();
    }
}

/// Appends `size` bytes from `output` to the output, revealing it 8 bytes at a time. At most
/// [`MAX_OUTPUT_BYTES`] may be written in total.
#[unsafe(no_mangle)]
unsafe extern "C" fn write_output(output: *const u8, size: usize) {
    // zkVM guests run on one thread.
    let (buffer, len) = unsafe { (&mut *(&raw mut OUTPUT), &mut *(&raw mut OUTPUT_LEN)) };
    let end = *len + size;
    assert!(
        end <= MAX_OUTPUT_BYTES,
        "Maximum output size is {MAX_OUTPUT_BYTES} bytes, got {end} bytes",
    );
    buffer[*len..end].copy_from_slice(unsafe { core::slice::from_raw_parts(output, size) });
    // Reveal every word the new bytes touch, including a partly written word of earlier output.
    for index in *len / 8..end.div_ceil(8) {
        let word = buffer[index * 8..index * 8 + 8].try_into().unwrap();
        openvm::io::reveal_u64(u64::from_le_bytes(word), index);
    }
    *len = end;
}

/// Failed termination, with exit code 1 (OpenVM's exit code is an instruction immediate).
#[unsafe(no_mangle)]
extern "C" fn abort() -> ! {
    openvm::platform::rust_rt::terminate::<1>()
}
