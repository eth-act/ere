use std::{
    env,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use tracing::{debug, warn};

/// Configuration for GPU metrics collection
#[derive(Clone, Debug)]
pub struct GpuMetricsConfig {
    /// Name of the zkVM (e.g., "sp1", "risc0")
    pub zkvm_name: &'static str,
    /// Output directory for CSV files
    pub output_dir: PathBuf,
}

/// Handle for a running GPU metrics collection process
pub struct GpuMetricsCollector {
    process: Child,
    output_file: PathBuf,
}

impl GpuMetricsCollector {
    /// Start collecting GPU metrics
    ///
    /// Spawns `nvidia-smi` subprocess that writes CSV to disk every second.
    /// Returns `Ok(None)` if nvidia-smi is not available.
    /// Returns `Err` only on unexpected errors.
    pub fn start(config: &GpuMetricsConfig) -> Result<Option<Self>> {
        // Create output directory if it doesn't exist
        fs::create_dir_all(&config.output_dir).with_context(|| {
            format!(
                "Failed to create directory: {}",
                config.output_dir.display()
            )
        })?;

        // Generate filename: metrics_{zkvm}_{timestamp}.csv
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("metrics_{}_{}.csv", config.zkvm_name, timestamp);
        let output_file = config.output_dir.join(filename);

        // Get query fields (from env or default)
        let fields = env::var("ERE_GPU_METRICS").unwrap_or_else(|_| {
            "timestamp,name,utilization.gpu,utilization.memory,memory.used,memory.total,temperature.gpu"
                .to_string()
        });

        let file = File::create(&output_file)
            .with_context(|| format!("Failed to create metrics file: {}", output_file.display()))?;

        // Build and spawn nvidia-smi command
        let process = match Command::new("nvidia-smi")
            .args(["--query-gpu", &fields])
            .args(["--format", "csv"])
            .args(["-l", "1"]) // Sample every 1 second
            .stdout(Stdio::from(file))
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(process) => process,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                warn!("nvidia-smi not found; skipping GPU metrics collection");
                return Ok(None);
            }
            Err(e) => return Err(e).context("Failed to spawn nvidia-smi process"),
        };

        debug!(
            "Started GPU metrics collection (PID: {}): {}",
            process.id(),
            output_file.display()
        );

        Ok(Some(Self {
            process,
            output_file,
        }))
    }

    /// Stop collecting GPU metrics and return the file path
    pub fn stop(mut self) -> Result<PathBuf> {
        debug!(
            "Stopping GPU metrics collection (PID: {})",
            self.process.id()
        );

        let _ = self.process.kill();
        let _ = self.process.wait();

        Ok(self.output_file.clone())
    }

    /// Get the output file path
    pub fn output_file(&self) -> &Path {
        &self.output_file
    }
}

impl Drop for GpuMetricsCollector {
    fn drop(&mut self) {
        // Ensure process is killed and reaped
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
