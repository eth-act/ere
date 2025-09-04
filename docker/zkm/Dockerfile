# syntax=docker/dockerfile:1

ARG BASE_IMAGE_TAG=ere-base:latest

FROM ${BASE_IMAGE_TAG}

ARG USERNAME=ere_user

# Copy the ZKM SDK installer script
COPY --chmod=+x scripts/sdk_installers/install_zkm_sdk.sh /tmp/install_zkm_sdk.sh

# Define where ZKM SDK will be installed within the image.
ENV ZKM_DIR="/root/.zkm-toolchain"

# Run the ZKM SDK installation script
RUN /tmp/install_zkm_sdk.sh && rm /tmp/install_zkm_sdk.sh

# Verify ZKM installation
RUN zkmup list-available

# Add Docker CLI (multi-stage)
FROM docker:cli AS docker_cli
FROM ${BASE_IMAGE_TAG}
COPY --from=docker_cli /usr/local/bin/docker /usr/local/bin/docker

CMD ["/bin/bash"]

