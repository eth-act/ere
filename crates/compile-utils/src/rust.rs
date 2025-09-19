use crate::CompileError;
use cargo_metadata::{Metadata, MetadataCommand};
use std::{fs, iter, path::Path, process::Command};
use tempfile::tempdir;

const CARGO_ENCODED_RUSTFLAGS_SEPARATOR: &str = "\x1f";

/// A builder for configuring `cargo build` invocation.
#[derive(Clone)]
pub struct CargoBuildCmd {
    toolchain: String,
    profile: String,
    rustflags: Vec<String>,
    build_options: Vec<String>,
    linker_script: Option<String>,
}

impl Default for CargoBuildCmd {
    fn default() -> Self {
        Self {
            toolchain: "stable".into(),
            profile: "release".into(),
            rustflags: Default::default(),
            build_options: Default::default(),
            linker_script: Default::default(),
        }
    }
}

impl CargoBuildCmd {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toolchain to use.
    pub fn toolchain(mut self, toolchain: impl AsRef<str>) -> Self {
        self.toolchain = toolchain.as_ref().to_string();
        self
    }

    /// Profile to use.
    pub fn profile(mut self, profile: impl AsRef<str>) -> Self {
        self.profile = profile.as_ref().to_string();
        self
    }

    /// Environment variable `RUSTFLAGS`.
    pub fn rustflags(mut self, rustflags: &[impl AsRef<str>]) -> Self {
        self.rustflags = rustflags
            .iter()
            .map(|rustflag| rustflag.as_ref().to_string())
            .collect();
        self
    }

    /// Options after `cargo build`.
    pub fn build_options(mut self, build_options: &[impl AsRef<str>]) -> Self {
        self.build_options = build_options
            .iter()
            .map(|v| v.as_ref().to_string())
            .collect();
        self
    }

    /// Linker script to be saved into a file and pass to `RUSTFLAGS`.
    pub fn linker_script(mut self, linker_script: Option<impl AsRef<str>>) -> Self {
        self.linker_script = linker_script.map(|v| v.as_ref().to_string());
        self
    }

    /// Takes the path to the manifest directory and the target triple, then
    /// runs configured `cargo build` and returns built ELF.
    pub fn exec(
        &self,
        manifest_dir: impl AsRef<Path>,
        target: impl AsRef<str>,
    ) -> Result<Vec<u8>, CompileError> {
        let metadata = cargo_metadata(manifest_dir.as_ref())?;

        let package = metadata.root_package().unwrap();

        let tempdir = tempdir().map_err(CompileError::Tempdir)?;
        let linker_script_path = tempdir
            .path()
            .join("linker_script")
            .to_string_lossy()
            .to_string();
        if let Some(linker_script) = &self.linker_script {
            fs::write(&linker_script_path, linker_script.as_bytes())
                .map_err(CompileError::CreateLinkerScript)?;
        }

        let encoded_rustflags = iter::empty()
            .chain(self.rustflags.iter().cloned())
            .chain(
                self.linker_script
                    .as_ref()
                    .map(|_| ["-C".into(), format!("link-arg=-T{linker_script_path}")])
                    .into_iter()
                    .flatten(),
            )
            .collect::<Vec<_>>()
            .join(CARGO_ENCODED_RUSTFLAGS_SEPARATOR);

        let args = iter::empty()
            .chain([format!("+{}", &self.toolchain)])
            .chain(["build".into()])
            .chain(self.build_options.iter().cloned())
            .chain(["--profile".into(), self.profile.clone()])
            .chain(["--target".into(), target.as_ref().into()])
            .chain(["--manifest-path".into(), package.manifest_path.to_string()]);

        let status = Command::new("cargo")
            .env("CARGO_ENCODED_RUSTFLAGS", encoded_rustflags)
            .args(args)
            .status()
            .map_err(CompileError::CargoBuild)?;

        if !status.success() {
            return Err(CompileError::CargoBuildFailed(status));
        }

        let elf_path = metadata
            .target_directory
            .join(target.as_ref())
            .join(&self.profile)
            .join(&package.name);
        let elf = fs::read(elf_path).map_err(CompileError::ReadElf)?;

        Ok(elf)
    }
}

/// Returns `Metadata` of `manifest_dir` and guarantees the `root_package` can be resolved.
pub fn cargo_metadata(manifest_dir: impl AsRef<Path>) -> Result<Metadata, CompileError> {
    let manifest_path = manifest_dir.as_ref().join("Cargo.toml");
    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()
        .map_err(|err| CompileError::CargoMetadata {
            err,
            manifest_dir: manifest_dir.as_ref().to_path_buf(),
        })?;

    if metadata.root_package().is_none() {
        return Err(CompileError::RootPackageNotFound(
            manifest_dir.as_ref().to_path_buf(),
        ));
    }

    Ok(metadata)
}
