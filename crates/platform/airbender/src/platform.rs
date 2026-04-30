use alloc::vec::Vec;
use core::{array, fmt::Write, iter::repeat_with, ops::Deref};

use ere_platform_core::Platform;

/// Airbender [`Platform`] implementation.
///
/// Note that the maximum output size is 32 bytes, and output less than 32
/// bytes will be padded to 32 bytes.
pub struct AirbenderPlatform;

impl Platform for AirbenderPlatform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        let len = airbender::rt::sys::read_word() as usize;
        repeat_with(airbender::rt::sys::read_word)
            .take(len.div_ceil(4))
            .flat_map(u32::to_le_bytes)
            .take(len)
            .collect::<Vec<_>>()
    }

    fn write_whole_output(output: &[u8]) {
        assert!(
            output.len() <= 32,
            "Maximum output size is 32 bytes, got {} bytes",
            output.len()
        );
        let words = array::from_fn(|i| {
            u32::from_le_bytes(array::from_fn(|j| *output.get(4 * i + j).unwrap_or(&0)))
        });
        airbender::rt::sys::exit_success(&words)
    }

    fn print(message: &str) {
        let _ = airbender::rt::uart::QuasiUart::new().write_str(message);
    }
}

#[cfg(not(feature = "allocator-custom"))]
#[macro_export]
macro_rules! entrypoint {
    ($name:ident) => {
        #[unsafe(no_mangle)]
        #[unsafe(link_section = ".init.rust")]
        pub extern "C" fn _start_rust() -> ! {
            $crate::airbender::rt::start(|| {
                $name();
                unsafe { core::hint::unreachable_unchecked() }
            })
        }
    };
}
