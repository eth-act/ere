use crate::zkvm::error::Error;
use airbender_execution_utils::{
    Machine, ProgramProof, compute_chain_encoding, generate_params_for_binary,
    universal_circuit_verifier_vk,
};
use ere_verifier_airbender::{AirbenderProgramVk, extract_public_values_and_program_vk};
use ere_zkvm_interface::zkvm::{CommonError, PublicValues};
use std::{
    fs,
    io::BufRead,
    process::{Command, Stdio},
};
use tempfile::tempdir;

pub struct AirbenderSdk {
    bin: Vec<u8>,
    program_vk: AirbenderProgramVk,
    gpu: bool,
}

impl AirbenderSdk {
    pub fn new(elf: &[u8], gpu: bool) -> Result<Self, Error> {
        let bin = objcopy_elf_to_bin(elf)?;
        let program_vk = {
            // Compute base VK as `blake(PC || setup_caps)`.
            let base_vk = generate_params_for_binary(&bin, Machine::Standard);
            // The 1st recursion layer VK
            let verifier_vk = universal_circuit_verifier_vk().params;
            // Compute hash chain as `blake(blake(0 || guest_vk) || verifier_vk)`,
            // that is expected to be exposed by second layer recursion program.
            AirbenderProgramVk(compute_chain_encoding(vec![[0; 8], base_vk, verifier_vk]))
        };
        Ok(Self {
            bin,
            program_vk,
            gpu,
        })
    }

    pub fn program_vk(&self) -> &AirbenderProgramVk {
        &self.program_vk
    }

    pub fn execute(&self, input: &[u8]) -> Result<(PublicValues, u64), Error> {
        let tempdir = tempdir().map_err(CommonError::tempdir)?;

        let bin_path = tempdir.path().join("guest.bin");
        fs::write(&bin_path, &self.bin)
            .map_err(|err| CommonError::write_file("guest.bin", &bin_path, err))?;

        let input_path = tempdir.path().join("input.hex");
        fs::write(&input_path, encode_input(input))
            .map_err(|err| CommonError::write_file("input.hex", &input_path, err))?;

        let mut cmd = Command::new("airbender-cli");
        let output = cmd
            .arg("run")
            .arg("--bin")
            .arg(&bin_path)
            .arg("--input-file")
            .arg(&input_path)
            .args(["--cycles", &u64::MAX.to_string()])
            .output()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !output.status.success() {
            Err(CommonError::command_exit_non_zero(
                &cmd,
                output.status,
                Some(&output),
            ))?
        }

        // Parse public values 8 u32 words (32 bytes) from stdout in format of:
        // `Result: {v0}, {v1}, {v2}, {v3}, {v4}, {v5}, {v6}, {v7}`
        let public_values = output
            .stdout
            .lines()
            .find_map(|line| {
                let line = line.ok()?;
                let line = line.split_once("Result:")?.1;
                let mut words = line.split(',');
                let mut bytes = Vec::with_capacity(32);
                for _ in 0..8 {
                    bytes.extend(words.next()?.trim().parse::<u32>().ok()?.to_le_bytes())
                }
                Some(bytes.into())
            })
            .ok_or_else(|| {
                Error::ParsePublicValue(String::from_utf8_lossy(&output.stdout).to_string())
            })?;

        // Parse cycles from stdout in format of:
        // `Took {cycles} cycles to finish`
        let cycles = output
            .stdout
            .lines()
            .find_map(|line| {
                let line = line.ok()?;
                let line = line.split_once("Took ")?.1;
                let cycle = line.split_once(" cycles")?.0;
                cycle.parse().ok()
            })
            .ok_or_else(|| {
                Error::ParseCycles(String::from_utf8_lossy(&output.stdout).to_string())
            })?;

        Ok((public_values, cycles))
    }

