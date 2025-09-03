use std::process::Command;

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
