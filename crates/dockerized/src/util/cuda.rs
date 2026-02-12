use std::{env, process::Command};
use tracing::{info, warn};

/// Detects CUDA compute capabilities of all visible GPUs.
///
/// Returns a sorted, deduplicated list of numeric compute capabilities
/// (e.g. `[89, 120]` for a mix of RTX 40 and RTX 50 series GPUs).
///
/// Returns an empty vec if `nvidia-smi` is not available or fails.
pub fn detect_compute_caps() -> Vec<u32> {
    let Ok(output) = Command::new("nvidia-smi")
        .args(["--query-gpu=compute_cap", "--format=csv,noheader"])
        .output()
    else {
        return vec![];
    };

    if !output.status.success() {
        return vec![];
    }

    let mut caps: Vec<u32> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().replace('.', "").parse::<u32>().ok())
        .collect();
    caps.sort_unstable();
    caps.dedup();
    caps
}

/// Returns CUDA architectures as a list of numeric values (e.g. `[89, 120]`).
///
/// It does the following checks and returns the first valid value:
/// 1. Read env variable `CUDA_ARCHS` and validate format (comma-separated numbers).
/// 2. Detect compute capabilities of all visible GPUs.
///
/// Returns an empty vec if neither source provides valid architectures.
pub fn cuda_archs() -> Vec<u32> {
    if let Ok(val) = env::var("CUDA_ARCHS") {
        let archs: Option<Vec<u32>> = val.split(',').map(|s| s.parse::<u32>().ok()).collect();
        match archs {
            Some(archs) if !archs.is_empty() => {
                info!("Using CUDA_ARCHS {val} from env variable");
                return archs;
            }
            _ => warn!(
                "Skipping CUDA_ARCHS {val} from env variable \
                 (expected comma-separated numbers, e.g. \"89,120\")"
            ),
        }
    }

    let caps = detect_compute_caps();
    if !caps.is_empty() {
        info!("Detected CUDA compute capabilities (CUDA_ARCHS={caps:?})");
        return caps;
    }

    vec![]
}
