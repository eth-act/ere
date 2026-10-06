use std::{
    any::Any,
    env,
    ops::Range,
    panic::{self, AssertUnwindSafe},
    sync::Arc,
    time::Duration,
};

use ere_cluster_client_zisk::ZiskClusterClient;
use ere_compiler_core::Elf;
use ere_prover_core::{
    CallTree, CommonError, CostEstimation, CostProfile, Input, PeakMemory, ProverResource,
    ProverResourceKind, PublicValues, RasAction, SymbolMap, loadable_segments,
};
use ere_util_tokio::block_on;
use ere_verifier_zisk::{ZiskProgramVk, ZiskProof, ensure_program_vk_matches};
use tokio::time::Instant;
use zisk_common::EmuTrace;
use zisk_core::{
    INPUT_ADDR, OUTPUT_ADDR, OUTPUT_MAX_SIZE, RAM_ADDR, RAM_SIZE, ROM_ENTRY, SYS_ADDR, SYS_SIZE,
    ZiskRom,
};
use zisk_transpiler_riscv::Riscv2zisk;
use ziskemu::{Emu, EmuOptions};

use crate::{
    cost::{self, COMPONENTS},
    error::Error,
    executor::ZiskExecutor,
    sdk::local::LocalProver,
};

mod local;
mod proving_key;

/// Address of the control input that the ASM emulator maps over the input region, from
/// `emulator-asm/src/constants.hpp`.
const CONTROL_INPUT_ADDR: u64 = 0x7000_0000;

/// Largest stdin that fits in the input region before the control input, after the free-input word
/// and the length prefix.
const MAX_STDIN_SIZE: u64 = CONTROL_INPUT_ADDR - INPUT_ADDR - 16;

/// Step limit of the ZisK prover, whose PIL gives each step index 36 bits.
pub(crate) const MAX_STEPS: u64 = 1 << 36;

/// Default ZisK cluster prove timeout seconds.
const DEFAULT_ZISK_CLUSTER_PROVE_TIMEOUT_SECS: u64 = 600;

#[allow(clippy::large_enum_variant)]
enum Backend {
    Local(LocalProver),
    Cluster {
        client: ZiskClusterClient,
        prove_timeout: Duration,
    },
}

pub struct ZiskSdk {
    resource: ProverResource,
    backend: Backend,
    rom: ZiskRom,
    executor: ZiskExecutor,
    symbol_map: Arc<SymbolMap>,
    non_heap: Vec<Range<u64>>,
}

impl ZiskSdk {
    pub fn new(elf: &Elf, resource: ProverResource) -> Result<Self, Error> {
        let rom = rom(elf)?;

        let executor = ZiskExecutor::new(elf, &rom);
        let symbol_map = Arc::new(SymbolMap::from_elf(elf)?);
        let non_heap = non_heap(elf)?;

        // Initialize prover
        let backend = match &resource {
            ProverResource::Cpu | ProverResource::Gpu => {
                Backend::Local(LocalProver::new(elf.clone(), &resource)?)
            }
            ProverResource::Cluster(config) => {
                let client = block_on(ZiskClusterClient::new(config, elf.clone()))?;
                let prove_timeout = Duration::from_secs(
                    env::var("ERE_ZISK_CLUSTER_PROVE_TIMEOUT_SECS")
                        .ok()
                        .and_then(|val| val.parse::<u64>().ok())
                        .unwrap_or(DEFAULT_ZISK_CLUSTER_PROVE_TIMEOUT_SECS),
                );
                Backend::Cluster {
                    client,
                    prove_timeout,
                }
            }
            ProverResource::Network(_) => {
                return Err(CommonError::unsupported_prover_resource_kind(
                    resource.kind(),
                    [
                        ProverResourceKind::Cpu,
                        ProverResourceKind::Gpu,
                        ProverResourceKind::Cluster,
                    ],
                )
                .into());
            }
        };

        Ok(Self {
            resource,
            backend,
            rom,
            executor,
            symbol_map,
            non_heap,
        })
    }

    /// Replaces the program.
    pub fn setup(&mut self, elf: &Elf) -> Result<(), Error> {
        let rom = rom(elf)?;
        let executor = ZiskExecutor::new(elf, &rom);
        let symbol_map = Arc::new(SymbolMap::from_elf(elf)?);
        let non_heap = non_heap(elf)?;

        match &mut self.backend {
            Backend::Local(local) => local.setup(elf.clone())?,
            Backend::Cluster { client, .. } => {
                let ProverResource::Cluster(config) = &self.resource else {
                    unreachable!("a cluster backend runs on a cluster resource")
                };
                *client = block_on(ZiskClusterClient::new(config, elf.clone()))?;
            }
        }

        self.rom = rom;
        self.executor = executor;
        self.symbol_map = symbol_map;
        self.non_heap = non_heap;
        Ok(())
    }

