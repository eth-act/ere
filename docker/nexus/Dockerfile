ARG BASE_IMAGE_TAG=latest
FROM ere-base:${BASE_IMAGE_TAG}

# The ere-base image provides Rust, Cargo, and common tools.
# We operate as root for SDK installation.

# Copy the Nexus SDK installer script from the workspace context
COPY scripts/sdk_installers/install_nexus_sdk.sh /tmp/install_nexus_sdk.sh
RUN chmod +x /tmp/install_nexus_sdk.sh

RUN rustup default nightly-2025-06-05 && \
    rustup target add riscv32i-unknown-none-elf

# Run the Nexus SDK installation script.
# This script installs the specific Rust toolchain (nightly-2025-06-05)
# and installs cargo-nexus
# The CARGO_HOME from ere-base (e.g., /root/.cargo) will be used, and cargo-nexus will be in its bin.
RUN /tmp/install_nexus_sdk.sh && rm /tmp/install_nexus_sdk.sh # Clean up the script

# Define the Nexus toolchain for convenience in subsequent commands if needed, though cargo-nexus should use it.
ENV NEXUS_TOOLCHAIN_VERSION="nightly-2025-06-05"

# Verify Nexus installation
RUN echo "Verifying Nexus installation in Dockerfile (post-script)..." && cargo-nexus --version

# Copy the entire ere project context
# The WORKDIR is /app from the base image
WORKDIR /app
COPY . .

# Build
RUN echo "Build tests for ere-nexus library..." && \
    cargo build --tests --release -p ere-nexus

# Run tests
RUN echo "Running tests for ere-nexus library..." && \
    cargo test --release -p ere-nexus --lib -- --color always && \
    echo "Running Nexus tests Success..."

CMD ["/bin/bash"]