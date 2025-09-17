ARG BASE_IMAGE_TAG=ere-base:latest

FROM ${BASE_IMAGE_TAG}

# Set default toolchain to MSRV of Miden
RUN rustup default 1.88.0

# Miden Configuration
ENV MIDEN_VERSION="v0.17.1" \
    MIDEN_TOOLCHAIN_VERSION="1.88.0"

# Miden CLI Installation
# COPY --chmod=755 scripts/sdk_installers/install_miden_sdk.sh /tmp/
# RUN /tmp/install_miden_sdk.sh && rm /tmp/install_miden_sdk.sh

# Verify
# RUN miden-vm --version

CMD ["/bin/bash"]
