use anyhow::Context;
use clap::{Parser, Subcommand};
use risc0_zkvm::{ExecutorEnv, ProverOpts, default_executor, default_prover};
use std::{fs, path::PathBuf, process::Command};
use toml::Value as TomlValue;
use tracing::info;
use zkvm_interface::{ProgramExecutionReport, ProgramProvingReport};

#[derive(Parser)]
#[command(author, version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a guest program
    Compile {
        /// Path to the guest program crate directory.
        guest_folder: PathBuf,
        /// Output folder where compiled `guest.elf` and `image_id` will be placed.
        output_folder: PathBuf,
    },
    /// Execute a compiled program
    Execute {
        /// Path to the compiled ELF file
        elf_path: PathBuf,
        /// Path to the serialized input bytes file
        input_path: PathBuf,
        /// Path where the execution report will be written
        report_path: PathBuf,
    },
    /// Prove execution of a compiled program
    Prove {
        /// Path to the compiled ELF file
        elf_path: PathBuf,
        /// Path to the serialized input bytes file
        input_path: PathBuf,
        /// Path where the proof will be written
        proof_path: PathBuf,
        /// Path where the report will be written
        report_path: PathBuf,
    },
}

pub fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    match args.command {
        Commands::Compile {
            guest_folder,
            output_folder,
        } => compile(guest_folder, output_folder),
        Commands::Prove {
            elf_path,
            input_path,
            proof_path,
            report_path,
        } => prove(elf_path, input_path, proof_path, report_path),
        Commands::Execute {
            elf_path,
            input_path,
            report_path,
        } => execute(elf_path, input_path, report_path),
    }
}

fn compile(guest_folder: PathBuf, output_folder: PathBuf) -> anyhow::Result<()> {
    let dir = guest_folder;

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
        output_folder.display()
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

    fs::copy(&elf_path, output_folder.join("guest.elf")).with_context(|| {
        format!(
            "Failed to copy elf file from {} to {}",
            elf_path.display(),
            output_folder.join("guest.elf").display()
        )
    })?;
    fs::write(output_folder.join("image_id"), hex::decode(image_id)?).with_context(|| {
        format!(
            "Failed to write image id to {}",
            output_folder.join("image_id").display()
        )
    })?;

    Ok(())
}

fn execute(elf_path: PathBuf, input_path: PathBuf, report_path: PathBuf) -> anyhow::Result<()> {
    info!("Starting execution for ELF at {}", elf_path.display());

    // Read the ELF file
    let elf = fs::read(&elf_path)
        .with_context(|| format!("Failed to read ELF file at {}", elf_path.display()))?;

    // Read the serialized input bytes
    let input_bytes = fs::read(&input_path)
        .with_context(|| format!("Failed to read input bytes at {}", input_path.display()))?;

    info!("ELF size: {} bytes", elf.len());
    info!("Input size: {} bytes", input_bytes.len());

    // Create executor environment using write_slice to write the serialized input bytes directly
    let executor = default_executor();
    let env = ExecutorEnv::builder()
        .write_slice(&input_bytes)
        .build()
        .context("Failed to build executor environment")?;

    info!("Starting execution...");
    let start = std::time::Instant::now();

    // Execute the program
    let session_info = executor
        .execute(env, &elf)
        .context("Failed to execute program")?;

    let execution_duration = start.elapsed();

    info!("Execution completed in {:?}", execution_duration);
    info!("Total cycles: {}", session_info.cycles());

    // Create execution report
    let report = ProgramExecutionReport {
        total_num_cycles: session_info.cycles() as u64,
        execution_duration,
        ..Default::default()
    };

    // Serialize and write the report
    let report_bytes =
        bincode::serialize(&report).context("Failed to serialize execution report")?;

    fs::write(&report_path, report_bytes)
        .with_context(|| format!("Failed to write report to {}", report_path.display()))?;

    info!("Execution report written to {}", report_path.display());
    Ok(())
}

fn prove(
    elf_path: PathBuf,
    input_path: PathBuf,
    proof_path: PathBuf,
    report_path: PathBuf,
) -> anyhow::Result<()> {
    info!(
        "Starting proof generation for ELF at {}",
        elf_path.display()
    );

    // Read the ELF file
    let elf = fs::read(&elf_path)
        .with_context(|| format!("Failed to read ELF file at {}", elf_path.display()))?;

    // Read the serialized input bytes
    let input_bytes = fs::read(&input_path)
        .with_context(|| format!("Failed to read input bytes at {}", input_path.display()))?;

    info!("ELF size: {} bytes", elf.len());
    info!("Input size: {} bytes", input_bytes.len());

    // Create prover environment using write_slice to write the serialized input bytes directly
    let prover = default_prover();
    let env = ExecutorEnv::builder()
        .write_slice(&input_bytes)
        .build()
        .context("Failed to build executor environment")?;

    info!("Starting proof generation...");

    let now = std::time::Instant::now();

    // Generate proof
    let prove_info = prover
        .prove_with_opts(env, &elf, &ProverOpts::succinct())
        .context("Failed to generate proof")?;

    let proving_time = now.elapsed();

    info!("Proof generation completed in {:?}", proving_time);

    // Serialize and write the proof
    let proof_bytes = borsh::to_vec(&prove_info.receipt).context("Failed to serialize proof")?;
    fs::write(&proof_path, proof_bytes)
        .with_context(|| format!("Failed to write proof to {}", proof_path.display()))?;

    let report_bytes = bincode::serialize(&ProgramProvingReport::new(proving_time))
        .context("Failed to serialize report")?;
    fs::write(&report_path, report_bytes)
        .with_context(|| format!("Failed to write report to {}", report_path.display()))?;

    info!("Proof written to {}", proof_path.display());
    info!("Report written to {}", report_path.display());

    Ok(())
}
