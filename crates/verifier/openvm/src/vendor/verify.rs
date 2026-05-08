use std::borrow::Borrow;

use openvm_circuit::{
    arch::{
        CONNECTOR_AIR_ID, PROGRAM_AIR_ID, PROGRAM_CACHED_TRACE_INDEX, PUBLIC_VALUES_AIR_ID,
        VmVerificationError,
        hasher::{Hasher, poseidon2::vm_poseidon2_hasher},
    },
    system::{
        memory::{CHUNK, merkle::public_values::UserPublicValuesProofError},
        program::trace::compute_exe_commit,
    },
};
use openvm_continuations::verifier::{
    common::types::VmVerifierPvs, internal::types::InternalVmVerifierPvs,
};
use openvm_stark_sdk::{
    config::baby_bear_poseidon2::BabyBearPoseidon2Engine,
    engine::{StarkEngine, StarkFriEngine},
};

use crate::{
    Error,
    vendor::{commit::AppExecutionCommit, keygen::AggVerifyingKey, proof::VmStarkProof},
};

pub fn verify_proof(
    agg_vk: &AggVerifyingKey,
    expected_app_commit: AppExecutionCommit,
    proof: &VmStarkProof,
) -> Result<(), Error> {
    if proof.inner.per_air.len() < 3 {
        return Err(VmVerificationError::NotEnoughAirs(proof.inner.per_air.len()).into());
    } else if proof.inner.per_air[0].air_id != PROGRAM_AIR_ID {
        return Err(VmVerificationError::SystemAirMissing {
            air_id: PROGRAM_AIR_ID,
        }
        .into());
    } else if proof.inner.per_air[1].air_id != CONNECTOR_AIR_ID {
        return Err(VmVerificationError::SystemAirMissing {
            air_id: CONNECTOR_AIR_ID,
        }
        .into());
    } else if proof.inner.per_air[2].air_id != PUBLIC_VALUES_AIR_ID {
        return Err(VmVerificationError::SystemAirMissing {
            air_id: PUBLIC_VALUES_AIR_ID,
        }
        .into());
    }
    let public_values_air_proof_data = &proof.inner.per_air[2];

    let program_commit = proof.inner.commitments.main_trace[PROGRAM_CACHED_TRACE_INDEX].as_ref();
    let internal_commit: &[_; CHUNK] = &agg_vk.internal_verifier_program_commit.into();

    let (fri_params_final, vk_final, claimed_app_vm_commit) = if program_commit == internal_commit {
        let internal_pvs: &InternalVmVerifierPvs<_> = public_values_air_proof_data
            .public_values
            .as_slice()
            .borrow();
        if internal_commit != &internal_pvs.extra_pvs.internal_program_commit {
            return Err(VmVerificationError::ProgramCommitMismatch { index: 0 }.into());
        }
        (
            agg_vk.internal_fri_params,
            &agg_vk.internal_vk,
            internal_pvs.extra_pvs.leaf_verifier_commit,
        )
    } else {
        (agg_vk.leaf_fri_params, &agg_vk.leaf_vk, *program_commit)
    };
    let e = BabyBearPoseidon2Engine::new(fri_params_final);
    e.verify(vk_final, &proof.inner)
        .map_err(VmVerificationError::from)?;

    let pvs: &VmVerifierPvs<_> =
        public_values_air_proof_data.public_values[..VmVerifierPvs::<u8>::width()].borrow();

    if let Some(exit_code) = pvs.connector.exit_code() {
        if exit_code != 0 {
            return Err(VmVerificationError::ExitCodeMismatch {
                expected: 0,
                actual: exit_code,
            }
            .into());
        }
    } else {
        return Err(VmVerificationError::IsTerminateMismatch {
            expected: true,
            actual: false,
        }
        .into());
    }

    let hasher = vm_poseidon2_hasher();
    let public_values_root = hasher.merkle_root(&proof.user_public_values);
    if public_values_root != pvs.public_values_commit {
        return Err(VmVerificationError::UserPublicValuesError(
            UserPublicValuesProofError::UserPublicValuesCommitMismatch,
        )
        .into());
    }

    let claimed_app_exe_commit = compute_exe_commit(
        &hasher,
        &pvs.app_commit,
        &pvs.memory.initial_root,
        pvs.connector.initial_pc,
    );
    let claimed_app_commit =
        AppExecutionCommit::from_field_commit(claimed_app_exe_commit, claimed_app_vm_commit);
    let exe_commit_bn254 = claimed_app_commit.app_exe_commit.to_bn254();
    let vm_commit_bn254 = claimed_app_commit.app_vm_commit.to_bn254();

    if exe_commit_bn254 != expected_app_commit.app_exe_commit.to_bn254() {
        return Err(Error::InvalidAppExeCommit {
            expected: expected_app_commit.app_exe_commit,
            actual: claimed_app_commit.app_exe_commit,
        });
    } else if vm_commit_bn254 != expected_app_commit.app_vm_commit.to_bn254() {
        return Err(Error::InvalidAppVmCommit {
            expected: expected_app_commit.app_vm_commit,
            actual: claimed_app_commit.app_vm_commit,
        });
    }
    Ok(())
}
