ARG BASE_IMAGE_TAG=ere-base:latest

FROM ${BASE_IMAGE_TAG}

ARG USERNAME=ere_user

# Copy the SP1 SDK installer script
COPY --chmod=755 scripts/sdk_installers/install_sp1_sdk.sh /tmp/install_sp1_sdk.sh

# Define where SP1 SDK will be installed within the image.
# The install_sp1_sdk.sh script will respect these ENV variables.
ENV SP1_DIR="/root/.sp1" \
    SP1_VERSION="v5.2.1"

# Run the SP1 SDK installation script
# It will use the SP1_DIR and SP1_VERSION defined above.
RUN /tmp/install_sp1_sdk.sh && rm /tmp/install_sp1_sdk.sh

# Update the image's persistent PATH to include SP1 binaries.
# This uses the SP1_DIR defined above.
ENV PATH="${SP1_DIR}/bin:$PATH"

# Verify SP1 installation (optional here, as script does it, but good for sanity)
RUN cargo prove --version

# Add `rust-src` component to enable std build for nightly rust.
RUN rustup +nightly component add rust-src

# Add Docker CLI
COPY --from=docker:cli /usr/local/bin/docker /usr/local/bin/docker

CMD ["/bin/bash"]

# TODO:  Maybe we use root to install it in ere_user and then switch back to ere_user for security
# USER ${USERNAME} # Switch to non-root user again
