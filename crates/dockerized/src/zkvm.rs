use crate::{
    base_image, base_zkvm_image,
    compiler::SerializedProgram,
    server_zkvm_image,
    util::{
        cuda::cuda_arch,
        docker::{
            DockerBuildCmd, DockerRunCmd, docker_container_exists, docker_image_exists,
            force_rebuild, stop_docker_container,
        },
        home_dir, workspace_dir,
    },
    zkVMKind,
};
use ere_server::client::{Url, zkVMClient};
use ere_zkvm_interface::{
    CommonError,
    zkvm::{
        ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind, ProverResourceType,
        PublicValues, zkVM,
    },
};
use parking_lot::RwLock;
use std::{future::Future, iter};
use tempfile::TempDir;
use tracing::{error, info};

mod error;

pub use error::Error;

/// This method builds 3 Docker images in sequence:
/// 1. `ere-base:{version}` - Base image with common dependencies
/// 2. `ere-base-{zkvm}:{version}` - zkVM-specific base image with the zkVM SDK
/// 3. `ere-server-{zkvm}:{version}` - Server image with the `ere-server` binary
///    built with the selected zkVM feature
///
/// When [`ProverResourceType::Gpu`] is selected, the image with GPU support
/// will be built and tagged with specific suffix.
///
/// Images are cached and only rebuilt if they don't exist or if the
/// `ERE_FORCE_REBUILD_DOCKER_IMAGE` environment variable is set.
fn build_server_image(zkvm_kind: zkVMKind, gpu: bool) -> Result<(), Error> {
    let workspace_dir = workspace_dir();
    let docker_dir = workspace_dir.join("docker");
    let docker_zkvm_dir = docker_dir.join(zkvm_kind.as_str());

    let force_rebuild = force_rebuild();
    let base_image = base_image(zkvm_kind, gpu);
    let base_zkvm_image = base_zkvm_image(zkvm_kind, gpu);
    let server_zkvm_image = server_zkvm_image(zkvm_kind, gpu);

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

            match zkvm_kind {
                zkVMKind::Airbender | zkVMKind::OpenVM | zkVMKind::Risc0 | zkVMKind::Zisk => {
                    if let Some(cuda_arch) = cuda_arch() {
                        cmd = cmd.build_arg("CUDA_ARCH", cuda_arch)
                    }
                }
                _ => {}
            }
        }

        cmd.exec(&workspace_dir)?;
    }

    // Build `ere-server-{zkvm_kind}`
    if force_rebuild || !docker_image_exists(&server_zkvm_image)? {
        info!("Building image {server_zkvm_image}...");

        let mut cmd = DockerBuildCmd::new()
            .file(docker_zkvm_dir.join("Dockerfile.server"))
            .tag(&server_zkvm_image)
            .build_arg("BASE_ZKVM_IMAGE", &base_zkvm_image)
            .build_arg_from_env("RUSTFLAGS");

        if gpu {
            cmd = cmd.build_arg("CUDA", "1");
        }

        cmd.exec(&workspace_dir)?;
    }

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
        resource: &ProverResourceType,
    ) -> Result<Self, Error> {
        let port = Self::PORT_OFFSET + zkvm_kind as u16;

        let name = format!("ere-server-{zkvm_kind}-{port}");
        let gpu = matches!(resource, ProverResourceType::Gpu);
        let mut cmd = DockerRunCmd::new(server_zkvm_image(zkvm_kind, gpu))
            .rm()
            .inherit_env("RUST_LOG")
            .inherit_env("NO_COLOR")
            .publish(port.to_string(), port.to_string())
            .name(&name);

        // zkVM specific options
        cmd = match zkvm_kind {
            zkVMKind::Risc0 => cmd
                .inherit_env("RISC0_SEGMENT_PO2")
                .inherit_env("RISC0_KECCAK_PO2"),
            // ZisK uses shared memory to exchange data between processes, it
            // requires at least 8G shared memory, here we set 16G for safety.
            zkVMKind::Zisk => cmd
                .option("shm-size", "16G")
                .option("ulimit", "memlock=-1:-1")
                .inherit_env("ZISK_PORT")
                .inherit_env("ZISK_CHUNK_SIZE_BITS")
                .inherit_env("ZISK_UNLOCK_MAPPED_MEMORY")
                .inherit_env("ZISK_MINIMAL_MEMORY")
                .inherit_env("ZISK_PREALLOCATE")
                .inherit_env("ZISK_SHARED_TABLES")
                .inherit_env("ZISK_MAX_STREAMS")
                .inherit_env("ZISK_NUMBER_THREADS_WITNESS")
                .inherit_env("ZISK_MAX_WITNESS_STORED"),
            _ => cmd,
        };

        // zkVM specific options when using GPU
        if gpu {
            cmd = match zkvm_kind {
                zkVMKind::Airbender => cmd.gpus("all"),
                zkVMKind::OpenVM => cmd.gpus("all"),
                // SP1 runs docker command to spin up the server to do GPU
                // proving, to give the client access to the prover service, we
                // need to use the host networking driver.
                zkVMKind::SP1 => cmd.mount_docker_socket().network("host"),
                zkVMKind::Risc0 => cmd.gpus("all").inherit_env("RISC0_DEFAULT_PROVER_NUM_GPUS"),
                zkVMKind::Zisk => cmd.gpus("all"),
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

        let endpoint = Url::parse(&format!("http://127.0.0.1:{port}")).unwrap();
        let client = block_on(zkVMClient::new(endpoint))?;

        Ok(ServerContainer {
            name,
            tempdir,
            client,
        })
    }
}

pub struct DockerizedzkVM {
    zkvm_kind: zkVMKind,
    program: SerializedProgram,
    resource: ProverResourceType,
    container: RwLock<Option<ServerContainer>>,
}

impl DockerizedzkVM {
    pub fn new(
        zkvm_kind: zkVMKind,
        program: SerializedProgram,
        resource: ProverResourceType,
    ) -> Result<Self, Error> {
        build_server_image(zkvm_kind, matches!(resource, ProverResourceType::Gpu))?;

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

    pub fn resource(&self) -> &ProverResourceType {
        &self.resource
    }

    fn with_retry<T, F>(&self, mut f: F) -> anyhow::Result<T>
    where
        F: FnMut(&zkVMClient) -> Result<T, ere_server::client::Error>,
    {
        const MAX_RETRY: usize = 3;

        let mut attempt = 1;
        loop {
            let err = match f(&self.container.read().as_ref().unwrap().client) {
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

            let mut container = self.container.write();
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
    fn execute(&self, input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let input = input.to_vec();
        self.with_retry(|client| block_on(client.execute(input.clone())))
    }

    fn prove(
        &self,
        input: &[u8],
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        let input = input.to_vec();
        self.with_retry(|client| block_on(client.prove(input.clone(), proof_kind)))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        let proof = proof.clone();
        self.with_retry(|client| block_on(client.verify(&proof)))
    }

    fn name(&self) -> &'static str {
        self.zkvm_kind.as_str()
    }

    fn sdk_version(&self) -> &'static str {
        self.zkvm_kind.sdk_version()
    }
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => tokio::runtime::Runtime::new().unwrap().block_on(future),
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
    use ere_test_utils::{host::*, program::basic::BasicProgramInput};
    use ere_zkvm_interface::zkvm::{ProofKind, ProverResourceType, zkVM};

    fn zkvm(
        zkvm_kind: zkVMKind,
        compiler_kind: CompilerKind,
        program: &'static str,
    ) -> DockerizedzkVM {
        let program = compile(zkvm_kind, compiler_kind, program).clone();
        DockerizedzkVM::new(zkvm_kind, program, ProverResourceType::Cpu).unwrap()
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
            [BasicProgramInput::valid().into_output_sha256()],
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod jolt {
        use super::*;
        test!(
            Jolt,
            RustCustomized,
            "basic",
            [BasicProgramInput::valid()],
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod nexus {
        use super::*;
        test!(
            Nexus,
            Rust,
            "basic",
            [BasicProgramInput::valid()],
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod openvm {
        use super::*;
        test!(
            OpenVM,
            RustCustomized,
            "basic",
            [BasicProgramInput::valid().into_output_sha256()],
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod pico {
        use super::*;
        test!(
            Pico,
            RustCustomized,
            "basic",
            [BasicProgramInput::valid()],
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod risc0 {
        use super::*;
        test!(
            Risc0,
            RustCustomized,
            "basic",
            [BasicProgramInput::valid()],
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod sp1 {
        use super::*;
        test!(
            SP1,
            RustCustomized,
            "basic",
            [BasicProgramInput::valid()],
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod ziren {
        use super::*;
        test!(
            Ziren,
            RustCustomized,
            "basic",
            [BasicProgramInput::valid()],
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }

    mod zisk {
        use super::*;
        test!(
            Zisk,
            RustCustomized,
            "basic_rust",
            [BasicProgramInput::valid().into_output_sha256()],
            [Vec::new(), BasicProgramInput::invalid().serialized_input()]
        );
    }
}
