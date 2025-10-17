pub mod client;

#[allow(dead_code)]
pub(crate) mod api {
    include!(concat!(env!("OUT_DIR"), "/api.rs"));
}

#[cfg(feature = "server")]
pub mod server;
