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

    /// Output folder where compiled `guest.elf` and `image_id` will be placed.
    output_folder: PathBuf,
}

pub fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    let dir = args.guest_folder;

    info!("Compiling Risc0 program at {}", dir.display());

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
        "Running `cargo risczero build` → dir: {}",
        args.output_folder.display()
    );

    let output = Command::new("cargo")
        .current_dir(&dir)
        .args(["risczero", "build"])
        .stderr(std::process::Stdio::inherit())
        .output()
        .with_context(|| {
            format!(
                "Failed to execute `cargo risczer build` in {}",
                dir.display()
            )
        })?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to execute `cargo risczero build` in {}",
            dir.display()
        )
    }

    let (image_id, elf_path) = {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout
            .lines()
            .find(|line| line.starts_with("ImageID: "))
            .unwrap();
        let (image_id, elf_path) = line
            .trim_start_matches("ImageID: ")
            .split_once(" - ")
            .unwrap();
        (image_id.to_string(), PathBuf::from(elf_path))
    };

    if !elf_path.exists() {
        anyhow::bail!(
            "Compiled ELF not found at expected path: {}",
            elf_path.display()
        );
    }

    let elf_bytes = fs::read(&elf_path)
        .with_context(|| format!("Failed to read file at {}", elf_path.display()))?;
    info!("Risc0 program compiled OK - {} bytes", elf_bytes.len());
    info!("Image ID - {image_id}");

    fs::copy(&elf_path, args.output_folder.join("guest.elf")).with_context(|| {
        format!(
            "Failed to copy elf file from {} to {}",
            elf_path.display(),
            args.output_folder.join("guest.elf").display()
        )
    })?;
    fs::write(args.output_folder.join("image_id"), hex::decode(image_id)?).with_context(|| {
        format!(
            "Failed to write image id to {}",
            args.output_folder.join("image_id").display()
        )
    })?;

    Ok(())
}
