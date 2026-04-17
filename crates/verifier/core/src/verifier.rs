use core::error::Error;

use crate::{
    PublicValues,
    codec::{Decode, Encode},
};

/// zkVMProver verifier trait.
///
/// Note that a zkVMProver verifier instance is created for specific program.
#[allow(non_camel_case_types)]
#[auto_impl::auto_impl(&, Arc, Box)]
pub trait zkVMVerifier {
    type ProgramVk: 'static + Send + Sync + Encode + Decode;
    type Proof: 'static + Send + Sync + Encode + Decode;
    type Error: 'static + Send + Sync + Error;

    /// Verifies a proof of the program used to create this zkVMProver instance, then
    /// returns the public values extracted from the proof.
    #[must_use = "Public values must be used"]
    fn verify(&self, proof: &Self::Proof) -> Result<PublicValues, Self::Error>;

    /// Returns the verifying key for the specific program.
    fn program_vk(&self) -> &Self::ProgramVk;

    /// Returns the name of the zkVMProver.
    fn name(&self) -> &'static str;

    /// Returns the version of the zkVMProver SDK (e.g. 0.1.0).
    fn sdk_version(&self) -> &'static str;
}
