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

/// Returns the GPU code in format `sm_{numeric_compute_cap}` (e.g. `sm_120`).
///
/// It does the following checks and returns the first valid value:
/// 1. Read env variable `CUDA_ARCH` and check if it is in valid format.
/// 2. Detect compute capability of the first visible GPU and format to GPU code.
///
/// Otherwise it returns `None`.
pub fn cuda_arch() -> Option<String> {
    if let Ok(cuda_arch) = env::var("CUDA_ARCH") {
        if cuda_arch.starts_with("sm_") && cuda_arch[3..].parse::<usize>().is_ok() {
            info!("Using CUDA_ARCH {cuda_arch} from env variable");
            Some(cuda_arch)
        } else {
            warn!(
                "Skipping CUDA_ARCH {cuda_arch} from env variable (expected to be in format `sm_XX`)"
            );
            None
        }
    } else if let Some(cap) = cuda_compute_cap() {
        info!("Using CUDA compute capability {} detected", cap);
        Some(format!("sm_{}", cap.replace(".", "")))
    } else {
        None
    }
}
