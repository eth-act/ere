use crate::{Risc0Program, SDK_VERSION};
use bonsai_sdk::blocking::Client;
use risc0_zkvm::{Receipt, VERSION, serde::to_vec};
use std::{
    env,
    ffi::OsStr,
    io::{self, Write},
    process::{Command, Output, Stdio},
    time::Duration,
};
use tempfile::tempdir;
use zkvm_interface::{Input, InputItem, zkVMError};

const URL: &str = "http://localhost:8081";
const KEY: &str = "";

// Copied and modified from https://github.com/risc0/risc0/blob/main/bento/crates/bento-client/src/bento_cli.rs.
pub fn prove(program: &Risc0Program, inputs: &Input) -> Result<(Receipt, Duration), zkVMError> {
    let client = Client::from_parts(URL.to_string(), KEY.to_string(), VERSION)
        .map_err(|err| zkVMError::Other(err.into()))?;

    // Serialize `inputs` in the same way `ExecutorEnv` does.
    let mut input_bytes = Vec::new();
    for input in inputs.iter() {
        match input {
            InputItem::Object(obj) => {
                input_bytes.extend(bytemuck::cast_slice(&to_vec(obj).unwrap()));
            }
            InputItem::SerializedObject(bytes) => {
                input_bytes.extend(bytes);
            }
            InputItem::Bytes(bytes) => {
                input_bytes.extend((bytes.len() as u32).to_le_bytes());
                input_bytes.extend(bytes);
            }
        }
    }

    client
        .upload_img(&program.image_id.to_string(), program.elf.clone())
        .map_err(|err| zkVMError::Other(err.into()))?;
    let input_id = client
        .upload_input(input_bytes)
        .map_err(|err| zkVMError::Other(err.into()))?;

    let now = std::time::Instant::now();

    let session = client
        .create_session(program.image_id.to_string(), input_id, vec![], false)
        .map_err(|err| zkVMError::Other(err.into()))?;

    loop {
        // By interval check if the proving is still running or already succeeded.
        // FIXME: The response `SessionStatusRes` has a field `elapsed_time` but
        //        currently always set to `None` because it's not implemented.
        //        So we setting 200ms to not make the proving time measurement too
        //        inaccurate, but if `RUST_LOG=debug` is set, we should be able to do
        //        `docker log {container} --since` and grep the following pattern:
        //        ```
        //        {timestamp} DEBUG workflow::tasks::resolve: Resolve complete for job_id: {session.uuid}.
        //        ```
        const INTERVAL_MILLIS: u64 = 200;

        let res = session
            .status(&client)
            .map_err(|err| zkVMError::Other(err.into()))?;

        match res.status.as_ref() {
            "RUNNING" => {
                std::thread::sleep(Duration::from_millis(INTERVAL_MILLIS));
                continue;
            }
            "SUCCEEDED" => {
                let receipt_bytes = client
                    .receipt_download(&session)
                    .map_err(|err| zkVMError::Other(err.into()))?;
                break Ok((bincode::deserialize(&receipt_bytes).unwrap(), now.elapsed()));
            }
            "FAILED" => {
                return Err(zkVMError::Other(
                    format!(
                        "Job failed with error message: {}",
                        res.error_msg.unwrap_or_default()
                    )
                    .into(),
                ));
            }
            _ => {
                return Err(zkVMError::Other(
                    format!("Unexpected proving status: {}", res.status).into(),
                ));
            }
        }
    }
}

fn cmd_output_checked(cmd: &mut Command) -> Result<Output, io::Error> {
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!("Failed to run `{cmd:?}`")));
    }
    Ok(output)
}

fn cmd_exec_checked(cmd: &mut Command) -> Result<(), io::Error> {
    let status = cmd.status()?;
    if !status.success() {
        return Err(io::Error::other(format!("Failed to run `{cmd:?}`")));
    }
    Ok(())
}

fn docker_image_exists(image: impl AsRef<OsStr>) -> Result<bool, io::Error> {
    let output = cmd_output_checked(
        Command::new("docker")
            .args(["images", "--quiet"])
            .arg(image),
    )?;
    // If image exists, image id will be printed hence stdout will be non-empty.
    Ok(!output.stdout.is_empty())
}

fn docker_image_tag(src: impl AsRef<OsStr>, dst: impl AsRef<OsStr>) -> Result<(), io::Error> {
    cmd_exec_checked(
        Command::new("docker")
            .args(["image", "tag"])
            .arg(src)
            .arg(dst),
    )
}

