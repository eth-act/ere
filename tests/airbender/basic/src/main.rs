#![no_std]
#![no_main]
#![no_builtins]
#![allow(incomplete_features)]
#![feature(allocator_api)]
#![feature(generic_const_exprs)]

use ere_platform_airbender::AirbenderPlatform;
use ere_test_utils::{
    guest::Sha256,
    io::serde::bincode::BincodeLegacy,
    program::{basic::BasicProgram, Program},
};

mod airbender_rt;

#[inline(never)]
fn main() {
    BasicProgram::<BincodeLegacy>::run::<AirbenderPlatform<Sha256>>();
}
