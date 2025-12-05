#[rustfmt::skip]
pub mod api;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

#[cfg(test)]
mod test;
