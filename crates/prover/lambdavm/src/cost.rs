use std::{collections::BTreeMap, sync::Arc};

use ere_compiler_core::Elf;
use ere_prover_core::{CostEstimation, PublicValues};
use lambda_vm_prover::count_elements;

use crate::{
    error::Error,
    executor::{extract_public_values, run},
};

pub(crate) struct CostEstimator {
    elf: Elf,
    program: Arc<lambda_vm_executor::elf::Elf>,
}

impl CostEstimator {
    pub(crate) fn new(elf: &Elf, program: &Arc<lambda_vm_executor::elf::Elf>) -> Self {
        Self {
            elf: elf.clone(),
            program: program.clone(),
        }
    }

    pub(crate) fn estimate(&self, stdin: &[u8]) -> Result<(PublicValues, CostEstimation), Error> {
        let executor = run(&self.program, stdin)?;

        let (main_elements, aux_elements) =
            count_elements(&self.elf.0, stdin).map_err(Error::EstimateCost)?;

        let cost = BTreeMap::from([
            ("main_elements".to_owned(), main_elements),
            ("aux_elements".to_owned(), aux_elements),
        ]);

        Ok((extract_public_values(executor)?, CostEstimation { cost }))
    }
}
