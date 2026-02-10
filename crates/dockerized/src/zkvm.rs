use crate::{
    compiler::SerializedProgram,
    image::{base_image, base_zkvm_image, server_zkvm_image},
    util::{
        cuda::cuda_archs,
        docker::{
            DockerBuildCmd, DockerRunCmd, docker_container_exists, docker_image_exists,
            docker_pull_image, stop_docker_container,
        },
        env::{
            ERE_DOCKER_NETWORK, ERE_GPU_DEVICES, docker_network, force_rebuild_docker_image,
            image_registry,
        },
        home_dir, workspace_dir,
    },
    zkVMKind,
};
use ere_server::{
    api::twirp::reqwest::Client,
    client::{self, Url, zkVMClient},
};
use ere_zkvm_interface::{
    CommonError,
    zkvm::{
        Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind, ProverResource,
        PublicValues, zkVM,
    },
};
use std::{
    future::Future,
    iter,
    pin::Pin,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokio::{sync::RwLock, time::sleep};
use tracing::{error, info};

mod error;

pub use error::Error;

/// Applies per-zkVM CUDA architecture build args to a Docker build command.
///
/// Each zkVM expects a different format for specifying CUDA architectures:
/// - Airbender: `CUDAARCHS` (semicolon-separated, e.g. "89;120")
/// - OpenVM: `CUDA_ARCH` (comma-separated, e.g. "89,120")
/// - Risc0: `NVCC_APPEND_FLAGS` (nvcc --generate-code flags)
/// - Zisk: `CUDA_ARCH` (single largest arch, e.g. "sm_120")
fn apply_cuda_build_args(
    cmd: DockerBuildCmd,
    zkvm_kind: zkVMKind,
    cuda_archs: &str,
) -> DockerBuildCmd {
    match zkvm_kind {
        zkVMKind::Airbender => cmd.build_arg("CUDAARCHS", cuda_archs.replace(',', ";")),
        zkVMKind::OpenVM => cmd.build_arg("CUDA_ARCH", cuda_archs),
        zkVMKind::Risc0 => {
            let flags = cuda_archs
                .split(',')
                .map(|arch| format!("--generate-code arch=compute_{arch},code=sm_{arch} "))
                .collect::<String>();
            cmd.build_arg("NVCC_APPEND_FLAGS", flags.trim_end())
        }
        zkVMKind::Zisk => {
            let max_cuda_arch = cuda_archs
                .split(',')
                .filter_map(|s| s.parse::<u32>().ok())
                .max()
                .unwrap_or(120);
            cmd.build_arg("CUDA_ARCH", format!("sm_{max_cuda_arch}"))
        }
        _ => cmd,
    }
}

/// This method builds 3 Docker images in sequence:
/// 1. `ere-base:{version}` - Base image with common dependencies
/// 2. `ere-base-{zkvm}:{version}` - zkVM-specific base image with the zkVM SDK
/// 3. `ere-server-{zkvm}:{version}` - Server image with the `ere-server` binary
///    built with the selected zkVM feature
///
/// When [`ProverResource::Gpu`] is selected, the image with GPU support
/// will be built and tagged with specific suffix.
///
/// Images are cached and only rebuilt if they don't exist or if the
/// `ERE_FORCE_REBUILD_DOCKER_IMAGE` environment variable is set.
fn build_server_image(zkvm_kind: zkVMKind, gpu: bool) -> Result<(), Error> {
    let force_rebuild = force_rebuild_docker_image();
    let base_image = base_image(zkvm_kind, gpu);
    let base_zkvm_image = base_zkvm_image(zkvm_kind, gpu);
    let server_zkvm_image = server_zkvm_image(zkvm_kind, gpu);

    if !force_rebuild {
        if docker_image_exists(&server_zkvm_image)? {
            info!("Image {server_zkvm_image} exists, skip building");
            return Ok(());
        }

        if image_registry().is_some()
            && docker_pull_image(&server_zkvm_image).is_ok()
            && docker_image_exists(&server_zkvm_image)?
        {
            info!("Image {server_zkvm_image} pulled, skip building");
            return Ok(());
        }
    }

    let workspace_dir = workspace_dir()?;
    let docker_dir = workspace_dir.join("docker");
    let docker_zkvm_dir = docker_dir.join(zkvm_kind.as_str());

    // Resolve CUDA architectures once for both base-zkvm and server builds.
    let cuda_archs = if gpu { cuda_archs() } else { None };

    // Build `ere-base`
    if force_rebuild || !docker_image_exists(&base_image)? {
        info!("Building image {base_image}...");

        let mut cmd = DockerBuildCmd::new()
            .file(docker_dir.join("Dockerfile.base"))
            .tag(&base_image);

        if gpu {
            cmd = cmd.build_arg("CUDA", "1");
        }

        cmd.exec(&workspace_dir)?;
    }

    // Build `ere-base-{zkvm_kind}`
    if force_rebuild || !docker_image_exists(&base_zkvm_image)? {
        info!("Building image {base_zkvm_image}...");

        let mut cmd = DockerBuildCmd::new()
            .file(docker_zkvm_dir.join("Dockerfile.base"))
            .tag(&base_zkvm_image)
            .build_arg("BASE_IMAGE", &base_image)
            .build_arg_from_env("RUSTFLAGS");

        if gpu {
            cmd = cmd.build_arg("CUDA", "1");

            if let Some(ref cuda_archs) = cuda_archs {
                cmd = apply_cuda_build_args(cmd, zkvm_kind, cuda_archs);
            }
        }

        cmd.exec(&workspace_dir)?;
    }

    // Build `ere-server-{zkvm_kind}`
    info!("Building image {server_zkvm_image}...");

    let mut cmd = DockerBuildCmd::new()
        .file(docker_zkvm_dir.join("Dockerfile.server"))
        .tag(&server_zkvm_image)
        .build_arg("BASE_ZKVM_IMAGE", &base_zkvm_image)
        .build_arg_from_env("RUSTFLAGS");

    if gpu {
        cmd = cmd.build_arg("CUDA", "1");

        if let Some(ref cuda_archs) = cuda_archs {
            cmd = apply_cuda_build_args(cmd, zkvm_kind, cuda_archs);
        }
    }

    cmd.exec(&workspace_dir)?;

    Ok(())
}

struct ServerContainer {
    name: String,
    client: zkVMClient,
    #[allow(dead_code)]
    tempdir: TempDir,
}

impl Drop for ServerContainer {
    fn drop(&mut self) {
        if let Err(err) = stop_docker_container(&self.name) {
            error!("Failed to stop docker container: {err}");
        }
    }
}

impl ServerContainer {
    /// Offset of port used for `ere-server`.
    const PORT_OFFSET: u16 = 4174;

    fn new(
        zkvm_kind: zkVMKind,
        program: &SerializedProgram,
        resource: &ProverResource,
    ) -> Result<Self, Error> {
        let port = Self::PORT_OFFSET + zkvm_kind as u16;

        let name = format!("ere-server-{zkvm_kind}");
        let gpu = resource.is_gpu();
        let mut cmd = DockerRunCmd::new(server_zkvm_image(zkvm_kind, gpu))
            .rm()
            .inherit_env("RUST_LOG")
            .inherit_env("NO_COLOR")
            .publish(port.to_string(), port.to_string())
            .name(&name);

        let host = if let Some(network) = docker_network() {
            cmd = cmd.network(network);
            name.as_str()
        } else {
            "127.0.0.1"
        };

        // zkVM specific options
        cmd = match zkvm_kind {
            zkVMKind::Risc0 => cmd
                .inherit_env("ERE_RISC0_SEGMENT_PO2")
                .inherit_env("ERE_RISC0_KECCAK_PO2"),
            // ZisK uses shared memory to exchange data between processes, it
            // requires at least 16G shared memory, here we set 32G for safety.
            zkVMKind::Zisk => cmd
                .option("shm-size", "32G")
                .option("ulimit", "memlock=-1:-1")
                .inherit_env("ERE_ZISK_SETUP_ON_INIT")
                .inherit_env("ERE_ZISK_PORT")
                .inherit_env("ERE_ZISK_UNLOCK_MAPPED_MEMORY")
                .inherit_env("ERE_ZISK_MINIMAL_MEMORY")
                .inherit_env("ERE_ZISK_PREALLOCATE")
                .inherit_env("ERE_ZISK_SHARED_TABLES")
                .inherit_env("ERE_ZISK_MAX_STREAMS")
                .inherit_env("ERE_ZISK_NUMBER_THREADS_WITNESS")
                .inherit_env("ERE_ZISK_MAX_WITNESS_STORED")
                .inherit_env("ERE_ZISK_START_SERVER_TIMEOUT_SEC")
                .inherit_env("ERE_ZISK_SHUTDOWN_SERVER_TIMEOUT_SEC")
                .inherit_env("ERE_ZISK_PROVE_TIMEOUT_SEC"),
            _ => cmd,
        };

        // zkVM specific options when using GPU
        if gpu {
            cmd = match zkvm_kind {
                zkVMKind::Airbender => cmd.gpus(),
                zkVMKind::OpenVM => cmd.gpus(),
                // SP1 runs docker command to spin up the server to do GPU
                // proving, to give the client access to the prover service, we
                // need to use the host networking driver if env variable
                // `ERE_DOCKER_NETWORK` is not set.
                zkVMKind::SP1 => match docker_network() {
                    Some(_) => cmd.inherit_env(ERE_DOCKER_NETWORK),
                    None => cmd.network("host"),
                }
                .mount_docker_socket()
                .inherit_env("SP1_GPU_IMAGE")
                .inherit_env(ERE_GPU_DEVICES),
                zkVMKind::Risc0 => cmd.gpus().inherit_env("RISC0_DEFAULT_PROVER_NUM_GPUS"),
                zkVMKind::Zisk => cmd.gpus(),
                _ => cmd,
            }
        }

        let tempdir = TempDir::new().map_err(CommonError::tempdir)?;

        // zkVM specific options needed for proving Groth16 proof.
        cmd = match zkvm_kind {
            // Risc0 and SP1 runs docker command to prove Groth16 proof, and
            // they pass the input by mounting temporary directory. Here we
            // create a temporary directory and mount it on the top level, so
            // the volume could be shared, and override `TMPDIR` so we don't
            // need to mount the whole `/tmp`.
            zkVMKind::Risc0 => cmd
                .mount_docker_socket()
                .env("TMPDIR", tempdir.path().to_string_lossy())
                .volume(tempdir.path(), tempdir.path()),
            zkVMKind::SP1 => {
                let groth16_circuit_path = home_dir().join(".sp1").join("circuits").join("groth16");
                cmd.mount_docker_socket()
                    .env(
                        "SP1_GROTH16_CIRCUIT_PATH",
                        groth16_circuit_path.to_string_lossy(),
                    )
                    .env("TMPDIR", tempdir.path().to_string_lossy())
                    .volume(tempdir.path(), tempdir.path())
                    .volume(&groth16_circuit_path, &groth16_circuit_path)
            }
            _ => cmd,
        };

        cmd.spawn(
            iter::empty()
                .chain(["--port", &port.to_string()])
                .chain(resource.to_args()),
            &program.0,
        )?;

        let endpoint = Url::parse(&format!("http://{host}:{port}"))?;
        let http_client = Client::new();
        block_on(wait_until_healthy(&endpoint, http_client.clone()))?;

        Ok(ServerContainer {
            name,
            tempdir,
            client: zkVMClient::new(endpoint, http_client)?,
        })
    }
}

pub struct DockerizedzkVM {
    zkvm_kind: zkVMKind,
    program: SerializedProgram,
    resource: ProverResource,
    container: RwLock<Option<ServerContainer>>,
}

impl DockerizedzkVM {
    pub fn new(
        zkvm_kind: zkVMKind,
        program: SerializedProgram,
        resource: ProverResource,
    ) -> Result<Self, Error> {
        build_server_image(zkvm_kind, resource.is_gpu())?;

        let container = ServerContainer::new(zkvm_kind, &program, &resource)?;

        Ok(Self {
            zkvm_kind,
            program,
            resource,
            container: RwLock::new(Some(container)),
        })
    }

    pub fn zkvm_kind(&self) -> zkVMKind {
        self.zkvm_kind
    }

    pub fn program(&self) -> &SerializedProgram {
        &self.program
    }

    pub fn resource(&self) -> &ProverResource {
        &self.resource
    }

    pub async fn execute_async(
        &self,
        input: Input,
    ) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        self.with_retry(|client| {
            let input = input.clone();
            Box::pin(async move { client.execute(input).await })
        })
        .await
    }

    pub async fn prove_async(
        &self,
        input: Input,
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        self.with_retry(|client| {
            let input = input.clone();
            Box::pin(async move { client.prove(input, proof_kind).await })
        })
        .await
    }

    pub async fn verify_async(&self, proof: Proof) -> anyhow::Result<PublicValues> {
        self.with_retry(|client| {
            let proof = proof.clone();
            Box::pin(async move { client.verify(proof).await })
        })
        .await
    }

    async fn with_retry<T, F>(&self, f: F) -> anyhow::Result<T>
    where
        F: Fn(zkVMClient) -> Pin<Box<dyn Future<Output = Result<T, client::Error>> + Send>>,
    {
        const MAX_RETRY: usize = 3;

        let mut attempt = 1;
        loop {
            let err = match f(self.container.read().await.as_ref().unwrap().client.clone()).await {
                Ok(ok) => return Ok(ok),
                Err(err) => Error::from(err),
            };

            if matches!(&err, Error::zkVM(_))
                // Rpc error but not connection one
                || matches!(&err, Error::Rpc(err) if err.rust_error().is_none_or(|err| !err.to_lowercase().contains("connect")))
                || attempt > MAX_RETRY
            {
                return Err(err.into());
            }

            error!("Rpc failed (attempt {attempt}/{MAX_RETRY}): {err}, checking container...");

            let mut container = self.container.write().await;
            if docker_container_exists(&container.as_ref().unwrap().name).is_ok_and(|exists| exists)
            {
                info!("Container is still running, retrying...");
            } else {
                info!("Container not found, recreating...");

                drop(container.take());
                *container = Some(ServerContainer::new(
                    self.zkvm_kind,
                    &self.program,
                    &self.resource,
                )?);
            }
            attempt += 1;
        }
    }
}

