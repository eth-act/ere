mod rust_rv64imac;
mod rust_rv64imac_customized;

pub use rust_rv64imac::RustRv64imac;
pub use rust_rv64imac_customized::RustRv64imacCustomized;

pub type JoltProgram = Vec<u8>;
