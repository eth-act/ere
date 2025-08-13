ARG BASE_IMAGE_TAG=ere-base:latest

FROM ${BASE_IMAGE_TAG}

# The ere-base image provides Rust, Cargo (with a default nightly), and common tools.
# We operate as root for SDK installation.

# Copy the Jolt SDK (CLI) installer script from the workspace context
COPY scripts/sdk_installers/install_jolt_sdk.sh /tmp/install_jolt_sdk.sh
RUN chmod +x /tmp/install_jolt_sdk.sh

# Run the Jolt CLI installation script.
# This script installs the 'jolt' binary to $CARGO_HOME/bin.
RUN /tmp/install_jolt_sdk.sh && rm /tmp/install_jolt_sdk.sh # Clean up the script

# The jolt CLI is installed in $CARGO_HOME/bin, which is already in PATH from ere-base.

# Verify jolt CLI is accessible.
RUN jolt --version

CMD ["/bin/bash"]