    pub fn program_vk(&self) -> ZiskProgramVk {
        match &self.backend {
            Backend::Local(local) => local.program_vk(),
            Backend::Cluster { client, .. } => client.program_vk(),
        }
    }

    /// Execute the ELF with the given `stdin`.
    pub fn execute(&self, input: &Input) -> Result<(PublicValues, Duration), Error> {
        ensure_stdin_fits(input)?;
        self.executor
            .execute(&self.rom, framed_stdin(input.stdin()))
    }

    pub fn execute_estimated_cost(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, CostEstimation), Error> {
        let (emu, report) = self.run_with_stats(input, |_, _| {})?;
        let cost = cost::parse(&report)?;
        let public_values = emu.get_output_8().into();
        Ok((public_values, CostEstimation { cost }))
    }

    /// Splits the cost of [`Self::execute_estimated_cost`] over the guest call stacks, and measures
    /// the peak memory use.
    pub fn profile(&self, input: &Input) -> Result<(PublicValues, CostProfile), Error> {
        let mut call_tree = CallTree::new(
            Arc::clone(&self.symbol_map),
            ROM_ENTRY,
            &COMPONENTS.map(|(component, _)| component),
        );
        let mut running = [0; COMPONENTS.len()];
        let mut memory = PeakMemory::new(RAM_ADDR..RAM_ADDR + RAM_SIZE, &self.non_heap);
        let (emu, report) = self.run_with_stats(input, |emu, pc| {
            let instruction = emu.rom.get_instruction(pc);
            let context = &emu.ctx.inst_ctx;
            let action = instruction
                .meta_rd
                .and_then(|rd| RasAction::from_jump(rd, instruction.meta_rs1?));
            // The step ran in the frame that runs before a frame change.
            cost::record_accesses(&mut memory, call_tree.frame(), instruction, context);
            // An odd PC is an internal instruction of the RISC-V instruction that runs.
            // https://github.com/0xPolygonHermez/zisk/blob/v1.3.1-alpha/core/src/zisk_rom.rs#L545-L563
            if action.is_some() || (context.pc % 2 == 0 && !call_tree.contains(context.pc)) {
                running = cost::running_cost(emu.ctx.stats.get_costs());
                call_tree.charge(&running);
                // A call writes its return address to `rd`, 8 bytes past a fused `auipc`.
                // https://github.com/0xPolygonHermez/zisk/blob/v1.3.1-alpha/transpilers/riscv/src/riscv2zisk_context.rs#L1261-L1285
                let return_address = instruction
                    .meta_rd
                    .map_or(0, |rd| context.regs[rd as usize]);
                call_tree.transfer(action, context.pc, return_address, context.regs[2]);
            }
            memory.stack_pointer(context.regs[2], cost::stack_pointer_write(instruction));
        })?;
        // The steps after the last frame change belong to the frame that runs at the end.
        running = cost::running_cost(emu.ctx.stats.get_costs());
        call_tree.charge(&running);

        // The report adds `base` and the ROM and RAM init in `memory` to what the steps spend.
        let report = cost::parse(&report)?;
        let total = COMPONENTS.map(|(component, _)| report[component]);
        for (((component, _), total), stepped) in COMPONENTS.iter().zip(total).zip(running) {
            assert!(
                total == stepped || (total > stepped && matches!(*component, "base" | "memory")),
                "the steps spend {stepped} {component} cells, the report {total}"
            );
        }
        running[0] = total[0];
        call_tree.charge_root("[base]", &running);
        call_tree.charge_root("[init]", &total);

        let public_values = emu.get_output_8().into();
        let cost_profile = call_tree.into_profile("cells", &memory);

        Ok((public_values, cost_profile))
    }

