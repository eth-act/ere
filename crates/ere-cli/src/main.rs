use anyhow::{Context, Error};
use clap::{Parser, Subcommand};
use ere_dockerized::input::deserialize_inputs;
use std::{
    fs,
    path::{Path, PathBuf},
};
use zkvm_interface::{Compiler, Input, zkVM};

// Compile-time check to ensure exactly one backend feature is enabled
const _: () = {
    let features = [
        cfg!(feature = "jolt"),
        cfg!(feature = "nexus"),
        cfg!(feature = "openvm"),
        cfg!(feature = "pico"),
        cfg!(feature = "risc0"),
        cfg!(feature = "sp1"),
        cfg!(feature = "zisk"),
    ];
    let mut count = 0;
    let mut idx = 0;
    while idx < features.len() {
        count += features[idx] as usize;
        idx += 1;
    }
    match count {
        0 => panic!("Exactly one zkVM backend feature must be enabled"),
        1 => {}
        _ => panic!("Only one zkVM backend feature can be enabled at a time"),
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
        guest_path: PathBuf,
        /// Path where the compiled program will be written
        program_path: PathBuf,
    },
    /// Execute a compiled program
    Execute {
        /// Path to the compiled program
        program_path: PathBuf,
        /// Path to the serialized input bytes file
        input_path: PathBuf,
        /// Path where the execution report will be written
        report_path: PathBuf,
    },
    /// Prove execution of a compiled program
    Prove {
        /// Path to the compiled program
        program_path: PathBuf,
        /// Prover resource
        #[arg(value_enum)]
        resource: ProverResourceType,
        /// Path to the serialized input bytes file
        input_path: PathBuf,
        /// Path where the proof will be written
        proof_path: PathBuf,
        /// Path where the report will be written
        report_path: PathBuf,
    },
    /// Verify execution of a compiled program
    Verify {
        /// Path to the compiled program
        program_path: PathBuf,
        /// Path to the proof
        proof_path: PathBuf,
    },
}

#[derive(Clone, clap::ValueEnum)]
enum ProverResourceType {
    Cpu,
    Gpu,
}

impl From<ProverResourceType> for zkvm_interface::ProverResourceType {
    fn from(value: ProverResourceType) -> Self {
        match value {
            ProverResourceType::Cpu => Self::Cpu,
            ProverResourceType::Gpu => Self::Gpu,
        }
    }
}

fn main() -> Result<(), Error> {
    let args = Cli::parse();

    match args.command {
        Commands::Compile {
            guest_path,
            program_path,
        } => compile(guest_path, program_path),
        Commands::Prove {
            program_path,
            resource,
            input_path,
            proof_path,
            report_path,
        } => prove(program_path, resource, input_path, proof_path, report_path),
        Commands::Execute {
            program_path,
            input_path,
            report_path,
        } => execute(program_path, input_path, report_path),
        Commands::Verify {
            program_path,
            proof_path,
        } => verify(program_path, proof_path),
    }
}

fn compile(guest_path: PathBuf, program_path: PathBuf) -> Result<(), Error> {
    macro_rules! compile {
        ($compiler:expr, serialize) => {
            $compiler
                .compile(&guest_path)
                .with_context(|| "Failed to compile program")
                .and_then(|program| {
                    bincode::serialize(&program).with_context(|| "Failed to serialize program")
                })?
        };
        ($compiler:expr) => {
            $compiler
                .compile(&guest_path)
                .with_context(|| "Failed to compile program")?
        };
    }

    #[cfg(feature = "jolt")]
    let program = compile!(ere_jolt::JOLT_TARGET, serialize);

    #[cfg(feature = "nexus")]
    let program = compile!(ere_nexus::NEXUS_TARGET, serialize);

    #[cfg(feature = "openvm")]
    let program = compile!(ere_openvm::OPENVM_TARGET, serialize);

    #[cfg(feature = "pico")]
    let program = compile!(ere_pico::PICO_TARGET);

    #[cfg(feature = "risc0")]
    let program = compile!(ere_risc0::RV32_IM_RISC0_ZKVM_ELF, serialize);

    #[cfg(feature = "sp1")]
    let program = compile!(ere_sp1::RV32_IM_SUCCINCT_ZKVM_ELF);

    #[cfg(feature = "zisk")]
    let program = compile!(ere_zisk::RV64_IMA_ZISK_ZKVM_ELF);

    fs::write(&program_path, program)
        .with_context(|| format!("Failed to write program to {}", program_path.display()))
}

