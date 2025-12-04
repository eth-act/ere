use crate::zkvm::error::Error;
use airbender_execution_utils::{
    Machine, ProgramProof, compute_chain_encoding, generate_params_for_binary,
    universal_circuit_verifier_vk, verify_recursion_log_23_layer,
};
use ere_zkvm_interface::zkvm::{CommonError, PublicValues};
use std::{array, fs, io::BufRead, process::Command};
use tempfile::tempdir;

/// Verification key hash chain.
///
/// For recursive verifier program, it exposes the chaining hash of verification
/// keys of programs that it verifies, which is computed as
/// `blake(blake(blake(0 || base_vk)|| verifier_0_vk) || verifier_1_vk)...`.
///
/// For a base program, the VK is computed as `blake(PC || setup_caps)`, where
/// `PC` is the program counter value at the end of execution, and  `setup_caps`
/// is the merkle tree caps derived from the program.
pub type VkHashChain = [u32; 8];

pub struct AirbenderSdk {
    bin: Vec<u8>,
    vk_hash_chain: VkHashChain,
    gpu: bool,
}

impl AirbenderSdk {
    pub fn new(bin: &[u8], gpu: bool) -> Self {
        let vk_hash_chain = {
            // Compute base VK as `blake(PC || setup_caps)`.
            let base_vk = generate_params_for_binary(bin, Machine::Standard);
            // The 1st recursion layer VK
            let verifier_vk = universal_circuit_verifier_vk().params;
            // Compute hash chain as `blake(blake(0 || guest_vk) || verifier_vk)`,
            // that is expected to be exposed by second layer recursion program.
            compute_chain_encoding(vec![[0; 8], base_vk, verifier_vk])
        };
        Self {
            bin: bin.to_vec(),
            vk_hash_chain,
            gpu,
        }
    }

    pub fn vk_chain_hash(&self) -> &VkHashChain {
        &self.vk_hash_chain
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
                Some(bytes)
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

        let (public_values, vk_hash_chain) = extract_public_values_and_vk_hash_chain(&proof)?;

        if self.vk_hash_chain != vk_hash_chain {
            return Err(Error::UnexpectedVkHashChain {
                preprocessed: self.vk_hash_chain,
                proved: vk_hash_chain,
            });
        }

        Ok((public_values, proof))
    }

    pub fn verify(&self, proof: &ProgramProof) -> Result<PublicValues, Error> {
        let is_valid = verify_recursion_log_23_layer(proof);
        if !is_valid {
            return Err(Error::ProofVerificationFailed);
        }

        let (public_values, vk_hash_chain) = extract_public_values_and_vk_hash_chain(proof)?;

        if self.vk_hash_chain != vk_hash_chain {
            return Err(Error::UnexpectedVkHashChain {
                preprocessed: self.vk_hash_chain,
                proved: vk_hash_chain,
            });
        }

        Ok(public_values)
    }
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

// Extract public values and VK hash chain from register values.
fn extract_public_values_and_vk_hash_chain(
    proof: &ProgramProof,
) -> Result<(PublicValues, VkHashChain), Error> {
    if proof.register_final_values.len() != 32 {
        return Err(Error::InvalidRegisterCount(
            proof.register_final_values.len(),
        ));
    }

    let public_values = proof.register_final_values[10..18]
        .iter()
        .flat_map(|value| value.value.to_le_bytes())
        .collect();

    let vk_chain_hash = array::from_fn(|i| proof.register_final_values[18 + i].value);

    Ok((public_values, vk_chain_hash))
}
