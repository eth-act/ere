use std::{
    any::Any,
    env, fs,
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use airbender_execution_utils::{
    setups::{self, unrolled_circuits::get_unified_circuit_artifact_for_machine_type},
    unified_circuit::compute_unified_setup_for_machine_configuration,
    verifier_binaries::{RECURSION_UNIFIED_BIN, RECURSION_UNIFIED_TXT},
};
use airbender_host::{
    ExecutionResult, Runner, TranspilerRunner, TranspilerRunnerBuilder, UnifiedVk,
};
#[cfg(feature = "cuda")]
use airbender_host::{GpuProver, GpuProverBuilder, Proof, ProveResult, Prover as _};
use airbender_riscv_transpiler::cycle::IWithoutByteAccessIsaConfigWithDelegation;
use ere_compiler_core::Elf;
use ere_prover_core::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, ProverResource,
    ProverResourceKind, PublicValues, zkVMProver,
};
use ere_verifier_airbender::{
    AirbenderProgramVk, AirbenderProof, AirbenderVerifier, words_to_le_bytes,
};
use sha3::{Digest, Keccak256};
use tempfile::tempdir;

use crate::error::Error;

pub struct AirbenderProver {
    verifier: AirbenderVerifier,
    resource: ProverResource,
    runner: TranspilerRunner,
    #[cfg(feature = "cuda")]
    gpu_prover: Option<GpuProver>,
}

impl AirbenderProver {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        if !matches!(resource, ProverResource::Cpu | ProverResource::Gpu) {
            Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
            ))?;
        }

        let (bin_hash, bin_path) = elf_to_bin(&elf)?;

        let program_vk = compute_program_vk(bin_hash);
        let verifier = AirbenderVerifier::new(program_vk);

        let runner = TranspilerRunnerBuilder::new(&bin_path).build()?;

        #[cfg(feature = "cuda")]
        let gpu_prover = match resource {
            ProverResource::Gpu => Some(GpuProverBuilder::new(&bin_path).build()?),
            _ => None,
        };

        Ok(Self {
            verifier,
            runner,
            resource,
            #[cfg(feature = "cuda")]
            gpu_prover,
        })
    }
}

impl zkVMProver for AirbenderProver {
    type Verifier = AirbenderVerifier;
    type Error = Error;

    fn verifier(&self) -> &AirbenderVerifier {
        &self.verifier
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        if input.proofs.is_some() {
            return Err(CommonError::unsupported_input("no dedicated proofs stream"))?;
        }

        let input_words = input_to_words(input.stdin());

        let start = Instant::now();
        let ExecutionResult {
            receipt,
            cycles_executed,
            reached_end,
            ..
        } = panic::catch_unwind(AssertUnwindSafe(|| self.runner.run(&input_words)))
            .map_err(|err| Error::ExecutePanic(panic_msg(err)))??;
        let execution_duration = start.elapsed();

        if !reached_end {
            return Err(Error::ExecutionDidNotTerminate);
        }

        Ok((
            words_to_le_bytes(receipt.output).into(),
            ProgramExecutionReport {
                total_num_cycles: cycles_executed as u64,
                execution_duration,
                ..Default::default()
            },
        ))
    }