fn prove(
    program_path: PathBuf,
    resource: ProverResourceType,
    input_path: PathBuf,
    proof_path: PathBuf,
    report_path: PathBuf,
) -> Result<(), Error> {
    let zkvm = construct_zkvm(program_path, resource.into())?;
    let inputs = read_inputs(input_path)?;
    let (proof, report) = zkvm.prove(&inputs).with_context(|| "Failed to prove")?;
    fs::write(&proof_path, proof)
        .with_context(|| format!("Failed to write proof to {}", proof_path.display()))?;
    fs::write(
        &report_path,
        bincode::serialize(&report).with_context(|| "Failed to serialize report")?,
    )
    .with_context(|| format!("Failed to write report to {}", report_path.display()))?;
    Ok(())
}

fn execute(program_path: PathBuf, input_path: PathBuf, report_path: PathBuf) -> Result<(), Error> {
    let zkvm = construct_zkvm(program_path, Default::default())?;
    let inputs = read_inputs(input_path)?;
    let report = zkvm.execute(&inputs).with_context(|| "Failed to execute")?;
    fs::write(
        &report_path,
        bincode::serialize(&report).with_context(|| "Failed to serialize report")?,
    )
    .with_context(|| format!("Failed to write report to {}", report_path.display()))?;
    Ok(())
}

fn verify(program_path: PathBuf, proof_path: PathBuf) -> Result<(), Error> {
    let zkvm = construct_zkvm(program_path, Default::default())?;
    let proof = fs::read(&proof_path)
        .with_context(|| format!("Failed to read proof from {}", proof_path.display()))?;
    zkvm.verify(&proof)
        .with_context(|| "Failed to verify proof")?;
    Ok(())
}

fn construct_zkvm(
    program_path: PathBuf,
    resource: zkvm_interface::ProverResourceType,
) -> Result<impl zkVM, Error> {
    let program = fs::read(&program_path)
        .with_context(|| format!("Failed to read program at {}", program_path.display()))?;

    #[cfg(feature = "jolt")]
    {
        let program =
            bincode::deserialize(&program).with_context(|| "Failed to deserialize program")?;
        Ok(ere_jolt::EreJolt::new(program, resource))
    }

    #[cfg(feature = "nexus")]
    {
        let program =
            bincode::deserialize(&program).with_context(|| "Failed to deserialize program")?;
        Ok(ere_nexus::EreNexus::new(program, resource))
    }

    #[cfg(feature = "openvm")]
    {
        let program =
            bincode::deserialize(&program).with_context(|| "Failed to deserialize program")?;
        Ok(ere_openvm::EreOpenVM::new(program, resource))
    }

    #[cfg(feature = "pico")]
    return Ok(ere_pico::ErePico::new(program, resource));

    #[cfg(feature = "risc0")]
    {
        let program =
            bincode::deserialize(&program).with_context(|| "Failed to deserialize program")?;
        Ok(ere_risc0::EreRisc0::new(program, resource))
    }

    #[cfg(feature = "sp1")]
    return Ok(ere_sp1::EreSP1::new(program, resource));

    #[cfg(feature = "zisk")]
    return Ok(ere_zisk::EreZisk::new(program, resource));
}

fn read_inputs(input_path: PathBuf) -> Result<Input, Error> {
    let bytes = fs::read(&input_path)
        .with_context(|| format!("Failed to read input at {}", input_path.display()))?;
    deserialize_inputs(&bytes)
}
