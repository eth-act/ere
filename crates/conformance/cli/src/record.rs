//! Run records for the conformance dashboard.
//!
//! The dashboard reads `{dashboard}/config.json` and one history file per
//! zkVM and suite, `{dashboard}/data/history/{zkvm}-{suite}.json`, shaped as
//! `{"runs": [Run, ...]}` with the newest run last.

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::Serialize;
use serde_json::{Value, json};

/// Final outcome of one test; each test lands in exactly one bucket.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Passed,
    Failed,
    ProveFailed,
    VerifyFailed,
}

/// Result of one test, kept in the per-run details file.
#[derive(Debug, Serialize)]
pub struct TestResult {
    pub name: String,
    pub outcome: Outcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Seconds spent starting the server, including program key generation.
    pub setup_secs: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execute_secs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prove_secs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verify_secs: Option<f64>,
}

/// One history entry.
#[derive(Debug, Serialize)]
pub struct Run {
    pub date: String,
    pub ere_rev: String,
    pub ere_version: String,
    pub ere_image: String,
    pub sdk_version: String,
    pub act4_commit: String,
    pub act4_version: String,
    pub isa: String,
    pub mode: String,
    pub resource: String,
    pub total: usize,
    pub passed: Vec<String>,
    pub failed: Vec<String>,
    pub prove_failed: Vec<String>,
    pub verify_failed: Vec<String>,
    pub has_proving: bool,
}

impl Run {
    /// Fills the per-outcome name lists from `results`.
    pub fn with_results(mut self, results: &[TestResult]) -> Self {
        self.total = results.len();
        for result in results {
            let bucket = match result.outcome {
                Outcome::Passed => &mut self.passed,
                Outcome::Failed => &mut self.failed,
                Outcome::ProveFailed => &mut self.prove_failed,
                Outcome::VerifyFailed => &mut self.verify_failed,
            };
            bucket.push(result.name.clone());
        }
        self
    }
}

/// Full ISA of `zkvm` as declared in the dashboard's `config.json`.
pub fn isa(dashboard: &Path, zkvm: &str) -> anyhow::Result<String> {
    let config = read_json(&dashboard.join("config.json"))?;
    config["zkvms"][zkvm]["isa"]
        .as_str()
        .map(str::to_string)
        .with_context(|| format!("config.json has no zkvms.{zkvm}.isa"))
}

/// Appends `run` to the history of `zkvm` and `suite`, and records the ACT4
/// pin it used in `config.json`.
pub fn append(dashboard: &Path, zkvm: &str, suite: &str, run: &Run) -> anyhow::Result<PathBuf> {
    let path = dashboard
        .join("data/history")
        .join(format!("{zkvm}-{suite}.json"));
    let mut history = if path.exists() {
        read_json(&path)?
    } else {
        json!({ "runs": [] })
    };
    history["runs"]
        .as_array_mut()
        .with_context(|| format!("{} has no `runs` array", path.display()))?
        .push(serde_json::to_value(run)?);
    write_json(&path, &history)?;

    let config_path = dashboard.join("config.json");
    let mut config = read_json(&config_path)?;
    config["act4_commit"] = json!(run.act4_commit);
    config["act4_version"] = json!(run.act4_version);
    write_json(&config_path, &config)?;

    Ok(path)
}

/// Writes per-test outcomes, errors and timings of one run.
pub fn write_details(path: &Path, run: &Run, results: &[TestResult]) -> anyhow::Result<()> {
    write_json(path, &json!({ "run": run, "tests": results }))
}

fn read_json(path: &Path) -> anyhow::Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_json(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut json = serde_json::to_string_pretty(value)?;
    json.push('\n');
    fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))
}
