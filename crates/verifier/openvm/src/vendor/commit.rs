use openvm_continuations::F;
use openvm_stark_sdk::{
    openvm_stark_backend::p3_field::{FieldAlgebra, PrimeField32},
    p3_baby_bear::BabyBear,
    p3_bn254_fr::Bn254Fr,
};
use serde::{Deserialize, Serialize};

const BN254_BYTES: usize = 32;
const DIGEST_SIZE: usize = 8;

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitBytes(pub [u8; BN254_BYTES]);

impl CommitBytes {
    #[allow(clippy::wrong_self_convention)]
    pub fn to_bn254(&self) -> Bn254Fr {
        bytes_to_bn254(&self.0)
    }

    pub fn from_u32_digest(digest: &[u32; DIGEST_SIZE]) -> Self {
        Self(u32_digest_to_bytes(digest))
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppExecutionCommit {
    pub app_exe_commit: CommitBytes,
    pub app_vm_commit: CommitBytes,
}

impl AppExecutionCommit {
    pub fn from_field_commit<G: PrimeField32>(
        exe_commit: [G; DIGEST_SIZE],
        vm_commit: [G; DIGEST_SIZE],
    ) -> Self {
        Self {
            app_exe_commit: CommitBytes::from_u32_digest(&exe_commit.map(|x| x.as_canonical_u32())),
            app_vm_commit: CommitBytes::from_u32_digest(&vm_commit.map(|x| x.as_canonical_u32())),
        }
    }
}

fn bytes_to_bn254(bytes: &[u8; BN254_BYTES]) -> Bn254Fr {
    let order = Bn254Fr::from_canonical_u32(1 << 8);
    let mut ret = Bn254Fr::ZERO;
    let mut base = Bn254Fr::ONE;
    for byte in bytes.iter().rev() {
        ret += base * Bn254Fr::from_canonical_u8(*byte);
        base *= order;
    }
    ret
}

fn bn254_to_bytes(bn254: Bn254Fr) -> [u8; BN254_BYTES] {
    let mut ret = bn254.value.to_bytes();
    ret.reverse();
    ret
}

fn u32_digest_to_bytes(digest: &[u32; DIGEST_SIZE]) -> [u8; BN254_BYTES] {
    bn254_to_bytes(babybear_digest_to_bn254(&digest.map(F::from_canonical_u32)))
}

fn babybear_digest_to_bn254(digest: &[F; DIGEST_SIZE]) -> Bn254Fr {
    let mut ret = Bn254Fr::ZERO;
    let order = Bn254Fr::from_canonical_u32(BabyBear::ORDER_U32);
    let mut base = Bn254Fr::ONE;
    digest.iter().for_each(|&x| {
        ret += base * Bn254Fr::from_canonical_u32(x.as_canonical_u32());
        base *= order;
    });
    ret
}
