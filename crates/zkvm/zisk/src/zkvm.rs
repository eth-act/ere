use crate::{
    program::ZiskProgram,
    zkvm::sdk::{RomDigest, ZiskOptions, ZiskSdk, ZiskServer},
};
use anyhow::bail;
use ere_zkvm_interface::zkvm::{
    CommonError, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMProgramDigest,
};
use std::{
    sync::{Mutex, MutexGuard},
    time::Instant,
};

mod error;
mod sdk;

pub use error::Error;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub struct EreZisk {
    sdk: ZiskSdk,
    /// Use `Mutex` because the server can only handle signle proving task at a
    /// time.
    ///
    /// Use `Option` inside to lazily initialize only when `prove` is called.
    server: Mutex<Option<ZiskServer>>,
}

impl EreZisk {
    pub fn new(program: ZiskProgram, resource: ProverResourceType) -> Result<Self, Error> {
        if matches!(resource, ProverResourceType::Network(_)) {
            panic!("Network proving not yet implemented for ZisK. Use CPU or GPU resource type.");
        }
        let sdk = ZiskSdk::new(program.elf, resource, ZiskOptions::from_env())?;
        Ok(Self {
            sdk,
            server: Mutex::new(None),
        })
    }

    fn server(&'_ self) -> Result<MutexGuard<'_, Option<ZiskServer>>, Error> {
        let mut server = self.server.lock().map_err(|_| Error::MutexPoisoned)?;

        match &mut *server {
            // Recreate the server if it has been created but failed to get status.
            Some(s) => {
                if s.status().is_err() {
                    *server = Some(self.sdk.server()?);
                }
            }
            // Create the server if it has not been created.
            None => {
                *server = Some(self.sdk.server()?);
            }
        }

        // FIXME: Use `MutexGuard::map` to unwrap the inner `Option` when it's stabilized.
        Ok(server)
    }
}

impl zkVM for EreZisk {
    fn execute(&self, input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let start = Instant::now();
        let (public_values, total_num_cycles) = self.sdk.execute(input)?;
        let execution_duration = start.elapsed();

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles,
                execution_duration,
                ..Default::default()
            },
        ))
    }

    fn prove(
        &self,
        input: &[u8],
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        if proof_kind != ProofKind::Compressed {
            bail!(CommonError::unsupported_proof_kind(
                proof_kind,
                [ProofKind::Compressed]
            ))
        }

        let mut server = self.server()?;
        let server = server.as_mut().expect("server initialized");

        let start = Instant::now();
        let (public_values, proof) = server.prove(input)?;
        let proving_time = start.elapsed();

        Ok((
            public_values,
            Proof::Compressed(proof),
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        let Proof::Compressed(proof) = proof else {
            bail!(CommonError::unsupported_proof_kind(
                proof.kind(),
                [ProofKind::Compressed]
            ))
        };

        Ok(self.sdk.verify(proof)?)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

impl zkVMProgramDigest for EreZisk {
    type ProgramDigest = RomDigest;

    fn program_digest(&self) -> anyhow::Result<Self::ProgramDigest> {
        Ok(self.sdk.rom_digest()?)
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv64imaCustomized, program::ZiskProgram, zkvm::EreZisk};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgramInput,
    };
    use ere_zkvm_interface::{
        compiler::Compiler,
        zkvm::{ProofKind, ProverResourceType, zkVM},
    };
    use std::sync::{Mutex, OnceLock};

    /// It fails if multiple servers created concurrently using the same port,
    /// so we have a lock to avoid that.
    static PROVE_LOCK: Mutex<()> = Mutex::new(());

    fn basic_program() -> ZiskProgram {
        static PROGRAM: OnceLock<ZiskProgram> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                RustRv64imaCustomized
                    .compile(&testing_guest_directory("zisk", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgramInput::valid().into_output_sha256();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_input() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        let _guard = PROVE_LOCK.lock().unwrap();

        let test_case = BasicProgramInput::valid().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_input() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        let _guard = PROVE_LOCK.lock().unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }
}
