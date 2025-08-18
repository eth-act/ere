ARG BASE_IMAGE_TAG=ere-base:latest

FROM ${BASE_IMAGE_TAG}

# The ere-base image provides Rust, Cargo, and common tools.
# We operate as root for SDK installation.

# Copy the Nexus SDK installer script from the workspace context
COPY scripts/sdk_installers/install_nexus_sdk.sh /tmp/install_nexus_sdk.sh
RUN chmod +x /tmp/install_nexus_sdk.sh

# Run the Nexus SDK installation script.
# This script installs the specific Rust toolchain (nightly-2025-04-06)
# and installs cargo-nexus
# The CARGO_HOME from ere-base (e.g., /root/.cargo) will be used, and cargo-nexus will be in its bin.
RUN /tmp/install_nexus_sdk.sh && rm /tmp/install_nexus_sdk.sh # Clean up the script

# Define the Nexus toolchain for convenience in subsequent commands if needed.
ENV NEXUS_TOOLCHAIN_VERSION="nightly-2025-04-06"

# Set default toolchain
RUN rustup default "$NEXUS_TOOLCHAIN_VERSION"

# Verify Nexus installation
RUN echo "Verifying Nexus installation in Dockerfile (post-script)..." && cargo-nexus --version

CMD ["/bin/bash"]
