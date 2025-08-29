ARG BASE_IMAGE_TAG=ere-base:latest

FROM ${BASE_IMAGE_TAG}

# If current environment is in CI or not.
ARG CI

# Install CUDA Toolkit 12.9 and clang
# If argument `CI` is set, we skip the installation.
RUN [ -n "$CI" ] || \
    (wget https://developer.download.nvidia.com/compute/cuda/repos/$(. /etc/os-release && echo "${ID}${VERSION_ID}" | tr -d '.')/$(uname -i)/cuda-keyring_1.1-1_all.deb && \
    dpkg -i cuda-keyring_1.1-1_all.deb && \
    rm cuda-keyring_1.1-1_all.deb && \
    apt update && \
    apt install -y cuda-toolkit-12-9 clang && \
    apt-get clean && rm -rf /var/lib/apt/lists/*)

# Install protoc with same version as https://github.com/risc0/risc0/blob/v3.0.1/bento/dockerfiles/agent.dockerfile#L24-L26.
# If argument `CI` is set, we skip the installation.
RUN [ -n "$CI" ] || \
    (curl -o protoc.zip -L https://github.com/protocolbuffers/protobuf/releases/download/v31.1/protoc-31.1-linux-x86_64.zip && \
    unzip protoc.zip -d /usr/local)

# Build for L40S, RTX 40 series, RTX 50 series
ARG NVCC_APPEND_FLAGS="\
    --generate-code arch=compute_89,code=sm_89 \
    --generate-code arch=compute_120,code=sm_120"
ENV NVCC_APPEND_FLAGS=${NVCC_APPEND_FLAGS}

# Add nvcc to path
ENV PATH=/usr/local/cuda/bin:$PATH

# Copy and run the Risc0 SDK installer script
COPY --chmod=+x scripts/sdk_installers/install_risc0_sdk.sh /tmp/install_risc0_sdk.sh

# The install_risc0_sdk.sh script will respect these ENV variables.
ENV RISC0_VERSION="3.0.1" \
    RISC0_CPP_VERSION="2024.1.5" \
    RISC0_RUST_VERSION="1.88.0"

# Add `rust-src` component to enable std build for nightly rust.
RUN rustup +nightly component add rust-src

# Run the Risc0 SDK installation script
# It will use the RISC0_VERSION, RISC0_CPP_VERSION and RISC0_RUST_VERSION defined above.
RUN /tmp/install_risc0_sdk.sh && rm /tmp/install_risc0_sdk.sh

# Verify Risc0 installation (script also does this, but good for Dockerfile sanity)
RUN echo "Verifying Risc0 installation in Dockerfile (post-script)..." && cargo risczero --version

CMD ["/bin/bash"]
