use ere_catalog::zkVMKind;
use ere_verifier_core::{PublicValues, codec::Decode, zkVMVerifier};

use crate::error::Error;

#[derive(Debug)]
pub enum Verifier {
    #[cfg(feature = "nightly")]
    Airbender(ere_verifier_airbender::AirbenderVerifier),
    OpenVM(ere_verifier_openvm::OpenVMVerifier),
    Risc0(ere_verifier_risc0::Risc0Verifier),
    SP1(ere_verifier_sp1::SP1Verifier),
    Zisk(ere_verifier_zisk::ZiskVerifier),
}

impl Verifier {
    pub fn new(zkvm_kind: zkVMKind, encoded_program_vk: &[u8]) -> Result<Self, Error> {
        Ok(match zkvm_kind {
            #[cfg(not(feature = "nightly"))]
            zkVMKind::Airbender => return Err(Error::NightlyFeatureRequired),
            #[cfg(feature = "nightly")]
            zkVMKind::Airbender => {
                let program_vk = Decode::decode_from_slice(encoded_program_vk)
                    .map_err(Error::decode_program_vk)?;
                Self::Airbender(ere_verifier_airbender::AirbenderVerifier::new(program_vk))
            }
            zkVMKind::OpenVM => {
                let program_vk = Decode::decode_from_slice(encoded_program_vk)
                    .map_err(Error::decode_program_vk)?;
                Self::OpenVM(ere_verifier_openvm::OpenVMVerifier::new(program_vk))
            }
            zkVMKind::Risc0 => {
                let program_vk = Decode::decode_from_slice(encoded_program_vk)
                    .map_err(Error::decode_program_vk)?;
                Self::Risc0(ere_verifier_risc0::Risc0Verifier::new(program_vk))
            }
            zkVMKind::SP1 => {
                let program_vk = Decode::decode_from_slice(encoded_program_vk)
                    .map_err(Error::decode_program_vk)?;
                Self::SP1(ere_verifier_sp1::SP1Verifier::new(program_vk))
            }
            zkVMKind::Zisk => {
                let program_vk = Decode::decode_from_slice(encoded_program_vk)
                    .map_err(Error::decode_program_vk)?;
                Self::Zisk(ere_verifier_zisk::ZiskVerifier::new(program_vk))
            }
        })
    }

    pub fn zkvm_kind(&self) -> zkVMKind {
        match self {
            #[cfg(feature = "nightly")]
            Self::Airbender(_) => zkVMKind::Airbender,
            Self::OpenVM(_) => zkVMKind::OpenVM,
            Self::Risc0(_) => zkVMKind::Risc0,
            Self::SP1(_) => zkVMKind::SP1,
            Self::Zisk(_) => zkVMKind::Zisk,
        }
    }

    pub fn verify(&self, encoded_proof: &[u8]) -> Result<PublicValues, Error> {
        Ok(match self {
            #[cfg(feature = "nightly")]
            Self::Airbender(verifier) => {
                let proof =
                    Decode::decode_from_slice(encoded_proof).map_err(Error::decode_proof)?;
                verifier.verify(&proof).map_err(Error::verification)?
            }
            Self::OpenVM(verifier) => {
                let proof =
                    Decode::decode_from_slice(encoded_proof).map_err(Error::decode_proof)?;
                verifier.verify(&proof).map_err(Error::verification)?
            }
            Self::Risc0(verifier) => {
                let proof =
                    Decode::decode_from_slice(encoded_proof).map_err(Error::decode_proof)?;
                verifier.verify(&proof).map_err(Error::verification)?
            }
            Self::SP1(verifier) => {
                let proof =
                    Decode::decode_from_slice(encoded_proof).map_err(Error::decode_proof)?;
                verifier.verify(&proof).map_err(Error::verification)?
            }
            Self::Zisk(verifier) => {
                let proof =
                    Decode::decode_from_slice(encoded_proof).map_err(Error::decode_proof)?;
                verifier.verify(&proof).map_err(Error::verification)?
            }
        })
    }
}
