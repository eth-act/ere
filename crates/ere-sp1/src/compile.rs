use std::{
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use build_utils::docker;
use tempfile::TempDir;
use tracing::info;

use crate::error::CompileError;

pub fn compile(
    workspace_directory: &Path,
    guest_program_relative: &Path,
) -> Result<Vec<u8>, CompileError> {
    // Build the SP1 docker image
    let tag = "ere-build-sp1:latest";
    docker::build_image(&PathBuf::from("docker/sp1/Dockerfile"), tag)
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
    let docker_cmd = DockerRunCommand::new(tag)
        .remove_after_run()
        .with_volume(mount_directory_str, "/guest-workspace")
        .with_volume(elf_output_dir_str, "/output")
        .with_command(["./guest-compiler", container_guest_program_str, "/output"]);

    let status = docker_cmd
        .run()
        .map_err(CompileError::DockerCommandFailed)?;

    if !status.success() {
        return Err(CompileError::DockerContainerRunFailed(status));
    }

    // Read the compiled ELF program from the output directory
    let elf = std::fs::read(elf_output_dir.path().join("guest.elf"))
        .map_err(CompileError::ReadCompiledELFProgram)?;

    Ok(elf)
}

#[cfg(test)]
mod tests {
    use zkvm_interface::Compiler;

    use crate::RV32_IM_SUCCINCT_ZKVM_ELF;

    use super::*;
    use std::path::PathBuf;

    // TODO: for now, we just get one test file
    // TODO: but this should get the whole directory and compile each test
    fn get_compile_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("sp1")
            .join("compile")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/compile/sp1")
    }

    #[test]
    fn test_compile_sp1_program() {
        let test_guest_path = get_compile_test_guest_program_path();

        match compile(&test_guest_path, Path::new("")) {
            Ok(elf_bytes) => {
                assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
            }
            Err(err) => {
                panic!("compile failed for dedicated guest: {err}");
            }
        }
    }

    #[test]
    fn test_compile_trait() {
        let test_guest_path = get_compile_test_guest_program_path();
        match RV32_IM_SUCCINCT_ZKVM_ELF::compile(&test_guest_path, Path::new("")) {
            Ok(elf_bytes) => {
                assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
            }
            Err(err) => {
                panic!("compile_sp1_program direct call failed for dedicated guest: {err}");
            }
        }
    }
}

#[derive(Debug)]
struct DockerRunCommand {
    image: String,
    volumes: Vec<(String, String)>, // (host_path, container_path)
    command: Vec<String>,
    // remove image after running
    remove_after: bool,
}

impl DockerRunCommand {
    fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
            volumes: Vec::new(),
            command: Vec::new(),
            remove_after: false,
        }
    }

    fn with_volume(
        mut self,
        host_path: impl Into<String>,
        container_path: impl Into<String>,
    ) -> Self {
        self.volumes.push((host_path.into(), container_path.into()));
        self
    }

    fn with_command(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.command.extend(args.into_iter().map(|s| s.into()));
        self
    }

    fn remove_after_run(mut self) -> Self {
        self.remove_after = true;
        self
    }

    fn to_args(&self) -> Vec<String> {
        let mut args = vec!["run".to_string()];

        if self.remove_after {
            args.push("--rm".to_string());
        }

        for (host_path, container_path) in &self.volumes {
            args.extend(["-v".to_string(), format!("{host_path}:{container_path}")]);
        }

        args.push(self.image.clone());
        args.extend(self.command.iter().cloned());

        args
    }

    fn run(&self) -> Result<std::process::ExitStatus, std::io::Error> {
        Command::new("docker").args(self.to_args()).status()
    }
}
