ARG BASE_IMAGE_TAG=ere-base:latest

FROM ${BASE_IMAGE_TAG}


# Install protobuf-compiler because cargo-ziren requires
RUN apt-get update && apt-get install -y --no-install-recommends protobuf-compiler \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Install go because cargo-ziren requires (no way to turn off)
RUN curl -fsSL https://golang.org/dl/go1.23.1.linux-amd64.tar.gz | tar -C /usr/local -xzf -

ENV PATH=/usr/local/go/bin:$PATH

# Set default toolchain to nightly
RUN rustup default nightly

# Copy the ZKM SDK installer script
COPY --chmod=+x scripts/sdk_installers/install_zkm_sdk.sh /tmp/install_zkm_sdk.sh

# The install_zkm_sdk.sh script will respect these ENV variables.
ENV ZIREM_VERSION="1.1.4"

# Run the ZKM SDK installation script
RUN /tmp/install_zkm_sdk.sh && rm /tmp/install_zkm_sdk.sh

# Verify cargo-ziren is accessible
RUN echo "Verifying ZKM installation in Dockerfile ..." && cargo ziren --version

CMD ["/bin/bash"]
