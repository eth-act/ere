use std::sync::OnceLock;

/// Run a future to completion, reusing the current tokio runtime or creating one.
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => {
            static FALLBACK_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
            FALLBACK_RT
                .get_or_init(|| tokio::runtime::Runtime::new().expect("Failed to create runtime"))
                .block_on(future)
        }
    }
}
