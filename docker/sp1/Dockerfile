ARG BASE_IMAGE_TAG=latest

# Build guest-compiler binary
FROM rust:1.85 AS builder
RUN apt-get update && apt-get install -y build-essential libclang-dev
WORKDIR /guest-compiler
COPY . .
RUN cargo build --release -p sp1-guest-compiler

# Build zkVM builder image
FROM ere-base:${BASE_IMAGE_TAG}

ARG USERNAME=ere_user

# Ensure Cargo/Rustup environment variables are set from the base image for SDK script
# TODO: These should be inherited from ere-base.
ENV RUSTUP_HOME=${RUSTUP_HOME:-/usr/local/rustup} \
    CARGO_HOME=${CARGO_HOME:-/usr/local/cargo} \
    PATH=${PATH:-/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin}

# Copy the SP1 SDK installer script
COPY scripts/sdk_installers/install_sp1_sdk.sh /tmp/install_sp1_sdk.sh
RUN chmod +x /tmp/install_sp1_sdk.sh

# TODO: Check the sp1up script to see if most of the below path configs are needed
# Define where SP1 SDK will be installed within the image.
# The install_sp1_sdk.sh script will respect these ENV variables.
# TODO: we are hardcoding /root which may not work for other users
ENV SP1UP_HOME="/root/.sp1up" \
    SP1UP_SDK_INSTALL_VERSION="v5.0.8" \
    SP1_HOME="/root/.sp1"

# Run the SP1 SDK installation script
# It will use the SP1UP_HOME and SP1_HOME defined above.
RUN /tmp/install_sp1_sdk.sh && rm /tmp/install_sp1_sdk.sh # Clean up script

# Update the image's persistent PATH to include SP1 binaries.
# This uses the SP1UP_HOME and SP1_HOME defined above.
ENV PATH="${SP1UP_HOME}/bin:${SP1_HOME}/bin:$PATH"

# Verify SP1 installation (optional here, as script does it, but good for sanity)
RUN cargo prove --version

# Copy guest compiler binary
COPY --from=builder /guest-compiler/target/release/sp1-guest-compiler /guest-compiler/guest-compiler
WORKDIR /guest-compiler

CMD ["/bin/bash"] 

# TODO:  Maybe we use root to install it in ere_user and then switch back to ere_user for security
# USER ${USERNAME} # Switch to non-root user again