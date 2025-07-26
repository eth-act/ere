ARG BASE_IMAGE_TAG=latest
FROM ere-base:${BASE_IMAGE_TAG}

# The ere-base image provides Rust, Cargo, and common tools.
# We operate as root for SDK installation.

# Copy the Pico SDK installer script from the workspace context
COPY scripts/sdk_installers/install_pico_sdk.sh /tmp/install_pico_sdk.sh
RUN chmod +x /tmp/install_pico_sdk.sh

RUN rustup default nightly

# Run the Pico SDK installation script.
# This script installs the specific Rust toolchain (nightly-2024-11-27)
# and installs pico-cli (as cargo-pico).
# The CARGO_HOME from ere-base (e.g., /root/.cargo) will be used, and cargo-pico will be in its bin.
RUN /tmp/install_pico_sdk.sh && rm /tmp/install_pico_sdk.sh # Clean up the script

# Define the Pico toolchain for convenience in subsequent commands if needed, though cargo pico should use it.
ENV PICO_TOOLCHAIN_VERSION="nightly-2024-11-27"

# Verify Pico installation
RUN echo "Verifying Pico installation in Dockerfile (post-script)..." && cargo "+${PICO_TOOLCHAIN_VERSION}" pico --version

# Copy the entire ere project context
# The WORKDIR is /app from the base image
WORKDIR /app
COPY . .

# Run tests
RUN echo "Running tests for ere-pico library..." && \
    cargo "+${PICO_TOOLCHAIN_VERSION}" test --release -p ere-pico --lib -- --color always

CMD ["/bin/bash"] 