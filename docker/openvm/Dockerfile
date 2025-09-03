ARG BASE_IMAGE_TAG=ere-base:latest

FROM ${BASE_IMAGE_TAG}

# The ere-base image provides Rust, Cargo, and common tools.
# We operate as root for SDK installation.

# If current environment is in CI or not.
ARG CI

# Install CUDA Toolkit 12.9
# If argument `CI` is set, we skip the installation.
RUN [ -n "$CI" ] || \
    (wget https://developer.download.nvidia.com/compute/cuda/repos/$(. /etc/os-release && echo "${ID}${VERSION_ID}" | tr -d '.')/$(uname -i)/cuda-keyring_1.1-1_all.deb && \
    dpkg -i cuda-keyring_1.1-1_all.deb && \
    rm cuda-keyring_1.1-1_all.deb && \
    apt update && \
    apt install -y cuda-toolkit-12-9 && \
    apt-get clean && rm -rf /var/lib/apt/lists/*)

# Add nvcc to path
ENV PATH=/usr/local/cuda/bin:$PATH

# Default to build for RTX 50 series
ARG CUDA_ARCH=120
ENV CUDA_ARCH=${CUDA_ARCH}

# Copy the OpenVM SDK installer script from the workspace context
COPY scripts/sdk_installers/install_openvm_sdk.sh /tmp/install_openvm_sdk.sh
RUN chmod +x /tmp/install_openvm_sdk.sh

# Add `rust-src` component to enable std build for nightly rust.
RUN rustup +nightly component add rust-src

# Run the OpenVM SDK installation script.
# This script installs a specific toolchain
# and installs cargo-openvm.
RUN /tmp/install_openvm_sdk.sh && rm /tmp/install_openvm_sdk.sh

# Verify cargo-openvm is accessible with the correct toolchain
RUN cargo openvm --version

CMD ["/bin/bash"]
