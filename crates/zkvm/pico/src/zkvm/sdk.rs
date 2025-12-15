// Copied and modified from https://github.com/brevis-network/pico/blob/v1.2.0/sdk/sdk/src/client.rs.
// The `EmbedProver` is removed because we don't need the proof to be verified
// on chain. Issue for tracking: https://github.com/eth-act/ere/issues/140.

use anyhow::{Error, Ok, Result};
use ere_zkvm_interface::zkvm::PublicValues;
use indexmap::IndexMap;
use pico_vm::{
    compiler::riscv::program::Program,
    configs::{config::StarkGenericConfig, stark_config::KoalaBearPoseidon2},
    emulator::{opts::EmulatorOpts, stdin::EmulatorStdinBuilder},
    instances::compiler::shapes::{
        recursion_shape::RecursionShapeConfig, riscv_shape::RiscvShapeConfig,
    },
    machine::{keys, proof},
    proverchain::{
        CombineProver, CompressProver, ConvertProver, InitialProverSetup, MachineProver,
        ProverChain, RiscvProver,
    },
};

pub type SC = KoalaBearPoseidon2;
pub type MetaProof = proof::MetaProof<SC>;
pub type BaseVerifyingKey = keys::BaseVerifyingKey<SC>;

pub struct ProverClient {
    riscv: RiscvProver<SC, Program>,
    convert: ConvertProver<SC, SC>,
    combine: CombineProver<SC, SC>,
    compress: CompressProver<SC, SC>,
}

impl ProverClient {
    pub fn new(elf: &[u8]) -> Self {
        let riscv = RiscvProver::new_initial_prover(
            (SC::new(), elf),
            EmulatorOpts::default().with_cycle_tracker(),
            Some(RiscvShapeConfig::default()),
        );
        let convert = ConvertProver::new_with_prev(
            &riscv,
            Default::default(),
            Some(RecursionShapeConfig::default()),
        );
        let combine = CombineProver::new_with_prev(
            &convert,
            Default::default(),
            Some(RecursionShapeConfig::default()),
        );
        let compress = CompressProver::new_with_prev(&combine, (), None);
        Self {
            riscv,
            convert,
            combine,
            compress,
        }
    }

    pub fn vk(&self) -> &BaseVerifyingKey {
        self.riscv.vk()
    }

    pub fn new_stdin_builder(&self) -> EmulatorStdinBuilder<Vec<u8>, SC> {
        EmulatorStdinBuilder::default()
    }

    /// Execute the program and return the cycles and public values
    pub fn execute(
        &self,
        stdin: EmulatorStdinBuilder<Vec<u8>, SC>,
    ) -> (u64, IndexMap<String, u64>, Vec<u8>) {
        let (stdin, _) = stdin.finalize();
        let (reports, public_values) = self.riscv.emulate(stdin);
        let total_num_cycles = reports.last().unwrap().current_cycle;
        let region_cycles = reports
            .into_iter()
            .fold(IndexMap::new(), |mut map, report| {
                if let Some(cycle_tracker) = report.cycle_tracker {
                    for (name, cycle_counts) in cycle_tracker {
                        let cycle_count = cycle_counts.iter().sum::<u64>();
                        *map.entry(name).or_default() += cycle_count;
                    }
                }
                map
            });
        (total_num_cycles, region_cycles, public_values)
    }

    /// Prove until `CompressProver`.
    pub fn prove(
        &self,
        stdin: EmulatorStdinBuilder<Vec<u8>, SC>,
    ) -> Result<(PublicValues, MetaProof), Error> {
        let (stdin, _) = stdin.finalize();
        let riscv_proof = self.riscv.prove(stdin);
        if !self.riscv.verify(&riscv_proof.clone(), self.riscv.vk()) {
            return Err(Error::msg("verify riscv proof failed"));
        }
        let proof = self.convert.prove(riscv_proof.clone());
        if !self.convert.verify(&proof, self.riscv.vk()) {
            return Err(Error::msg("verify convert proof failed"));
        }
        let proof = self.combine.prove(proof);
        if !self.combine.verify(&proof, self.riscv.vk()) {
            return Err(Error::msg("verify combine proof failed"));
        }
        let proof = self.compress.prove(proof);
        if !self.compress.verify(&proof, self.riscv.vk()) {
            return Err(Error::msg("verify compress proof failed"));
        }
        Ok((riscv_proof.pv_stream.clone().unwrap_or_default(), proof))
    }

    /// Verify a compressed proof.
    pub fn verify(&self, proof: &MetaProof) -> Result<(), Error> {
        if !self.compress.verify(proof, self.riscv.vk()) {
            return Err(Error::msg("verify compress proof failed"));
        }
        Ok(())
    }
}