impl zkVM for DockerizedzkVM {
    fn execute(&self, input: &Input) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        block_on(self.execute_async(input.clone()))
    }

    fn prove(
        &self,
        input: &Input,
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        block_on(self.prove_async(input.clone(), proof_kind))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        block_on(self.verify_async(proof.clone()))
    }

    fn name(&self) -> &'static str {
        self.zkvm_kind.as_str()
    }

    fn sdk_version(&self) -> &'static str {
        self.zkvm_kind.sdk_version()
    }
}

async fn wait_until_healthy(endpoint: &Url, http_client: Client) -> Result<(), Error> {
    const TIMEOUT: Duration = Duration::from_secs(300); // 5mins
    const INTERVAL: Duration = Duration::from_millis(500);

    let http_client = http_client.clone();
    let start = Instant::now();
    loop {
        if start.elapsed() > TIMEOUT {
            return Err(Error::ConnectionTimeout);
        }

        match http_client.get(endpoint.join("health")?).send().await {
            Ok(response) if response.status().is_success() => break Ok(()),
            _ => sleep(INTERVAL).await,
        }
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => {
            static FALLBACK_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
            FALLBACK_RT
                .get_or_init(|| tokio::runtime::Runtime::new().expect("Failed to create runtime"))
                .block_on(future)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        CompilerKind,
        compiler::test::compile,
        zkVMKind,
        zkvm::{DockerizedzkVM, Error},
    };
    use ere_test_utils::{
        host::*, io::serde::bincode::BincodeLegacy, program::basic::BasicProgram,
    };
    use ere_zkvm_interface::zkvm::{Input, ProofKind, ProverResource, zkVM};

    fn zkvm(
        zkvm_kind: zkVMKind,
        compiler_kind: CompilerKind,
        program: &'static str,
    ) -> DockerizedzkVM {
        let program = compile(zkvm_kind, compiler_kind, program).clone();
        DockerizedzkVM::new(zkvm_kind, program, ProverResource::Cpu).unwrap()
    }

    macro_rules! test {
        ($zkvm_kind:ident, $compiler_kind:ident, $program:literal, $valid_test_cases:expr, $invalid_test_cases:expr) => {
            #[test]
            fn test_execute() {
                let zkvm = zkvm(zkVMKind::$zkvm_kind, CompilerKind::$compiler_kind, $program);

                // Valid test cases
                for test_case in $valid_test_cases {
                    run_zkvm_execute(&zkvm, &test_case);
                }

                // Invalid test cases
                for input in $invalid_test_cases {
                    let err = zkvm.execute(&input).unwrap_err();
                    assert!(matches!(err.downcast::<Error>().unwrap(), Error::zkVM(_)));
                }
            }

            #[test]
            fn test_prove() {
                let zkvm = zkvm(zkVMKind::$zkvm_kind, CompilerKind::$compiler_kind, $program);

                // Valid test cases
                for test_case in $valid_test_cases {
                    run_zkvm_prove(&zkvm, &test_case);
                }

                // Invalid test cases
                for input in $invalid_test_cases {
                    let err = zkvm.prove(&input, ProofKind::default()).unwrap_err();
                    assert!(matches!(err.downcast::<Error>().unwrap(), Error::zkVM(_)));
                }
            }
        };
    }

    mod airbender {
        use super::*;
        test!(
            Airbender,
            Rust,
            "basic",
            [BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256()],
            [
                Input::new(),
                BasicProgram::<BincodeLegacy>::invalid_test_case().input()
            ]
        );
    }

    mod jolt {
        use super::*;
        test!(
            Jolt,
            RustCustomized,
            "basic",
            [BasicProgram::<BincodeLegacy>::valid_test_case()],
            [
                Input::new(),
                BasicProgram::<BincodeLegacy>::invalid_test_case().input()
            ]
        );
    }

    mod nexus {
        use super::*;
        test!(
            Nexus,
            Rust,
            "basic",
            [BasicProgram::<BincodeLegacy>::valid_test_case()],
            [
                Input::new(),
                BasicProgram::<BincodeLegacy>::invalid_test_case().input()
            ]
        );
    }

    mod openvm {
        use super::*;
        test!(
            OpenVM,
            RustCustomized,
            "basic",
            [BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256()],
            [
                Input::new(),
                BasicProgram::<BincodeLegacy>::invalid_test_case().input()
            ]
        );
    }

    mod pico {
        use super::*;
        test!(
            Pico,
            RustCustomized,
            "basic",
            [BasicProgram::<BincodeLegacy>::valid_test_case()],
            [
                Input::new(),
                BasicProgram::<BincodeLegacy>::invalid_test_case().input()
            ]
        );
    }

    mod risc0 {
        use super::*;
        test!(
            Risc0,
            RustCustomized,
            "basic",
            [BasicProgram::<BincodeLegacy>::valid_test_case()],
            [
                Input::new(),
                BasicProgram::<BincodeLegacy>::invalid_test_case().input()
            ]
        );
    }

    mod sp1 {
        use super::*;
        test!(
            SP1,
            RustCustomized,
            "basic",
            [BasicProgram::<BincodeLegacy>::valid_test_case()],
            [
                Input::new(),
                BasicProgram::<BincodeLegacy>::invalid_test_case().input()
            ]
        );
    }

    mod ziren {
        use super::*;
        test!(
            Ziren,
            RustCustomized,
            "basic",
            [BasicProgram::<BincodeLegacy>::valid_test_case()],
            [
                Input::new(),
                BasicProgram::<BincodeLegacy>::invalid_test_case().input()
            ]
        );
    }

    mod zisk {
        use super::*;
        test!(
            Zisk,
            RustCustomized,
            "basic_rust",
            [BasicProgram::<BincodeLegacy>::valid_test_case()],
            [
                Input::new(),
                BasicProgram::<BincodeLegacy>::invalid_test_case().input()
            ]
        );
    }
}
