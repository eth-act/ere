use crate::error::CompileError;
use build_utils::docker;
use risc0_zkvm::Digest;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};
use tempfile::TempDir;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risc0Program {
    // TODO: Seems like the risc0 compilation is also compiling
    // TODO: the analogous prover and verifying key
    pub(crate) elf: Vec<u8>,
    pub(crate) image_id: Digest,
}

pub fn compile_risc0_program(
    workspace_directory: &Path,
    guest_program_relative: &Path,
) -> Result<Risc0Program, CompileError> {
    // Build the SP1 docker image
    let tag = "ere-risc0-cli:latest";
    docker::build_image(&PathBuf::from("docker/risc0/Dockerfile"), tag)
        .map_err(|e| CompileError::DockerImageBuildFailed(Box::new(e)))?;

    // Prepare paths for compilation
    let mount_directory_str = workspace_directory
        .to_str()
        .ok_or_else(|| CompileError::InvalidMountPath(workspace_directory.to_path_buf()))?;

    let elf_output_dir = TempDir::new().map_err(CompileError::CreatingTempOutputDirectoryFailed)?;
    let elf_output_dir_str = elf_output_dir
        .path()
        .to_str()
        .ok_or_else(|| CompileError::InvalidTempOutputPath(elf_output_dir.path().to_path_buf()))?;

    let container_mount_directory = PathBuf::from_str("/guest-workspace").unwrap();
    let container_guest_program_path = container_mount_directory.join(guest_program_relative);
    let container_guest_program_str = container_guest_program_path
        .to_str()
        .ok_or_else(|| CompileError::InvalidGuestPath(guest_program_relative.to_path_buf()))?;

    info!(
        "Compiling program: mount_directory={} guest_program={}",
        mount_directory_str, container_guest_program_str
    );

    // Build and run Docker command
    let docker_cmd = docker::DockerRunCommand::new(tag)
        .remove_after_run()
        // Needed by `cargo risczero build` which uses docker in docker.
        .with_volume("/var/run/docker.sock", "/var/run/docker.sock")
        .with_volume(mount_directory_str, "/guest-workspace")
        .with_volume(elf_output_dir_str, "/output")
        .with_command(["compile", container_guest_program_str, "/output"]);

    let status = docker_cmd
        .run()
        .map_err(CompileError::DockerCommandFailed)?;

    if !status.success() {
        return Err(CompileError::DockerContainerRunFailed(status));
    }

    // Read the compiled ELF program from the output directory
    let elf = std::fs::read(elf_output_dir.path().join("guest.elf"))
        .map_err(CompileError::ReadCompiledELFProgram)?;
    let image_id = std::fs::read(elf_output_dir.path().join("image_id"))
        .and_then(|image_id| {
            Digest::try_from(image_id)
                .map_err(|image_id| format!("Invalid image id: {image_id:?}"))
                .map_err(std::io::Error::other)
        })
        .map_err(CompileError::ReadImageId)?;

    Ok(Risc0Program { elf, image_id })
}

#[cfg(test)]
mod tests {
    mod compile {
        use crate::compile::compile_risc0_program;
        use std::path::{Path, PathBuf};

        fn get_test_risc0_methods_crate_path() -> PathBuf {
            let workspace_dir = env!("CARGO_WORKSPACE_DIR");
            PathBuf::from(workspace_dir)
                .join("tests")
                .join("risc0")
                .join("compile")
                .join("basic")
                .canonicalize()
                .expect("Failed to find or canonicalize test Risc0 methods crate")
        }

        #[test]
        fn test_compile_risc0_method() {
            let test_methods_path = get_test_risc0_methods_crate_path();

            let program = compile_risc0_program(&test_methods_path, Path::new(""))
                .expect("risc0 compilation failed");
            assert!(
                !program.elf.is_empty(),
                "Risc0 ELF bytes should not be empty."
            );
        }
    }
}