    #[cfg(not(feature = "cuda"))]
    fn prove(
        &self,
        _input: &Input,
    ) -> Result<(PublicValues, AirbenderProof, ProgramProvingReport), Error> {
        match self.resource {
            ProverResource::Cpu => Err(Error::CpuProverNotAvailable),
            ProverResource::Gpu => Err(Error::CudaFeatureDisabled),
            _ => Err(CommonError::unsupported_prover_resource_kind(
                self.resource.kind(),
                [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
            )
            .into()),
        }
    }

    #[cfg(feature = "cuda")]
    fn prove(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, AirbenderProof, ProgramProvingReport), Error> {
        if self.resource == ProverResource::Cpu {
            return Err(Error::CpuProverNotAvailable);
        }

        if input.proofs.is_some() {
            return Err(CommonError::unsupported_input("no dedicated proofs stream"))?;
        }

        let gpu_prover = self.gpu_prover.as_ref().unwrap();
        let input_words = input_to_words(input.stdin());

        // Pre-flight via the interpreter to avoid the gpu prover `panic_nounwind`.
        panic::catch_unwind(AssertUnwindSafe(|| self.runner.run(&input_words)))
            .map_err(|err| Error::ExecutePanic(panic_msg(err)))??;

        let start = Instant::now();
        let (proof, receipt) = match gpu_prover.prove(&input_words)? {
            ProveResult {
                proof: Proof::Real(proof),
                receipt,
                ..
            } if proof.level() == airbender_host::ProverLevel::RecursionUnified => {
                (proof.into_inner(), receipt)
            }
            _ => Err(Error::Sdk(airbender_host::HostError::Prover(
                "Expected Proof::Real in ProverLevel::RecursionUnified".to_string(),
            )))?,
        };
        let proving_time = start.elapsed();

        Ok((
            words_to_le_bytes(receipt.output).into(),
            AirbenderProof(proof),
            ProgramProvingReport::new(proving_time),
        ))
    }
}

fn compute_program_vk(app_bin_hash: [u8; 32]) -> AirbenderProgramVk {
    let (binary, binary_u32) = setups::pad_binary(RECURSION_UNIFIED_BIN.to_vec());
    let (text, _) = setups::pad_binary(RECURSION_UNIFIED_TXT.to_vec());
    let unified_setup = compute_unified_setup_for_machine_configuration::<
        IWithoutByteAccessIsaConfigWithDelegation,
    >(&binary, &text);
    let unified_layouts = get_unified_circuit_artifact_for_machine_type::<
        IWithoutByteAccessIsaConfigWithDelegation,
    >(&binary_u32);
    AirbenderProgramVk(UnifiedVk {
        app_bin_hash,
        unified_setup,
        unified_layouts,
    })
}

fn elf_to_bin(elf: &[u8]) -> Result<([u8; 32], PathBuf), Error> {
    let tempdir = tempdir().map_err(CommonError::tempdir)?;
    let elf_path = tempdir.path().join("app.elf");
    let bin_path = tempdir.path().join("app.bin");
    let text_path = tempdir.path().join("app.text");

    fs::write(&elf_path, elf).map_err(|err| CommonError::write_file("elf", &elf_path, err))?;
    objcopy(
        &elf_path,
        &bin_path,
        &["-I", "elf32-little", "-O", "binary"],
    )?;
    objcopy(
        &elf_path,
        &text_path,
        &["-I", "elf32-little", "-O", "binary", "--only-section=.text"],
    )?;

    let bin = fs::read(&bin_path).map_err(|err| CommonError::write_file("bin", &bin_path, err))?;
    let bin_hash: [u8; 32] = Keccak256::digest(&bin).into();

    let cache_dir = cache_dir();
    fs::create_dir_all(&cache_dir)
        .map_err(|err| CommonError::create_dir("cache", &cache_dir, err))?;

    let bin_hash_hex: String = bin_hash.iter().map(|b| format!("{b:02x}")).collect();
    let cache_bin_path = cache_dir.join(format!("{bin_hash_hex}.bin"));
    let cache_text_path = cache_dir.join(format!("{bin_hash_hex}.text"));
    if !cache_bin_path.exists() {
        fs::rename(&bin_path, &cache_bin_path).map_err(|err| CommonError::io("rename", err))?;
    }
    if !cache_text_path.exists() {
        fs::rename(&text_path, &cache_text_path).map_err(|err| CommonError::io("rename", err))?;
    }

    Ok((bin_hash, cache_bin_path))
}

fn objcopy(input: &Path, output: &Path, extra_args: &[&str]) -> Result<(), Error> {
    let mut cmd = Command::new("objcopy");
    let output = cmd
        .args(extra_args)
        .arg(input)
        .arg(output)
        .output()
        .map_err(|err| CommonError::command(&cmd, err))?;

    if !output.status.success() {
        Err(CommonError::command_exit_non_zero(
            &cmd,
            output.status,
            Some(&output),
        ))?
    }

    Ok(())
}

fn cache_dir() -> PathBuf {
    PathBuf::from(env::var("HOME").expect("env `$HOME` should be set"))
        .join(".airbender")
        .join("cache")
}

fn input_to_words(stdin: &[u8]) -> Vec<u32> {
    stdin
        .chunks(4)
        .map(|chunk| {
            let mut padded = [0u8; 4];
            padded[..chunk.len()].copy_from_slice(chunk);
            u32::from_le_bytes(padded)
        })
        .collect()
}

fn panic_msg(err: Box<dyn Any + Send + 'static>) -> String {
    err.downcast_ref::<String>()
        .cloned()
        .or_else(|| err.downcast_ref::<&'static str>().map(ToString::to_string))
        .unwrap_or_else(|| "unknown panic".to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use ere_compiler_airbender::AirbenderRustRv32imaCustomized;
    use ere_compiler_core::{Compiler, Elf};
    use ere_prover_core::{Input, ProverResource, codec::Encode, zkVMProver};
    #[cfg(feature = "cuda")]
    use ere_util_test::host::run_zkvm_prove;
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{TestCase, run_zkvm_execute, testing_guest_directory},
        program::basic::BasicProgram,
    };
    use ere_verifier_airbender::AirbenderProgramVk;

    use crate::prover::{AirbenderProver, compute_program_vk, elf_to_bin};

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            AirbenderRustRv32imaCustomized
                .compile(testing_guest_directory("airbender", "basic"))
                .unwrap()
        })
        .clone()
    }

    #[test]
    fn test_execute() {
        let elf = basic_elf();
        let zkvm = AirbenderProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = AirbenderProver::new(elf, ProverResource::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_gpu() {
        let elf = basic_elf();
        let zkvm = AirbenderProver::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let elf = basic_elf();
        let zkvm = AirbenderProver::new(elf, ProverResource::Gpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn compute_program_vk_matches_sdk() {
        let elf = basic_elf();
        let (app_bin_hash, app_bin_path) = elf_to_bin(&elf).unwrap();

        let program_vk =
            AirbenderProgramVk(airbender_host::compute_unified_vk(&app_bin_path).unwrap());

        assert_eq!(
            compute_program_vk(app_bin_hash).encode_to_vec().unwrap(),
            program_vk.encode_to_vec().unwrap(),
        );
    }
}
