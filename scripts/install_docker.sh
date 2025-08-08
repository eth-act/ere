#!/bin/bash

set -e

if command -v docker &> /dev/null; then
    echo "Docker is already installed."
else
    # From https://docs.docker.com/engine/install/ubuntu/#install-using-the-repository.
    # Add Docker's official GPG key:
    apt-get update
    apt-get install ca-certificates curl
    install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc
    chmod a+r /etc/apt/keyrings/docker.asc

    # Add the repository to Apt sources:
    echo \
        "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu \
        $(. /etc/os-release && echo "${UBUNTU_CODENAME:-$VERSION_CODENAME}") stable" \
        | tee /etc/apt/sources.list.d/docker.list > /dev/null
    apt-get update
    apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
fi

# From https://docs.docker.com/engine/install/linux-postinstall/#manage-docker-as-a-non-root-user.
# Use docker as a non-root user:
grep -q docker /etc/group || groupadd docker
usermod -aG docker $USER
newgrp docker
