use anyhow::{Context, Error};
use clap::{Parser, Subcommand};
use ere_cli::serde;
use std::{fs, path::PathBuf};
use tracing_subscriber::EnvFilter;
use zkvm_interface::{Compiler, ProverResourceType, zkVM};

// Compile-time check to ensure exactly one backend feature is enabled
const _: () = {
    if cfg!(feature = "cli") {
        assert!(
            (cfg!(feature = "jolt") as u8
                + cfg!(feature = "miden") as u8
                + cfg!(feature = "nexus") as u8
                + cfg!(feature = "openvm") as u8
                + cfg!(feature = "pico") as u8
                + cfg!(feature = "risc0") as u8
                + cfg!(feature = "sp1") as u8
                + cfg!(feature = "ziren") as u8
                + cfg!(feature = "zisk") as u8)
                == 1,
            "Exactly one zkVM backend feature must be enabled"
        );
    }
};

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
        /// Path to the guest program
        #[arg(long)]
        guest_path: PathBuf,
        /// Path where the compiled program will be written
        #[arg(long)]
        program_path: PathBuf,
    },
    /// Execute a compiled program
    Execute {
        /// Path to the compiled program
        #[arg(long)]
        program_path: PathBuf,
        /// Path to the serialized input bytes file
        #[arg(long)]
        input_path: PathBuf,
        /// Path where the public values will be written
        #[arg(long)]
        public_values_path: PathBuf,
        /// Path where the execution report will be written
        #[arg(long)]
        report_path: PathBuf,
    },
    /// Prove execution of a compiled program
    Prove {
        /// Path to the compiled program
        #[arg(long)]
        program_path: PathBuf,
        /// Path to the serialized input bytes file
        #[arg(long)]
        input_path: PathBuf,
        /// Path where the public values will be written
        #[arg(long)]
        public_values_path: PathBuf,
        /// Path where the proof will be written
        #[arg(long)]
        proof_path: PathBuf,
        /// Path where the report will be written
        #[arg(long)]
        report_path: PathBuf,
        // Prover resource type
        #[command(subcommand)]
        resource: ProverResourceType,
    },
    /// Verify execution of a compiled program
    Verify {
        /// Path to the compiled program
        #[arg(long)]
        program_path: PathBuf,
        /// Path to the proof
        #[arg(long)]
        proof_path: PathBuf,
        /// Path where the public values will be written
        #[arg(long)]
        public_values_path: PathBuf,
    },
}

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Cli::parse();

    match args.command {
        Commands::Compile {
            guest_path,
            program_path,
        } => compile(guest_path, program_path),
        Commands::Execute {
            program_path,
            input_path,
            public_values_path,
            report_path,
        } => execute(program_path, input_path, public_values_path, report_path),
        Commands::Prove {
            program_path,
            input_path,
            public_values_path,
            proof_path,
            report_path,
            resource,
        } => prove(
            program_path,
            resource,
            input_path,
            public_values_path,
            proof_path,
            report_path,
        ),
        Commands::Verify {
            program_path,
            proof_path,
            public_values_path,
        } => verify(program_path, proof_path, public_values_path),
    }
}

fn compile(guest_path: PathBuf, program_path: PathBuf) -> Result<(), Error> {
    #[cfg(feature = "jolt")]
    let program = ere_jolt::JOLT_TARGET.compile(&guest_path);

    #[cfg(feature = "miden")]
    let program = ere_miden::MIDEN_TARGET.compile(&guest_path);

    #[cfg(feature = "nexus")]
    let program = ere_nexus::NEXUS_TARGET.compile(&guest_path);

    #[cfg(feature = "openvm")]
    let program = ere_openvm::OPENVM_TARGET.compile(&guest_path);

    #[cfg(feature = "pico")]
    let program = ere_pico::PICO_TARGET.compile(&guest_path);

    #[cfg(feature = "risc0")]
    let program = ere_risc0::RV32_IM_RISC0_ZKVM_ELF.compile(&guest_path);

    #[cfg(feature = "sp1")]
    let program = ere_sp1::RV32_IM_SUCCINCT_ZKVM_ELF.compile(&guest_path);

    #[cfg(feature = "ziren")]
    let program = ere_ziren::MIPS32R2_ZKM_ZKVM_ELF.compile(&guest_path);

    #[cfg(feature = "zisk")]
    let program = ere_zisk::RV64_IMA_ZISK_ZKVM_ELF.compile(&guest_path);

    serde::write(
        &program_path,
        &program.with_context(|| "Failed to compile program")?,
        "program",
    )
}

