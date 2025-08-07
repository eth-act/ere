ARG BASE_IMAGE_TAG=ere-base:latest

FROM ${BASE_IMAGE_TAG}

# Copy and run the Risc0 SDK installer script
COPY scripts/sdk_installers/install_risc0_sdk.sh /tmp/install_risc0_sdk.sh

# The install_risc0_sdk.sh script will respect these ENV variables.
ENV RISC0_CLI_VERSION="2.3.1" \
    RISC0_CPP_VERSION="2024.1.5" \
    RISC0_R0VM_VERSION="2.3.1" \
    RISC0_RUST_VERSION="1.88.0"

# Run the script without version arguments to install latest
RUN chmod +x /tmp/install_risc0_sdk.sh && /tmp/install_risc0_sdk.sh

# Verify Risc0 installation (script also does this, but good for Dockerfile sanity)
RUN echo "Verifying Risc0 installation in Dockerfile (post-script)..." && cargo risczero --version

# Get docker for `cargo risczero build`
RUN curl -fsSL https://get.docker.com | sh

CMD ["/bin/bash"]
