#![cfg_attr(target_arch = "riscv32", no_std, no_main)]

use nexus_rt::println;

#[nexus_rt::main]
#[nexus_rt::private_input(x)]
fn main(x: u32) {
    println!("Read public input:  {}", x);
    let res = fibonacci(x);
    println!("fib result:  {}", res);
}
pub fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 1,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