fn prove(
    program_path: PathBuf,
    resource: ProverResourceType,
    input_path: PathBuf,
    public_values_path: PathBuf,
    proof_path: PathBuf,
    report_path: PathBuf,
) -> Result<(), Error> {
    let zkvm = construct_zkvm(program_path, resource)?;
    let input = serde::read_input(&input_path)?;
    let (public_values, proof, report) = zkvm.prove(&input).with_context(|| "Failed to prove")?;
    fs::write(&public_values_path, &public_values).with_context(|| {
        format!(
            "Failed to write public values to {}",
            public_values_path.display()
        )
    })?;
    fs::write(&proof_path, proof)
        .with_context(|| format!("Failed to write proof to {}", proof_path.display()))?;
    serde::write(&report_path, &report, "report")?;
    Ok(())
}

fn execute(
    program_path: PathBuf,
    input_path: PathBuf,
    public_values_path: PathBuf,
    report_path: PathBuf,
) -> Result<(), Error> {
    let zkvm = construct_zkvm(program_path, ProverResourceType::Cpu)?;
    let input = serde::read_input(&input_path)?;
    let (public_values, report) = zkvm.execute(&input).with_context(|| "Failed to execute")?;
    fs::write(&public_values_path, &public_values).with_context(|| {
        format!(
            "Failed to write public values to {}",
            public_values_path.display()
        )
    })?;
    serde::write(&report_path, &report, "report")?;
    Ok(())
}

fn verify(
    program_path: PathBuf,
    proof_path: PathBuf,
    public_values_path: PathBuf,
) -> Result<(), Error> {
    let zkvm = construct_zkvm(program_path, ProverResourceType::Cpu)?;
    let proof = fs::read(&proof_path)
        .with_context(|| format!("Failed to read proof from {}", proof_path.display()))?;
    let public_values = zkvm
        .verify(&proof)
        .with_context(|| "Failed to verify proof")?;
    fs::write(&public_values_path, &public_values).with_context(|| {
        format!(
            "Failed to write public values to {}",
            public_values_path.display()
        )
    })?;
    Ok(())
}

fn construct_zkvm(program_path: PathBuf, resource: ProverResourceType) -> Result<impl zkVM, Error> {
    let program = serde::read(&program_path, "program")?;

    #[cfg(feature = "jolt")]
    let zkvm = ere_jolt::EreJolt::new(program, resource);

    #[cfg(feature = "miden")]
    let zkvm = ere_miden::EreMiden::new(program, resource);

    #[cfg(feature = "nexus")]
    let zkvm = Ok::<_, Error>(ere_nexus::EreNexus::new(program, resource));

    #[cfg(feature = "openvm")]
    let zkvm = ere_openvm::EreOpenVM::new(program, resource);

    #[cfg(feature = "pico")]
    let zkvm = Ok::<_, Error>(ere_pico::ErePico::new(program, resource));

    #[cfg(feature = "risc0")]
    let zkvm = ere_risc0::EreRisc0::new(program, resource);

    #[cfg(feature = "sp1")]
    let zkvm = Ok::<_, Error>(ere_sp1::EreSP1::new(program, resource));

    #[cfg(feature = "ziren")]
    let zkvm = Ok::<_, Error>(ere_ziren::EreZiren::new(program, resource));

    #[cfg(feature = "zisk")]
    let zkvm = ere_zisk::EreZisk::new(program, resource);

    zkvm.with_context(|| "Failed to instantiate zkVM")
}
