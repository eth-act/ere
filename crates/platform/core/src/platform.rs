use core::ops::Deref;

/// Platform dependent methods.
pub trait Platform {
    /// Reads the whole input from host.
    ///
    /// The default implementation calls the [zkvm-standards] `read_input` C ABI
    /// symbol. zkVMs whose runtime exports that symbol can rely on this
    /// default. Other zkVMs should implement with their SDK's input API.
    ///
    /// Note that this function should only be called once.
    ///
    /// [zkvm-standards]: https://github.com/eth-act/zkvm-standards
    fn read_input() -> impl Deref<Target = [u8]> {
        let mut buf_ptr: *const u8 = core::ptr::null();
        let mut buf_size: usize = 0;
        unsafe { zkvm_io::read_input(&mut buf_ptr, &mut buf_size) };
        if buf_size == 0 {
            [].as_slice()
        } else {
            unsafe { core::slice::from_raw_parts(buf_ptr, buf_size) }
        }
    }

    /// Writes the whole output to host.
    ///
    /// The default implementation calls the [zkvm-standards] `write_output` C ABI
    /// symbol. zkVMs whose runtime exports that symbol can rely on this
    /// default. Other zkVMs should implement with their SDK's output API.
    ///
    /// Note that this function should only be called once.
    ///
    /// [zkvm-standards]: https://github.com/eth-act/zkvm-standards
    fn write_output(output: &[u8]) {
        unsafe { zkvm_io::write_output(output.as_ptr(), output.len()) };
    }

    /// Prints a message to the host environment.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    fn print(message: &str);

    /// Returns the current cycle count.
    ///
    /// Note that this function will return `0` if the platform doesn't support.
    #[inline]
    fn cycle_count() -> u64 {
        0
    }

    /// Enters a cycle scope of `name`.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    #[inline]
    fn cycle_scope_start(_name: &str) {}

    /// Exits a cycle scope of `name`.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    #[inline]
    fn cycle_scope_end(_name: &str) {}

    /// Runs a given function `f` within a cycle scope `name`.
    ///
    /// Note that this function will be a no-op if the platform doesn't support.
    #[inline]
    fn cycle_scope<T>(name: &str, f: impl FnOnce() -> T) -> T {
        Self::cycle_scope_start(name);
        let t = f();
        Self::cycle_scope_end(name);
        t
    }
}

/// FFI bindings for the [zkvm-standards] guest I/O C ABI.
///
/// [`Platform::read_input`] and [`Platform::write_output`] default impls call
/// into them. If default impls are used, these symbols are expected to be
/// exported by the zkVM runtime.
///
/// [zkvm-standards]: https://github.com/eth-act/zkvm-standards
mod zkvm_io {
    unsafe extern "C" {
        /// Reads the input buffer, setting `*buf_ptr` and `*buf_size`.
        pub(super) fn read_input(buf_ptr: *mut *const u8, buf_size: *mut usize);

        /// Writes `size` bytes from `output` to the public output.
        pub(super) fn write_output(output: *const u8, size: usize);
    }
}
