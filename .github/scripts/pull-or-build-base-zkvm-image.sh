#!/bin/bash

set -e -o pipefail

ZKVM=$1
IMAGE_REGISTRY=$2
IMAGE_TAG=$3
CACHED_IMAGE_TAG=$4

BASE_IMAGE="$IMAGE_REGISTRY/ere-base:$IMAGE_TAG"
BASE_ZKVM_IMAGE="$IMAGE_REGISTRY/ere-base-$ZKVM:$IMAGE_TAG"
CACHED_BASE_IMAGE="$IMAGE_REGISTRY/ere-base:$CACHED_IMAGE_TAG"
CACHED_BASE_ZKVM_IMAGE="$IMAGE_REGISTRY/ere-base-$ZKVM:$CACHED_IMAGE_TAG"

# Pull or build ere-base locally
if docker image pull $CACHED_BASE_IMAGE; then
    echo "Tagging ere-base from cache"
    docker tag $CACHED_BASE_IMAGE $BASE_IMAGE
else
    echo "Building ere-base"
    docker build \
        --file docker/base/Dockerfile.base \
        --tag $BASE_IMAGE \
        .
fi

# Pull or build ere-base-$ZKVM locally
if docker image pull $CACHED_BASE_ZKVM_IMAGE; then
    echo "Tagging ere-base-$ZKVM from cache"
    docker tag $CACHED_BASE_ZKVM_IMAGE $BASE_ZKVM_IMAGE
else
    echo "Building ere-base-$ZKVM"
    docker build \
        --file docker/$ZKVM/Dockerfile.base \
        --tag $BASE_ZKVM_IMAGE \
        --build-arg BASE_IMAGE=$BASE_IMAGE \
        --build-arg CI=1 \
        .
fi
