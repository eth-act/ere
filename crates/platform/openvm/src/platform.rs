use alloc::vec::Vec;

use ere_platform_core::Platform;

/// Maximum bytes of output the guest may reveal.
pub const MAX_OUTPUT_BYTES: usize = 256;

/// Input that the first [`read_input`] reads from the hint stream.
static mut INPUT: Option<&'static [u8]> = None;

/// Output that [`write_output`] revealed so far, padded with zeros to whole words.
static mut OUTPUT: [u8; MAX_OUTPUT_BYTES] = [0; MAX_OUTPUT_BYTES];

/// Bytes of [`OUTPUT`] that [`write_output`] wrote so far.
static mut OUTPUT_LEN: usize = 0;

/// OpenVM [`Platform`] implementation.
///
/// `read_input` and `write_output` are inherited from the trait's default implementation, which
/// calls the [zkvm-standards] FFI symbols that this crate exports.
///
/// Note that the maximum output size is 256 bytes, and output less than 256
/// bytes will be padded to 256 bytes.
///
/// [zkvm-standards]: https://github.com/eth-act/zkvm-standards
pub struct OpenVMPlatform;

impl Platform for OpenVMPlatform {
    fn print(message: &str) {
        openvm::io::print(message)
    }
}

/// Sets `*buf_ptr` and `*buf_size` to the input, which the first call reads from the hint stream.
///
/// # Safety
///
/// `buf_ptr` and `buf_size` must be valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn read_input(buf_ptr: *mut *const u8, buf_size: *mut usize) {
    let input = unsafe { INPUT }.unwrap_or_else(|| {
        let input: &'static [u8] = Vec::leak(openvm::io::read_vec());
        unsafe { INPUT = Some(input) };
        input
    });
    unsafe {
        buf_ptr.write(input.as_ptr());
        buf_size.write(input.len());
    }
}

/// Appends `size` bytes from `output` to the public output, and reveals each word that they touch.
///
/// # Safety
///
/// If `size > 0`, `output` must point to at least `size` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn write_output(output: *const u8, size: usize) {
    if size == 0 {
        return;
    }
    let start = unsafe { OUTPUT_LEN };
    let end = start + size;
    assert!(
        end <= MAX_OUTPUT_BYTES,
        "Maximum output size is {MAX_OUTPUT_BYTES} bytes, got {end} bytes"
    );
    let buffer = &raw mut OUTPUT;
    unsafe {
        output.copy_to_nonoverlapping(buffer.cast::<u8>().add(start), size);
        OUTPUT_LEN = end;
    }
    (start / 8..end.div_ceil(8)).for_each(|index| {
        let word = unsafe { buffer.cast::<[u8; 8]>().add(index).read() };
        openvm::io::reveal_u64(u64::from_le_bytes(word), index)
    });
}
