#![no_std]

extern crate alloc;

use core::{array::from_fn, cell::UnsafeCell, hash::Hasher, ops::Deref};
use ere_platform_trait::LengthPrefixedStdin;
use fnv::FnvHasher;
use ziskos::ziskos_definitions::ziskos_config::UART_ADDR;

pub use ere_platform_trait::{Digest, OutputHashedPlatform, Platform};
pub use ziskos;

/// Hashes a scope name to a `u64` for use as a lookup key in the scope registry.
#[inline]
fn hash_name(name: &str) -> u64 {
    let mut hasher = FnvHasher::default();
    hasher.write(name.as_bytes());
    hasher.finish()
}

/// Global registry mapping scope name hashes to sequential tag IDs.
///
/// Each unique scope name gets a unique tag ID (0, 1, 2, ...) on first use.
/// Panics if more than 256 distinct scope names are registered.
struct ScopeRegistry {
    entries: UnsafeCell<[u64; 256]>, // name hashes; tag = index
    count: UnsafeCell<u8>,
}

// SAFETY: ZiskPlatform runs in a single-threaded zkVM environment.
unsafe impl Sync for ScopeRegistry {}

impl ScopeRegistry {
    const fn new() -> Self {
        Self {
            entries: UnsafeCell::new([0; 256]),
            count: UnsafeCell::new(0),
        }
    }

    /// Looks up or assigns a tag ID for the given scope name.
    fn get_or_assign_tag(&self, name: &str) -> u8 {
        let name_hash = hash_name(name);
        // SAFETY: Single-threaded zkVM — no concurrent access.
        unsafe {
            let entries = &mut *self.entries.get();
            let count = &mut *self.count.get();

            for i in 0..*count as usize {
                if entries[i] == name_hash {
                    return i as u8;
                }
            }

            assert!(
                (*count as u16) < 256,
                "Too many profiling scopes (max 256), cannot assign tag for scope"
            );
            entries[*count as usize] = name_hash;
            let tag = *count;
            *count += 1;
            tag
        }
    }
}

static SCOPE_REGISTRY: ScopeRegistry = ScopeRegistry::new();

/// Dispatches a runtime `u8` tag to a const-generic ziskos profiling function.
/// Generates a 256-arm match to bridge the runtime value to compile-time const generics.
macro_rules! dispatch_profile {
    (start, $tag:expr) => {
        dispatch_profile!(@start $tag,
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
            16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
            32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
            48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
            64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79,
            80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95,
            96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111,
            112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127,
            128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143,
            144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159,
            160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175,
            176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191,
            192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207,
            208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223,
            224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239,
            240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255)
    };
    (end, $tag:expr) => {
        dispatch_profile!(@end $tag,
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
            16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
            32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
            48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
            64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79,
            80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95,
            96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111,
            112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127,
            128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143,
            144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159,
            160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175,
            176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191,
            192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207,
            208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223,
            224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239,
            240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255)
    };
    (@start $tag:expr, $($n:literal),+) => {
        match $tag {
            $($n => ziskos::ziskos_profile_start::<$n>(),)+
        }
    };
    (@end $tag:expr, $($n:literal),+) => {
        match $tag {
            $($n => ziskos::ziskos_profile_end::<$n>(),)+
        }
    };
}

/// ZisK [`Platform`] implementation.
///
/// Note that the maximum output size is 256 bytes, and output size will be
/// padded to multiple of 4.
pub struct ZiskPlatform;

impl Platform for ZiskPlatform {
    fn read_whole_input() -> impl Deref<Target = [u8]> {
        LengthPrefixedStdin::new(ziskos::read_input())
    }

    fn write_whole_output(output: &[u8]) {
        assert!(
            output.len() <= 256,
            "Maximum output size is 256 bytes, got {}",
            output.len()
        );
        output.chunks(4).enumerate().for_each(|(idx, chunk)| {
            let value = u32::from_le_bytes(from_fn(|i| chunk.get(i).copied().unwrap_or_default()));
            ziskos::set_output(idx, value)
        });
    }

    fn print(message: &str) {
        let bytes = message.as_bytes();
        for byte in bytes {
            unsafe {
                core::ptr::write_volatile(UART_ADDR as *mut u8, *byte);
            }
        }
    }

    fn cycle_scope_start(name: &str) {
        let tag = SCOPE_REGISTRY.get_or_assign_tag(name);
        dispatch_profile!(start, tag);
    }

    fn cycle_scope_end(name: &str) {
        let tag = SCOPE_REGISTRY.get_or_assign_tag(name);
        dispatch_profile!(end, tag);
    }
}
