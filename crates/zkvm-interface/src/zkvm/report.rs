use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// ProgramExecutionReport produces information about a particular program
/// execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgramExecutionReport {
    /// Total number of cycles for the entire workload execution.
    pub total_num_cycles: u64,
    /// Region-specific cycles, mapping region names (e.g., "setup", "compute") to their cycle counts.
    pub region_cycles: IndexMap<String, u64>,
    /// Execution duration.
    pub execution_duration: Duration,
}

impl ProgramExecutionReport {
    pub fn new(total_num_cycles: u64) -> Self {
        ProgramExecutionReport {
            total_num_cycles,
            ..Default::default()
        }
    }
    pub fn insert_region(&mut self, region_name: String, num_cycles: u64) {
        self.region_cycles.insert(region_name, num_cycles);
    }
}

/// ProgramProvingReport produces information about proving a particular
/// program's instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgramProvingReport {
    pub proving_time: Duration,
}
impl ProgramProvingReport {
    pub fn new(proving_time: Duration) -> Self {
        Self { proving_time }
    }
}
