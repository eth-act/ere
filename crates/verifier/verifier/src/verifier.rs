use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

use ere_catalog::zkVMKind;
use ere_verifier_core::{PublicValues, codec::Decode, zkVMVerifier};

use crate::error::Error;

/// Verifier of one zkVM kind, bound to one program verifying key.
///
/// [`Verifier::new`] and [`Verifier::verify`] return a panic of the verifier code as
/// [`Error::Panic`]. A `Verifier` holds only values that `new` derives from the program verifying
/// key, and `verify` only reads them, so a caught panic leaves it usable.
///
/// The SP1 and OpenVM verifiers keep a process-wide key that initializes on first use. If that
/// initialization panics, every later call of that kind returns [`Error::Panic`].
#[derive(Debug)]
pub enum Verifier {
    OpenVM(Box<ere_verifier_openvm::OpenVMVerifier>),
    SP1(ere_verifier_sp1::SP1Verifier),
    Zisk(ere_verifier_zisk::ZiskVerifier),
    LambdaVM(ere_verifier_lambdavm::LambdaVMVerifier),
    Pico(ere_verifier_pico::PicoVerifier),
}

impl Verifier {
    /// Decodes `encoded_program_vk` as a program verifying key of `zkvm_kind` and constructs a
    /// verifier for it.
    pub fn new(zkvm_kind: zkVMKind, encoded_program_vk: &[u8]) -> Result<Self, Error> {
        catch_panic(|| Self::new_unguarded(zkvm_kind, encoded_program_vk))
    }

    fn new_unguarded(zkvm_kind: zkVMKind, encoded_program_vk: &[u8]) -> Result<Self, Error> {
        Ok(match zkvm_kind {
            zkVMKind::OpenVM => {
                let program_vk = Decode::decode_from_slice(encoded_program_vk)
                    .map_err(Error::decode_program_vk)?;
                Self::OpenVM(Box::new(ere_verifier_openvm::OpenVMVerifier::new(
                    program_vk,
                )))
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
            zkVMKind::LambdaVM => {
                let program_vk = Decode::decode_from_slice(encoded_program_vk)
                    .map_err(Error::decode_program_vk)?;
                Self::LambdaVM(ere_verifier_lambdavm::LambdaVMVerifier::new(program_vk))
            }
            zkVMKind::Pico => {
                let program_vk = Decode::decode_from_slice(encoded_program_vk)
                    .map_err(Error::decode_program_vk)?;
                Self::Pico(
                    ere_verifier_pico::PicoVerifier::new(program_vk)
                        .map_err(Error::decode_program_vk)?,
                )
            }
        })
    }

    pub fn zkvm_kind(&self) -> zkVMKind {
        match self {
            Self::OpenVM(_) => zkVMKind::OpenVM,
            Self::SP1(_) => zkVMKind::SP1,
            Self::Zisk(_) => zkVMKind::Zisk,
            Self::LambdaVM(_) => zkVMKind::LambdaVM,
            Self::Pico(_) => zkVMKind::Pico,
        }
    }

    /// Decodes and verifies `encoded_proof`, then returns the public values of the proof.
    ///
    /// A proof that verifies only shows that the program ran. The caller compares the returned
    /// public values with the expected bytes and treats a difference as a rejected proof.
    pub fn verify(&self, encoded_proof: &[u8]) -> Result<PublicValues, Error> {
        catch_panic(|| self.verify_inner(encoded_proof))
    }

    fn verify_inner(&self, encoded_proof: &[u8]) -> Result<PublicValues, Error> {
        Ok(match self {
            Self::OpenVM(verifier) => {
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
            Self::LambdaVM(verifier) => {
                let proof =
                    Decode::decode_from_slice(encoded_proof).map_err(Error::decode_proof)?;
                verifier.verify(&proof).map_err(Error::verification)?
            }
            Self::Pico(verifier) => {
                let proof =
                    Decode::decode_from_slice(encoded_proof).map_err(Error::decode_proof)?;
                verifier.verify(&proof).map_err(Error::verification)?
            }
        })
    }
}

/// Runs `body` and returns a panic of it as [`Error::Panic`].
fn catch_panic<T>(body: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
    // The body only reads a `Verifier` and its input bytes, so a panic leaves no state half
    // changed.
    catch_unwind(AssertUnwindSafe(body))
        .unwrap_or_else(|payload| Err(Error::Panic(panic_message(&*payload))))
}

/// Returns the message of a panic payload.
fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| message.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic payload is not a string".to_string())
}
