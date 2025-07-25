ARG BASE_IMAGE_TAG=latest

FROM ere-base:${BASE_IMAGE_TAG}

# Copy and run the Risc0 SDK installer script
COPY scripts/sdk_installers/install_risc0_sdk.sh /tmp/install_risc0_sdk.sh

# Run the script without version arguments to install latest
RUN chmod +x /tmp/install_risc0_sdk.sh && /tmp/install_risc0_sdk.sh

# Verify Risc0 installation (script also does this, but good for Dockerfile sanity)
RUN echo "Verifying Risc0 installation in Dockerfile (post-script)..." && cargo risczero --version

# Get docker for `cargo risczero build`
RUN curl -fsSL https://get.docker.com | sh
