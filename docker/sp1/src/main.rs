use std::{fs, path::PathBuf, process::Command};

use anyhow::Context;
use clap::Parser;
use toml::Value as TomlValue;
use tracing::info;

#[derive(Parser)]
#[command(author, version)]
struct Cli {
    /// Path to the guest program crate directory.
    guest_folder: PathBuf,

    /// Compiled ELF output folder where guest.elf will be placed.
    elf_output_folder: PathBuf,
}

pub fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let dir = args.guest_folder;

    info!("Compiling SP1 program at {}", dir.display());

    if !dir.exists() || !dir.is_dir() {
        anyhow::bail!(
            "Program path does not exist or is not a directory: {}",
            dir.display()
        );
    }

    let guest_manifest_path = dir.join("Cargo.toml");
    if !guest_manifest_path.exists() {
        anyhow::bail!(
            "Cargo.toml not found in program directory: {}. Expected at: {}",
            dir.display(),
            guest_manifest_path.display()
        );
    }

    // ── read + parse Cargo.toml ───────────────────────────────────────────
    let manifest_content = fs::read_to_string(&guest_manifest_path)
        .with_context(|| format!("Failed to read file at {}", guest_manifest_path.display()))?;

    let manifest_toml: TomlValue = manifest_content.parse::<TomlValue>().with_context(|| {
        format!(
            "Failed to parse guest Cargo.toml at {}",
            guest_manifest_path.display()
        )
    })?;

    let program_name = manifest_toml
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .with_context(|| {
            format!(
                "Could not find `[package].name` in guest Cargo.toml at {}",
                guest_manifest_path.display()
            )
        })?;

    info!("Parsed program name: {program_name}");

    // ── build into a temp dir ─────────────────────────────────────────────
    info!(
        "Running `cargo prove build` → dir: {}",
        args.elf_output_folder.display()
    );

    let status = Command::new("cargo")
        .current_dir(&dir)
        .args([
            "prove",
            "build",
            "--output-directory",
            args.elf_output_folder.to_str().unwrap(),
            "--elf-name",
            "guest.elf",
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to execute `cargo prove build` in {}", dir.display()))?;

    if !status.success() {
        anyhow::bail!("Failed to execute `cargo prove build` in {}", dir.display())
    }

    let elf_path = args.elf_output_folder.join("guest.elf");
    if !elf_path.exists() {
        anyhow::bail!(
            "Compiled ELF not found at expected path: {}",
            elf_path.display()
        );
    }

    let elf_bytes = fs::read(&elf_path)
        .with_context(|| format!("Failed to read file at {}", elf_path.display()))?;
    info!("SP1 program compiled OK - {} bytes", elf_bytes.len());

    Ok(())
}
