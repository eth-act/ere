use core::{future::Future, iter, pin::Pin, time::Duration};
use std::time::Instant;

use ere_compiler_core::Elf;
use ere_prover_core::{
    Input, ProgramExecutionReport, ProgramProvingReport, ProverResource, PublicValues,
};
use ere_server_client::{EncodedProof, reqwest::Client, url::Url, zkVMClient};
use ere_util_tokio::block_on;
use tokio::{
    sync::{RwLock, RwLockReadGuard},
    time::{sleep, timeout},
};
use tracing::{error, info, warn};

use crate::{
    image::{base_image, base_zkvm_image, server_zkvm_image},
    util::{
        cuda::cuda_archs,
        docker::{
            DockerBuildCmd, DockerRunCmd, docker_image_exists, docker_pull_image,
            docker_wait_for_exit, remove_docker_container,
        },
        env::{docker_network, force_rebuild_docker_image, image_registry},
        workspace_dir,
    },
    zkVMKind,
};

mod error;

pub use error::Error;

/// Applies per-zkVM CUDA architecture build args to a Docker build command.
///
/// Each zkVM expects a different format for specifying CUDA architectures:
/// - Airbender: `CUDAARCHS` (semicolon-separated, e.g. "89;120")
/// - OpenVM: `CUDA_ARCH` (comma-separated, e.g. "89,120")
/// - Risc0: `NVCC_APPEND_FLAGS` (nvcc --generate-code flags)
/// - Zisk: `CUDA_ARCH` (support only one CUDA architecture, e.g. "sm_120")
fn apply_cuda_build_args(
    cmd: DockerBuildCmd,
    zkvm_kind: zkVMKind,
    cuda_archs: &[u32],
) -> Result<DockerBuildCmd, Error> {
    if cuda_archs.is_empty() {
        warn!("No CUDA_ARCHS set or detected, use default value in Dockerfile");
        return Ok(cmd);
    }

    Ok(match zkvm_kind {
        zkVMKind::Airbender => {
            let value = cuda_archs
                .iter()
                .map(|arch| arch.to_string())
                .collect::<Vec<_>>()
                .join(";");
            cmd.build_arg("CUDAARCHS", value)
        }
        zkVMKind::OpenVM => {
            let value = cuda_archs
                .iter()
                .map(|arch| arch.to_string())
                .collect::<Vec<_>>()
                .join(",");
            cmd.build_arg("CUDA_ARCH", value)
        }
        zkVMKind::Risc0 => {
            let value = cuda_archs
                .iter()
                .map(|arch| format!("--generate-code arch=compute_{arch},code=sm_{arch}"))
                .collect::<Vec<_>>()
                .join(" ");
            cmd.build_arg("NVCC_APPEND_FLAGS", value)
        }
        zkVMKind::Zisk => {
            if cuda_archs.len() != 1 {
                return Err(Error::UnsupportedMultiCudaArchs(
                    zkVMKind::Zisk,
                    cuda_archs.to_vec(),
                ));
            }
            let value = format!("sm_{}", cuda_archs[0]);
            cmd.build_arg("CUDA_ARCH", value)
        }
        _ => cmd,
    })
}

/// This method builds 3 Docker images in sequence:
/// 1. `ere-base:{version}` - Base image with common dependencies
/// 2. `ere-base-{zkvm}:{version}` - zkVM-specific base image with the zkVM SDK
/// 3. `ere-server-{zkvm}:{version}` - Server image with the `ere-server` binary built with the
///    selected zkVM feature
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
    let cuda_archs = if gpu { cuda_archs() } else { vec![] };

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
            cmd = apply_cuda_build_args(cmd, zkvm_kind, &cuda_archs)?;
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
        cmd = apply_cuda_build_args(cmd, zkvm_kind, &cuda_archs)?;
    }

    cmd.exec(&workspace_dir)?;

    Ok(())
}

struct ServerContainer {
    id: String,
    client: zkVMClient,
}

