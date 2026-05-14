use core::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use ere_catalog::zkVMKind;
use ere_verifier::Verifier;

macro_rules! bench_verifier {
    ($zkvm_kind:ident) => {
        paste::paste! {
            fn [<bench_ $zkvm_kind:lower>](c: &mut Criterion) {
                const PROGRAM_VK: &[u8] = include_bytes!(concat!("../../", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/program_vk.bin"));
                const PROOF: &[u8] = include_bytes!(concat!("../../", stringify!([<$zkvm_kind:lower>]), "/tests/fixtures/proof.bin"));

                let verifier = Verifier::new(zkVMKind::$zkvm_kind, PROGRAM_VK).unwrap();

                let id = concat!("verify/", stringify!([<$zkvm_kind:lower>]));
                c.bench_function(id, |b| {
                    b.iter(|| verifier.verify(black_box(PROOF)).unwrap());
                });
            }
        }
    };
}

bench_verifier!(Airbender);
bench_verifier!(OpenVM);
bench_verifier!(Risc0);
bench_verifier!(SP1);
bench_verifier!(Zisk);

criterion_group!(
    verify,
    bench_airbender,
    bench_openvm,
    bench_risc0,
    bench_sp1,
    bench_zisk,
);
criterion_main!(verify);
