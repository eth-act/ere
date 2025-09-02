ARG BASE_IMAGE_TAG=ere-base:latest

FROM ${BASE_IMAGE_TAG}

# The ere-base image provides Rust, Cargo, and common tools.
# We operate as root for SDK installation.

# Copy the Pico SDK installer script from the workspace context
COPY scripts/sdk_installers/install_pico_sdk.sh /tmp/install_pico_sdk.sh
RUN chmod +x /tmp/install_pico_sdk.sh

RUN rustup default nightly

# Add `rust-src` component to enable std build for nightly rust.
RUN rustup +nightly component add rust-src

# Run the Pico SDK installation script.
# This script installs the specific Rust toolchain (nightly-2025-08-04)
# and installs pico-cli (as cargo-pico).
# The CARGO_HOME from ere-base (e.g., /root/.cargo) will be used, and cargo-pico will be in its bin.
RUN /tmp/install_pico_sdk.sh && rm /tmp/install_pico_sdk.sh # Clean up the script

# Define the Pico toolchain for convenience in subsequent commands if needed, though cargo pico should use it.
ENV PICO_TOOLCHAIN_VERSION="nightly-2025-08-04"

# Set default toolchain
RUN rustup default "$PICO_TOOLCHAIN_VERSION"

# Verify Pico installation
RUN echo "Verifying Pico installation in Dockerfile (post-script)..." && cargo pico --version

CMD ["/bin/bash"]