impl Drop for ServerContainer {
    fn drop(&mut self) {
        if let Err(err) = remove_docker_container(&self.id) {
            error!("Failed to remove docker container: {err}");
        }
    }
}

impl ServerContainer {
    /// Offset of port used for `ere-server`.
    const PORT_OFFSET: u16 = 4174;

    fn new(zkvm_kind: zkVMKind, elf: &Elf, resource: &ProverResource) -> Result<Self, Error> {
        let name = format!("ere-server-{zkvm_kind}");
        remove_docker_container(&name)?;

        let port = Self::PORT_OFFSET + zkvm_kind as u16;

        let gpu = resource.is_gpu();
        let mut cmd = DockerRunCmd::new(server_zkvm_image(zkvm_kind, gpu))
            .inherit_env("RUST_LOG")
            .inherit_env("RUST_BACKTRACE")
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
                .inherit_env("ERE_ZISK_UNLOCK_MAPPED_MEMORY")
                .inherit_env("ERE_ZISK_MINIMAL_MEMORY")
                .inherit_env("ERE_ZISK_PREALLOCATE")
                .inherit_env("ERE_ZISK_SHARED_TABLES")
                .inherit_env("ERE_ZISK_MAX_STREAMS")
                .inherit_env("ERE_ZISK_NUMBER_THREADS_WITNESS")
                .inherit_env("ERE_ZISK_MAX_WITNESS_STORED"),
            _ => cmd,
        };

        // zkVM specific options when using GPU
        if gpu {
            cmd = match zkvm_kind {
                zkVMKind::Airbender => cmd.gpus(),
                zkVMKind::OpenVM => cmd.gpus(),
                zkVMKind::SP1 => cmd.gpus(),
                zkVMKind::Risc0 => cmd.gpus().inherit_env("RISC0_DEFAULT_PROVER_NUM_GPUS"),
                zkVMKind::Zisk => cmd.gpus(),
            }
        }

        let (_, container_id) = cmd.spawn(
            iter::empty()
                .chain(["--port", &port.to_string()])
                .chain(resource.to_args()),
            elf,
        )?;

        let endpoint = Url::parse(&format!("http://{host}:{port}"))?;
        let http_client = Client::new();
        block_on(wait_until_healthy(&endpoint, http_client.clone()))?;

        Ok(ServerContainer {
            id: container_id,
            client: zkVMClient::new(endpoint, http_client, vec![])?,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct DockerizedzkVMConfig {
    pub execute_timeout: Option<Duration>,
    pub prove_timeout: Option<Duration>,
    pub verify_timeout: Option<Duration>,
}

pub struct DockerizedzkVM {
    zkvm_kind: zkVMKind,
    elf: Elf,
    resource: ProverResource,
    config: DockerizedzkVMConfig,
    container: RwLock<Option<ServerContainer>>,
}

impl DockerizedzkVM {
    pub fn new(
        zkvm_kind: zkVMKind,
        elf: Elf,
        resource: ProverResource,
        config: DockerizedzkVMConfig,
    ) -> Result<Self, Error> {
        build_server_image(zkvm_kind, resource.is_gpu())?;

        let container = ServerContainer::new(zkvm_kind, &elf, &resource)?;

        Ok(Self {
            zkvm_kind,
            elf,
            resource,
            config,
            container: RwLock::new(Some(container)),
        })
    }

    pub fn zkvm_kind(&self) -> zkVMKind {
        self.zkvm_kind
    }

    pub fn name(&self) -> &'static str {
        self.zkvm_kind.name()
    }

    pub fn sdk_version(&self) -> &'static str {
        self.zkvm_kind.sdk_version()
    }

    pub fn elf(&self) -> &Elf {
        &self.elf
    }

    pub fn resource(&self) -> &ProverResource {
        &self.resource
    }

    pub fn execute(&self, input: &Input) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        block_on(self.execute_async(input.clone()))
    }