    /// Runs `input` on the Rust emulator with statistics, and returns the emulator and its report.
    /// It steps the emulator as `Emu::run` does, and calls `on_step` after each step with the
    /// emulator and the PC of the step.
    /// https://github.com/0xPolygonHermez/zisk/blob/v1.3.1-alpha/emulator/src/emu.rs#L1672-L2092
    fn run_with_stats(
        &self,
        input: &Input,
        mut on_step: impl FnMut(&Emu<'_>, u64),
    ) -> Result<(Emu<'_>, String), Error> {
        ensure_stdin_fits(input)?;
        let stdin = framed_stdin(input.stdin());
        let options = EmuOptions {
            max_steps: MAX_STEPS,
            stats: true,
            ..Default::default()
        };
        let mut emu = Emu::new(&self.rom);

        panic::catch_unwind(AssertUnwindSafe(|| {
            emu.ctx = emu.create_emu_context(stdin, &options);
            emu.ctx.stats.load_rom_data(&self.rom);
            emu.ctx.do_stats = true;
            while !emu.ctx.inst_ctx.end && emu.ctx.inst_ctx.step < MAX_STEPS {
                let pc = emu.ctx.inst_ctx.pc;
                emu.step(&options, &None::<Box<dyn Fn(EmuTrace)>>);
                on_step(&emu, pc);
            }
        }))
        .map_err(|err| Error::EmulatorPanic(panic_msg(err)))?;

        if !emu.terminated() {
            return Err(Error::EmulatorNotTerminated);
        }

        if emu.ctx.inst_ctx.error {
            return Err(Error::EmulatorError);
        }

        emu.ctx.stats.on_finish(&emu.ctx.inst_ctx);
        emu.ctx.stats.set_use_thousands_sep(false);
        let report = emu.ctx.stats.report(&self.rom);
        Ok((emu, report))
    }

    pub fn prove(&self, input: &Input) -> Result<(PublicValues, ZiskProof, Duration), Error> {
        ensure_stdin_fits(input)?;
        if cfg!(not(feature = "cuda")) && self.resource == ProverResource::Gpu {
            return Err(Error::CudaFeatureDisabled);
        }

        let (proof, proving_time) = match &self.backend {
            Backend::Local(local) => local.prove(input)?,
            Backend::Cluster {
                client,
                prove_timeout,
            } => block_on(async {
                let deadline = Instant::now() + *prove_timeout;
                client.prove(input, deadline).await.map_err(Error::Cluster)
            })?,
        };

        let (program_vk, public_values) = proof.program_vk_and_public_values()?;

        ensure_program_vk_matches(self.program_vk(), program_vk)?;

        Ok((public_values, proof, proving_time))
    }
}

/// Address ranges of RAM that hold no heap, which are the loadable segments of `elf` and the system
/// and output areas that the startup and exit code of the transpiler use.
fn non_heap(elf: &Elf) -> Result<Vec<Range<u64>>, Error> {
    Ok(loadable_segments(elf)?
        .into_iter()
        .chain([
            SYS_ADDR..SYS_ADDR + SYS_SIZE,
            OUTPUT_ADDR..OUTPUT_ADDR + OUTPUT_MAX_SIZE,
        ])
        .collect())
}

/// Converts `elf` to the ZisK ROM.
fn rom(elf: &Elf) -> Result<ZiskRom, Error> {
    Riscv2zisk::new(elf)
        .run()
        .map_err(|err| Error::Riscv2zisk(err.to_string()))
}

/// Rejects a stdin that ZisK cannot read intact.
fn ensure_stdin_fits(input: &Input) -> Result<(), Error> {
    if input.stdin().len() as u64 > MAX_STDIN_SIZE {
        Err(CommonError::unsupported_input(format!(
            "stdin of {} bytes exceeds {MAX_STDIN_SIZE} bytes",
            input.stdin().len()
        )))?
    }
    Ok(())
}

/// Returns `data` with a LE u64 length prefix and padding to multiple of 8.
///
/// The length prefix and padding is expected by ZisK emulator/prover runtime.
fn framed_stdin(data: &[u8]) -> Vec<u8> {
    let len = (8 + data.len()).next_multiple_of(8);
    let mut buf = Vec::with_capacity(len);
    buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
    buf.extend_from_slice(data);
    buf.resize(len, 0);
    buf
}

pub(crate) fn panic_msg(err: Box<dyn Any + Send + 'static>) -> String {
    None.or_else(|| err.downcast_ref::<String>().cloned())
        .or_else(|| err.downcast_ref::<&'static str>().map(ToString::to_string))
        .unwrap_or_else(|| "unknown panic msg".to_string())
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use ere_prover_core::zkVMProver;
    use ere_verifier_zisk::ZiskProgramVk;
    use tempfile::tempdir;

    use crate::{
        prover::tests::{basic_elf, with_basic_elf, zkvm},
        sdk::proving_key::ensure_proving_key,
    };

    #[test]
    fn program_vk_matches_cargo_zisk_program_setup() {
        let program_vk = {
            ensure_proving_key().unwrap();

            let tempdir = tempdir().unwrap();
            let elf_path = tempdir.path().join("guest.elf");
            fs::write(&elf_path, &basic_elf().0).unwrap();

            let status = Command::new("cargo-zisk-dev")
                .arg("program-setup")
                .arg("-e")
                .arg(&elf_path)
                .arg("-o")
                .arg(tempdir.path())
                .status()
                .unwrap();
            assert!(status.success());

            let verkey_paths = fs::read_dir(tempdir.path())
                .unwrap()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(".verkey.bin"))
                })
                .collect::<Vec<_>>();
            assert_eq!(verkey_paths.len(), 1);

            ZiskProgramVk::try_from(fs::read(&verkey_paths[0]).unwrap().as_slice()).unwrap()
        };

        assert_eq!(*with_basic_elf(zkvm()).program_vk(), program_vk);
    }
}