    pub fn prove(&self, input: &[u8]) -> Result<(PublicValues, ProgramProof), Error> {
        let tempdir = tempdir().map_err(CommonError::tempdir)?;

        let bin_path = tempdir.path().join("guest.bin");
        fs::write(&bin_path, &self.bin)
            .map_err(|err| CommonError::write_file("guest.bin", &bin_path, err))?;

        let input_path = tempdir.path().join("input.hex");
        fs::write(&input_path, encode_input(input))
            .map_err(|err| CommonError::write_file("input.hex", &input_path, err))?;

        let output_dir = tempdir.path().join("output");
        fs::create_dir_all(&output_dir)
            .map_err(|err| CommonError::create_dir("output", &output_dir, err))?;

        // Prove guest program + 1st recursion layer (tree of recursive proofs until root).
        let mut cmd = Command::new("airbender-cli");
        let output = cmd
            .arg("prove")
            .arg("--bin")
            .arg(&bin_path)
            .arg("--output-dir")
            .arg(&output_dir)
            .arg("--input-file")
            .arg(&input_path)
            .args(["--until", "final-recursion"])
            .args(["--cycles", &u64::MAX.to_string()])
            .args(self.gpu.then_some("--gpu"))
            .output()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !output.status.success() {
            Err(CommonError::command_exit_non_zero(
                &cmd,
                output.status,
                Some(&output),
            ))?
        }

        let proof_path = output_dir.join("recursion_program_proof.json");
        if !proof_path.exists() {
            Err(CommonError::file_not_found("proof", &proof_path))?
        }

        // Prove 2nd recursion layer (wrapping root of 1st recursion layer)
        let mut cmd = Command::new("airbender-cli");
        let output = cmd
            .arg("prove-final")
            .arg("--input-file")
            .arg(&proof_path)
            .arg("--output-dir")
            .arg(&output_dir)
            .args(self.gpu.then_some("--gpu"))
            .output()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !output.status.success() {
            Err(CommonError::command_exit_non_zero(
                &cmd,
                output.status,
                Some(&output),
            ))?
        }

        let proof_path = output_dir.join("final_program_proof.json");
        let proof_bytes = fs::read(&proof_path)
            .map_err(|err| CommonError::read_file("proof", &proof_path, err))?;

        let proof: ProgramProof = serde_json::from_slice(&proof_bytes)
            .map_err(|err| CommonError::deserialize("proof", "serde_json", err))?;

        let (public_values, program_vk) = extract_public_values_and_program_vk(&proof)?;

        if self.program_vk != program_vk {
            return Err(ere_verifier_airbender::Error::UnexpectedProgramVk {
                expected: self.program_vk,
                got: program_vk,
            }
            .into());
        }

        Ok((public_values, proof))
    }
}

fn objcopy_elf_to_bin(elf: &[u8]) -> Result<Vec<u8>, Error> {
    let dir = tempfile::tempdir().map_err(CommonError::tempdir)?;
    let input_path = dir.path().join("input.elf");
    let output_path = dir.path().join("output.bin");

    fs::write(&input_path, elf)
        .map_err(|err| CommonError::write_file("objcopy input ELF", &input_path, err))?;

    let mut cmd = Command::new("objcopy");
    let output = cmd
        .args(["-I", "elf32-little", "-O", "binary"])
        .arg(&input_path)
        .arg(&output_path)
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| CommonError::command(&cmd, err))?;

    if !output.status.success() {
        Err(CommonError::command_exit_non_zero(
            &cmd,
            output.status,
            Some(&output),
        ))?
    }

    Ok(fs::read(&output_path)
        .map_err(|err| CommonError::read_file("objcopy output binary", &output_path, err))?)
}

/// Encode input with length prefixed to hex string for `airbender-cli`.
fn encode_input(input: &[u8]) -> String {
    input
        .chunks(4)
        .map(|chunk| {
            let mut bytes = [0u8; 4];
            bytes[..chunk.len()].copy_from_slice(chunk);
            format!("{:08x}", u32::from_le_bytes(bytes))
        })
        .collect()
}