    pub fn prove(
        &self,
        input: &Input,
    ) -> anyhow::Result<(PublicValues, EncodedProof, ProgramProvingReport)> {
        block_on(self.prove_async(input.clone()))
    }

    pub fn verify(&self, proof: &EncodedProof) -> anyhow::Result<PublicValues> {
        block_on(self.verify_async(proof.clone()))
    }

    pub async fn execute_async(
        &self,
        input: Input,
    ) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        self.with_retry(
            |client| {
                let input = input.clone();
                Box::pin(async move { client.execute(input).await })
            },
            self.config.execute_timeout,
        )
        .await
    }

    pub async fn prove_async(
        &self,
        input: Input,
    ) -> anyhow::Result<(PublicValues, EncodedProof, ProgramProvingReport)> {
        self.with_retry(
            |client| {
                let input = input.clone();
                Box::pin(async move { client.prove(input).await })
            },
            self.config.prove_timeout,
        )
        .await
    }

    pub async fn verify_async(&self, proof: EncodedProof) -> anyhow::Result<PublicValues> {
        self.with_retry(
            |client| {
                let proof = proof.clone();
                Box::pin(async move { client.verify(proof).await })
            },
            self.config.verify_timeout,
        )
        .await
    }

    async fn with_retry<T, F>(&self, f: F, timeout_duration: Option<Duration>) -> anyhow::Result<T>
    where
        F: Fn(
            zkVMClient,
        ) -> Pin<Box<dyn Future<Output = Result<T, ere_server_client::Error>> + Send>>,
    {
        const MAX_RETRY: usize = 3;

        // Timeout to wait for container to exit when the request is not fully
        // responded, which is usually OOM killed.
        const DOCKER_WAIT_FOR_EXIT_TIMEOUT: Duration = Duration::from_secs(10);

        let mut attempt = 1;
        loop {
            if attempt > MAX_RETRY {
                anyhow::bail!("Container is not available after {MAX_RETRY} attempts");
            }

            let container = match self.container().await {
                Ok(container) => container,
                Err(err) => {
                    error!("Failed to create container (attempt {attempt}/{MAX_RETRY}): {err}");
                    attempt += 1;
                    continue;
                }
            };
            let client = container.client.clone();

            let result = match timeout_duration {
                Some(duration) => match timeout(duration, f(client)).await {
                    Ok(result) => result,
                    Err(_) => {
                        let container_id = container.id.clone();
                        drop(container);

                        let mut guard = self.container.write().await;
                        if let Some(container) = &*guard
                            && container.id == container_id
                        {
                            info!("Operation timed out, removing container...");
                            drop(guard.take())
                        }

                        return Err(Error::Timeout { timeout: duration }.into());
                    }
                },
                None => f(client).await,
            };

            let err = match result {
                Ok(ok) => return Ok(ok),
                Err(err) => Error::from(err),
            };

            if matches!(&err, Error::Rpc(_))
                && !container.client.is_healthy().await
                && let Some(exit_info) =
                    docker_wait_for_exit(&container.id, DOCKER_WAIT_FOR_EXIT_TIMEOUT).await
            {
                return Err(Error::ContainerExited {
                    container_id: container.id.clone(),
                    exit_info,
                }
                .into());
            }

            return Err(err.into());
        }
    }

    async fn container(&self) -> anyhow::Result<RwLockReadGuard<'_, ServerContainer>> {
        let guard = self.container.read().await;
        let is_healthy = match guard.as_ref() {
            Some(container) => container.client.is_healthy().await,
            None => false,
        };
        if is_healthy {
            return Ok(RwLockReadGuard::map(guard, |opt| opt.as_ref().unwrap()));
        }
        drop(guard);

        let mut guard = self.container.write().await;
        let is_healthy = match guard.as_ref() {
            Some(container) => container.client.is_healthy().await,
            None => false,
        };
        if is_healthy {
            let guard = guard.downgrade();
            return Ok(RwLockReadGuard::map(guard, |opt| opt.as_ref().unwrap()));
        }

        info!("Server not healthy, recreating...");
        drop(guard.take());
        *guard = Some(ServerContainer::new(
            self.zkvm_kind,
            &self.elf,
            &self.resource,
        )?);

        let guard = guard.downgrade();
        Ok(RwLockReadGuard::map(guard, |opt| opt.as_ref().unwrap()))
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

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use ere_prover_core::{Input, ProverResource};
    use ere_util_test::{codec::BincodeLegacy, host::TestCase, program::basic::BasicProgram};

    use crate::{
        CompilerKind, DockerizedzkVMConfig,
        compiler::tests::compile,
        prover::{DockerizedzkVM, Error},
        zkVMKind,
    };

    fn zkvm(
        zkvm_kind: zkVMKind,
        compiler_kind: CompilerKind,
        program: &'static str,
    ) -> DockerizedzkVM {
        let elf = compile(zkvm_kind, compiler_kind, program).clone();
        DockerizedzkVM::new(
            zkvm_kind,
            elf,
            ProverResource::Cpu,
            DockerizedzkVMConfig::default(),
        )
        .unwrap()
    }

    macro_rules! test {
        ($zkvm_kind:ident, $compiler_kind:ident, $program:literal, $valid_test_cases:expr, $invalid_test_cases:expr) => {
            #[tokio::test(flavor = "multi_thread")]
            async fn test_execute() {
                let zkvm = zkvm(zkVMKind::$zkvm_kind, CompilerKind::$compiler_kind, $program);

                // Valid test cases
                for test_case in $valid_test_cases {
                    let (public_values, _report) = zkvm
                        .execute(&test_case.input())
                        .expect("execute should not fail with valid input");
                    test_case.assert_output(&public_values);
                }

                // Invalid test cases
                for input in $invalid_test_cases {
                    let err = zkvm.execute(&input).unwrap_err();
                    assert!(
                        matches!(err.downcast_ref::<Error>().unwrap(), Error::zkVM(_)),
                        "Expect error variant `Error::zkVM`, got {err:?}",
                    );
                }
            }

            #[tokio::test(flavor = "multi_thread")]
            async fn test_prove() {
                let zkvm = zkvm(zkVMKind::$zkvm_kind, CompilerKind::$compiler_kind, $program);

                // Valid test cases
                for test_case in $valid_test_cases {
                    let (prover_public_values, proof, _report) = zkvm
                        .prove(&test_case.input())
                        .expect("prove should not fail with valid input");
                    let verifier_public_values = zkvm
                        .verify(&proof)
                        .expect("verify should not fail with valid input");
                    assert_eq!(prover_public_values, verifier_public_values);
                    test_case.assert_output(&verifier_public_values);
                }

                // Invalid test cases
                for input in $invalid_test_cases {
                    let err = zkvm.prove(&input).unwrap_err();
                    assert!(
                        matches!(err.downcast_ref::<Error>().unwrap(), Error::zkVM(_)),
                        "Expect error variant `Error::zkVM`, got {err:?}",
                    );
                }

                // Should be able to recover
                for test_case in $valid_test_cases {
                    let (prover_public_values, proof, _report) = zkvm
                        .prove(&test_case.input())
                        .expect("prove should not fail with valid input");
                    let verifier_public_values = zkvm
                        .verify(&proof)
                        .expect("verify should not fail with valid input");
                    assert_eq!(prover_public_values, verifier_public_values);
                    test_case.assert_output(&verifier_public_values);
                }

                // Timeout
                let mut zkvm = zkvm;
                let prove_timeout = Duration::ZERO;
                zkvm.config.prove_timeout = Some(prove_timeout);
                let err = zkvm.prove(&Input::new()).unwrap_err();
                assert!(
                    matches!(
                        err.downcast_ref::<Error>().unwrap(),
                        Error::Timeout { timeout } if *timeout == prove_timeout,
                    ),
                    "Expect error variant `Error::Timeout`, got {err:?}",
                );
                assert!(zkvm.container.write().await.is_none());
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
