ARG BASE_IMAGE_TAG=latest

# Build guest-compiler binary
FROM rust:1.85 AS builder
RUN apt-get update && apt-get install -y build-essential libclang-dev
WORKDIR /guest-compiler
COPY . .
RUN cargo build --release -p risc0-guest-compiler

# Build zkVM builder image
FROM ere-base:${BASE_IMAGE_TAG}

ARG USERNAME=ere_user

# Ensure Cargo/Rustup environment variables are set from the base image for SDK script
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# Copy and run the Risc0 SDK installer script
COPY scripts/sdk_installers/install_risc0_sdk.sh /tmp/install_risc0_sdk.sh
RUN chmod +x /tmp/install_risc0_sdk.sh

# Run the script without version arguments to install latest
# TODO: We need to change this in all scripts so that we can fix the version in CI
RUN /tmp/install_risc0_sdk.sh

# Verify Risc0 installation (script also does this, but good for Dockerfile sanity)
RUN echo "Verifying Risc0 installation in Dockerfile (post-script)..." && cargo risczero --version

# Get docker for `cargo risczero build`
RUN curl -fsSL https://get.docker.com | sh

# Copy guest compiler binary
COPY --from=builder /guest-compiler/target/release/risc0-guest-compiler /guest-compiler/guest-compiler
WORKDIR /guest-compiler

CMD ["/bin/bash"]
