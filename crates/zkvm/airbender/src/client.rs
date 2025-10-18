use crate::error::AirbenderError;
use airbender_execution_utils::{
    Machine, ProgramProof, compute_chain_encoding, generate_params_for_binary,
    universal_circuit_verifier_vk, verify_recursion_log_23_layer,
};
use ere_zkvm_interface::PublicValues;
use std::{array, fs, io::BufRead, iter, process::Command};
use tempfile::tempdir;

pub type VerificationKey = [u32; 8];

pub struct AirbenderSdk {
    app_bin: Vec<u8>,
    final_vk: VerificationKey,
    gpu: bool,
}

impl AirbenderSdk {
    pub fn new(bin: &[u8], gpu: bool) -> Self {
        let final_vk = compute_chain_encoding(vec![
            [0u32; 8],
            generate_params_for_binary(bin, Machine::Standard),
            universal_circuit_verifier_vk().params,
        ]);
        Self {
            app_bin: bin.to_vec(),
            final_vk,
            gpu,
        }
    }

    pub fn execute(&self, input: &[u8]) -> Result<(PublicValues, u64), AirbenderError> {
        let tempdir = tempdir().map_err(AirbenderError::TempDir)?;

        let bin_path = tempdir.path().join("guest.bin");
        fs::write(&bin_path, &self.app_bin)
            .map_err(|err| AirbenderError::write_file(err, "guest.bin", &bin_path))?;

        let input_path = tempdir.path().join("input.hex");
        fs::write(&input_path, encode_input(input))
            .map_err(|err| AirbenderError::write_file(err, "input.hex", &input_path))?;

        let output = Command::new("airbender-cli")
            .arg("run")
            .arg("--bin")
            .arg(&bin_path)
            .arg("--input-file")
            .arg(&input_path)
            .args(["--cycles", &u64::MAX.to_string()])
            .output()
            .map_err(AirbenderError::AirbenderRun)?;

        if !output.status.success() {
            return Err(AirbenderError::AirbenderRunFailed {
                status: output.status,
            });
        }

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
                AirbenderError::ParsePublicValue(
                    String::from_utf8_lossy(&output.stdout).to_string(),
                )
            })?;

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
                AirbenderError::ParseCycles(String::from_utf8_lossy(&output.stdout).to_string())
            })?;

        Ok((public_values, cycles))
    }

    pub fn prove(&self, input: &[u8]) -> Result<(PublicValues, ProgramProof), AirbenderError> {
        let tempdir = tempdir().map_err(AirbenderError::TempDir)?;

        let bin_path = tempdir.path().join("guest.bin");
        fs::write(&bin_path, &self.app_bin)
            .map_err(|err| AirbenderError::write_file(err, "guest.bin", &bin_path))?;

        let input_path = tempdir.path().join("input.hex");
        fs::write(&input_path, encode_input(input))
            .map_err(|err| AirbenderError::write_file(err, "input.hex", &input_path))?;

        let output_dir = tempdir.path().join("output");
        fs::create_dir_all(&output_dir)
            .map_err(|err| AirbenderError::create_dir(err, "output", &output_dir))?;

        // Prove until `final-recursion`
        let output = Command::new("airbender-cli")
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
            .map_err(AirbenderError::AirbenderProve)?;

        if !output.status.success() {
            return Err(AirbenderError::AirbenderProveFailed {
                status: output.status,
            });
        }

        let recursion_proof_path = output_dir.join("recursion_program_proof.json");
        if !recursion_proof_path.exists() {
            return Err(AirbenderError::RecursionProofNotFound {
                path: recursion_proof_path,
            });
        }

        // Prove final proof
        let output = Command::new("airbender-cli")
            .arg("prove-final")
            .arg("--input-file")
            .arg(&recursion_proof_path)
            .arg("--output-dir")
            .arg(&output_dir)
            .args(self.gpu.then_some("--gpu"))
            .output()
            .map_err(AirbenderError::AirbenderProveFinal)?;

        if !output.status.success() {
            return Err(AirbenderError::AirbenderProveFinalFailed {
                status: output.status,
            });
        }

        let final_proof_path = output_dir.join("final_program_proof.json");
        if !final_proof_path.exists() {
            return Err(AirbenderError::FinalProofNotFound {
                path: final_proof_path,
            });
        }

        let final_proof_bytes = fs::read(&final_proof_path).map_err(|err| {
            AirbenderError::read_file(err, "final_program_proof.json", &final_proof_path)
        })?;

        let final_proof: ProgramProof =
            serde_json::from_slice(&final_proof_bytes).map_err(AirbenderError::JsonDeserialize)?;

        let (public_values, final_vk) = extract_public_values_and_final_vk(&final_proof)?;

        if self.final_vk != final_vk {
            return Err(AirbenderError::UnexpectedVerificationKey {
                preprocessed: self.final_vk,
                proved: final_vk,
            });
        }

        Ok((public_values, final_proof))
    }

    pub fn verify(&self, proof: &ProgramProof) -> Result<PublicValues, AirbenderError> {
        let is_valid = verify_recursion_log_23_layer(proof);
        if !is_valid {
            return Err(AirbenderError::ProofVerificationFailed);
        }

        let (public_values, final_vk) = extract_public_values_and_final_vk(proof)?;

        if self.final_vk != final_vk {
            return Err(AirbenderError::UnexpectedVerificationKey {
                preprocessed: self.final_vk,
                proved: final_vk,
            });
        }

        Ok(public_values)
    }
}

/// Encode input with length prefixed to hex string for `airbender-cli`.
fn encode_input(input: &[u8]) -> String {
    iter::once((input.len() as u32).to_le_bytes().as_slice())
        .chain(input.chunks(4))
        .map(|chunk| {
            let mut bytes = [0u8; 4];
            bytes[..chunk.len()].copy_from_slice(chunk);
            format!("{:08x}", u32::from_le_bytes(bytes))
        })
        .collect()
}

// Extract public values from register values.
fn extract_public_values_and_final_vk(
    proof: &ProgramProof,
) -> Result<(PublicValues, VerificationKey), AirbenderError> {
    if proof.register_final_values.len() != 32 {
        return Err(AirbenderError::InvalidRegisterCount(
            proof.register_final_values.len(),
        ));
    }

    let public_values = proof.register_final_values[10..18]
        .iter()
        .flat_map(|value| value.value.to_le_bytes())
        .collect();

    let vk = array::from_fn(|i| proof.register_final_values[18 + i].value);

    Ok((public_values, vk))
}