pub fn build_bento_images() -> Result<(), io::Error> {
    let agent_tag = format!("ere-risc0/agent:{SDK_VERSION}");
    let rest_api_tag = format!("ere-risc0/rest_api:{SDK_VERSION}");

    if docker_image_exists(&agent_tag)? && docker_image_exists(&rest_api_tag)? {
        return Ok(());
    }

    let tempdir = tempdir()?;

    cmd_exec_checked(
        Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "--branch",
                &format!("v{SDK_VERSION}"),
                "https://github.com/risc0/risc0.git",
            ])
            .arg(tempdir.path()),
    )?;

    cmd_exec_checked(
        Command::new("docker")
            .arg("compose")
            .arg("--file")
            .arg(tempdir.path().join("compose.yml"))
            .arg("--env-file")
            .arg(tempdir.path().join("bento/dockerfiles/sample.env"))
            .arg("build"),
    )?;

    docker_image_tag("agent", agent_tag)?;
    docker_image_tag("bento-rest_api", rest_api_tag)?;

    Ok(())
}

const BENTO_ENV: &str = include_str!("../../sample.env");
const BENTO_COMPOSE: &str = include_str!("../../compose.yml");
const BENTO_GPU_PROVER_AGENT_TEMPLATE: &str = include_str!("../../gpu_prover_agent.yml");

fn bento_compose_file() -> String {
    let cuda_visible_devices = env::var("CUDA_VISIBLE_DEVICES").unwrap_or_else(|_| "".to_string());
    let cuda_visible_devices = cuda_visible_devices
        .split(",")
        .flat_map(|device_id| device_id.parse::<usize>().ok())
        .collect::<Vec<_>>();

    let mut compose: serde_yaml::Mapping = serde_yaml::from_str(BENTO_COMPOSE).unwrap();
    let gpu_prover_agent: serde_yaml::Mapping =
        serde_yaml::from_str(BENTO_GPU_PROVER_AGENT_TEMPLATE).unwrap();

    if cuda_visible_devices.is_empty() {
        // If env `CUDA_VISIBLE_DEVICES` is not specified, only spin up single prover with all GPUs.
        let mut gpu_prover_agent = gpu_prover_agent.clone();
        let device = gpu_prover_agent["deploy"]["resources"]["reservations"]["devices"][0]
            .as_mapping_mut()
            .unwrap();
        device.remove("device_ids").unwrap();
        device.insert("count".into(), "all".into());
        compose["services"]
            .as_mapping_mut()
            .unwrap()
            .insert("gpu_prove_agent0".into(), gpu_prover_agent.into());
    } else {
        // Otherwise spin up provers with each having 1 GPU.
        for idx in cuda_visible_devices {
            let mut gpu_prover_agent = gpu_prover_agent.clone();
            let device = gpu_prover_agent["deploy"]["resources"]["reservations"]["devices"][0]
                .as_mapping_mut()
                .unwrap();
            device["device_ids"][0] = idx.to_string().into();
            compose["services"].as_mapping_mut().unwrap().insert(
                format!("gpu_prove_agent{idx}").into(),
                gpu_prover_agent.into(),
            );
        }
    }

    serde_yaml::to_string(&compose).unwrap()
}

/// Execute `docker compose ... {command}` with `bento_compose_file()`.
fn docker_compose_bento<I, S>(command: I) -> Result<(), io::Error>
where
    I: Clone + IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let envs = BENTO_ENV
        .lines()
        .flat_map(|line| {
            line.split_once("=")
                .map(|(key, val)| (key, env::var(key).unwrap_or_else(|_| val.to_string())))
        })
        .collect::<Vec<_>>();

    let mut child = Command::new("docker")
        .envs(envs)
        .env("RISC0_VERSION", SDK_VERSION)
        .args(["compose", "--file", "-"]) // Compose file from stdin.
        .args(command.clone())
        .stdin(Stdio::piped())
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(bento_compose_file().as_bytes())?;
    drop(stdin);

    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "Failed to spawn `docker compose --file - ${}`",
            command
                .into_iter()
                .map(|s| s.as_ref().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(" ")
        )));
    }

    Ok(())
}

/// Execute `docker compose ... up --detach` with `bento_compose_file()`.
pub fn docker_compose_bento_up() -> Result<(), io::Error> {
    docker_compose_bento(["up", "--detach", "--wait"])
}

/// Execute `docker compose ... down --volumes` with `bento_compose_file()`.
pub fn docker_compose_bento_down() -> Result<(), io::Error> {
    docker_compose_bento(["down", "--volumes"])
}
