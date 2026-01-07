use crate::{
    program::ZiskProgram,
    zkvm::sdk::{RomDigest, START_SERVER_TIMEOUT, ZiskOptions, ZiskSdk, ZiskServer},
};
use anyhow::bail;
use ere_zkvm_interface::zkvm::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMProgramDigest,
};
use std::{
    sync::{Mutex, MutexGuard},
    time::Instant,
};
use tracing::error;

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
        let sdk = ZiskSdk::new(program.elf, resource, ZiskOptions::from_env())?;
        Ok(Self {
            sdk,
            server: Mutex::new(None),
        })
    }

    fn server(&'_ self) -> Result<MutexGuard<'_, Option<ZiskServer>>, Error> {
        let mut server = self.server.lock().map_err(|_| Error::MutexPoisoned)?;

        if server
            .as_ref()
            .is_none_or(|server| server.status(START_SERVER_TIMEOUT).is_err())
        {
            const MAX_RETRY: usize = 3;
            let mut retry = 0;
            *server = loop {
                drop(server.take());
                match self.sdk.server() {
                    Ok(server) => break Some(server),
                    Err(Error::TimeoutWaitingServerReady) if retry < MAX_RETRY => {
                        error!("Timeout waiting server ready, restarting...");
                        retry += 1;
                        continue;
                    }
                    Err(err) => return Err(err),
                }
            }
        }

        // FIXME: Use `MutexGuard::map` to unwrap the inner `Option` when it's stabilized.
        Ok(server)
    }
}

impl zkVM for EreZisk {
    fn execute(&self, input: &Input) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        if input.proofs.is_some() {
            bail!(CommonError::unsupported_input("no dedicated proofs stream"))
        }

        let start = Instant::now();
        let (public_values, total_num_cycles) = self.sdk.execute(input.stdin())?;
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
        input: &Input,
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        if input.proofs.is_some() {
            bail!(CommonError::unsupported_input("no dedicated proofs stream"))
        }
        if proof_kind != ProofKind::Compressed {
            bail!(CommonError::unsupported_proof_kind(
                proof_kind,
                [ProofKind::Compressed]
            ))
        }

        let (public_values, proof, proving_time) =
            if let ProverResourceType::Network(_) = self.sdk.resource() {
                self.sdk.network_prove(input.stdin())?
            } else {
                let mut server = self.server()?;
                let server = server.as_mut().expect("server initialized");

                let start = Instant::now();
                let (public_values, proof) = server.prove(input.stdin())?;
                let proving_time = start.elapsed();
                (public_values, proof, proving_time)
            };

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
        io::serde::bincode::BincodeLegacy,
        program::basic::BasicProgram,
    };
    use ere_zkvm_interface::{
        NetworkProverConfig,
        compiler::Compiler,
        zkvm::{Input, ProofKind, ProverResourceType, zkVM},
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
                    .compile(&testing_guest_directory("zisk", "basic_rust"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        let _guard = PROVE_LOCK.lock().unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        let _guard = PROVE_LOCK.lock().unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }

    #[test]
    #[ignore = "Requires ZisK cluster running"]
    fn test_network_prove() {
        let program = basic_program();
        let zkvm = EreZisk::new(
            program,
            ProverResourceType::Network(NetworkProverConfig {
                endpoint: "http://127.0.0.1:50051".to_string(),
                ..Default::default()
            }),
        )
        .unwrap();

        let _guard = PROVE_LOCK.lock().unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }
}
