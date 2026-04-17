use std::{env, fs, path::Path, process::Command};

use crate::Error;
use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::CommonError;
use tempfile::tempdir;
use tracing::info;

pub struct ZiskGoCustomized;

impl Compiler for ZiskGoCustomized {
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
    use crate::ZiskGoCustomized;
    use ere_compiler_core::Compiler;
    use ere_util_test::host::testing_guest_directory;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("zisk", "basic_go");
        let elf = ZiskGoCustomized.compile(guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}
