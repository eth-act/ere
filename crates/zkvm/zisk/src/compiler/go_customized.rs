use std::{env, fs, path::Path, process::Command};

use crate::compiler::Error;
use ere_compile_utils::CommonError;
use ere_zkvm_interface::{Compiler, Elf};
use tempfile::tempdir;
use tracing::info;

pub struct GoCustomized;

impl Compiler for GoCustomized {
    type Error = Error;

    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error> {
        let guest_directory = guest_directory.as_ref();
        info!(
            "Compiling TamaGo ZisK program at {}",
            guest_directory.display()
        );

        let home_dir = env::var("HOME")
            .map(std::path::PathBuf::from)
            .map_err(|var_error| CommonError::env_var_error("HOME".to_string(), var_error))?;

        let ldflags = ["-ldflags", "-T 0x80001000 -D 0xa0020000"];
        let tags = [
            "-tags",
            "tamago,linkcpuinit,linkramstart,linkramsize,linkprintk,tinygo.wasm,tinygo,riscv64",
        ];

        let tempdir = tempdir().map_err(CommonError::tempdir)?;
        let executable = tempdir.path().join("program.elf");

        let mut cmd = Command::new(home_dir.join(".tamago").join("bin").join("go"));
        let status = cmd
            .current_dir(guest_directory)
            .env("CGO_ENABLED", "0")
            .env("GOROOT", home_dir.join(".tamago").as_os_str())
            .env("GOOS", "tamago")
            .env("GOARCH", "riscv64")
            .arg("build")
            .arg("-buildvcs=false")
            .args(ldflags)
            .args(tags)
            .args(["-o", executable.to_str().unwrap()])
            .arg(".")
            .status()
            .map_err(|err| CommonError::command(&cmd, err))?;

        if !status.success() {
            return Err(CommonError::command_exit_non_zero(&cmd, status, None))?;
        }

        let elf =
            fs::read(&executable).map_err(|err| CommonError::read_file("elf", executable, err))?;

        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use crate::{EreZisk, compiler::GoCustomized};
    use ere_test_utils::{
        host::{run_zkvm_execute, testing_guest_directory},
        io::serde::cbor::Cbor,
        program::basic::BasicProgram,
    };
    use ere_zkvm_interface::{ProverResource, compiler::Compiler};

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("zisk", "basic_go");
        let elf = GoCustomized.compile(guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("zisk", "basic_go");
        let elf = GoCustomized.compile(guest_directory).unwrap();
        let zkvm = EreZisk::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<Cbor>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }
}
