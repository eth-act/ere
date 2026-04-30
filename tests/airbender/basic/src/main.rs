#![no_main]

use ere_platform_airbender::{entrypoint, AirbenderPlatform};
use ere_util_test::{
    codec::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

entrypoint!(main);

fn main() {
    BasicProgram::<BincodeLegacy>::run_output_sha256::<AirbenderPlatform>();
}
