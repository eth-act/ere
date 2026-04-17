use crate::PublicValues;
use crate::codec::{Decode, Encode};
use core::error::Error;

/// zkVM verifier trait.
///
/// Note that a zkVM verifier instance is created for specific program.
#[allow(non_camel_case_types)]
#[auto_impl::auto_impl(&, Arc, Box)]
pub trait zkVMVerifier {
    type ProgramVk: 'static + Send + Sync + Encode + Decode;
    type Proof: 'static + Send + Sync + Encode + Decode;
    type Error: 'static + Send + Sync + Error;

    /// Verifies a proof of the program used to create this zkVM instance, then
    /// returns the public values extracted from the proof.
    #[must_use = "Public values must be used"]
    fn verify(&self, proof: &Self::Proof) -> Result<PublicValues, Self::Error>;

    /// Returns the verifying key for the specific program.
    fn program_vk(&self) -> &Self::ProgramVk;

    /// Returns the name of the zkVM.
    fn name(&self) -> &'static str;

    /// Returns the version of the zkVM SDK (e.g. 0.1.0).
    fn sdk_version(&self) -> &'static str;
}
