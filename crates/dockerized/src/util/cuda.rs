use std::{env, process::Command};
use tracing::{info, warn};

/// Returns Cuda GPU compute capability, for example
/// - RTX 50 series - returns `12.0`
/// - RTX 40 series - returns `8.9`
///
/// If there are multiple GPUs available, the first result will be returned.
pub fn cuda_compute_cap() -> Option<String> {
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=compute_cap", "--format=csv,noheader"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()?
            .trim()
            .to_string(),
    )
}

/// Returns CUDA architecture(s) as comma-separated numeric strings
/// (e.g. "120", "89,120").
///
/// It does the following checks and returns the first valid value:
/// 1. Read env variable `CUDA_ARCHS` and validate format (comma-separated numbers).
/// 2. Detect compute capability of the first visible GPU and convert to numeric format.
///
/// Otherwise it returns `None`.
pub fn cuda_archs() -> Option<String> {
    if let Ok(val) = env::var("CUDA_ARCHS") {
        let valid = !val.is_empty()
            && val
                .split(',')
                .all(|s| !s.is_empty() && s.parse::<u32>().is_ok());
        if valid {
            info!("Using CUDA_ARCHS {val} from env variable");
            return Some(val);
        }
        warn!(
            "Skipping CUDA_ARCHS {val} from env variable \
             (expected comma-separated numbers, e.g. \"89,120\")"
        );
    }

    if let Some(cap) = cuda_compute_cap() {
        let numeric = cap.replace('.', "");
        if numeric.parse::<u32>().is_ok() {
            info!("Using CUDA compute capability {cap} detected (CUDA_ARCHS={numeric})");
            return Some(numeric);
        }
        warn!(
            "Skipping CUDA compute capability {cap} detected \
            (expected a version number, e.g. 12.0)"
        );
    }

    None
}
