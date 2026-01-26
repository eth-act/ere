#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Default values
ZKVM=""
IMAGE_REGISTRY=""
IMAGE_TAG=""
CACHED_IMAGE_TAG=""

usage() {
    echo "Usage: $0 --zkvm <zkvm> --registry <registry> --tag <tag> [--cached-tag <cached-tag>]"
    echo ""
    echo "Required:"
    echo "  --zkvm <zkvm>              zkVM to build for (e.g., zisk, sp1, risc0)"
    echo "  --registry <registry>      Registry prefix (e.g., ghcr.io/eth-act/ere)"
    echo "  --tag <tag>                Image tag (e.g., 0.1.3, a8d7bc0)"
    echo ""
    echo "Optional:"
    echo "  --cached-tag <cached-tag>  Cached image tag to try pulling from (skips pull if empty)"
    exit 1
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --zkvm)
            ZKVM="$2"
            shift 2
            ;;
        --registry)
            IMAGE_REGISTRY="$2"
            shift 2
            ;;
        --tag)
            IMAGE_TAG="$2"
            shift 2
            ;;
        --cached-tag)
            CACHED_IMAGE_TAG="$2"
            shift 2
            ;;
        --help|-h)
            usage
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
done

# Validate required arguments
if [ -z "$ZKVM" ]; then
    echo "Error: --zkvm is required"
    usage
fi

if [ -z "$IMAGE_REGISTRY" ]; then
    echo "Error: --registry is required"
    usage
fi

if [ -z "$IMAGE_TAG" ]; then
    echo "Error: --tag is required"
    usage
fi

BASE_IMAGE="$IMAGE_REGISTRY/ere-base:$IMAGE_TAG"
BASE_ZKVM_IMAGE="$IMAGE_REGISTRY/ere-base-$ZKVM:$IMAGE_TAG"
CACHED_BASE_IMAGE="$IMAGE_REGISTRY/ere-base:$CACHED_IMAGE_TAG"
CACHED_BASE_ZKVM_IMAGE="$IMAGE_REGISTRY/ere-base-$ZKVM:$CACHED_IMAGE_TAG"

# Pull or build ere-base and ere-base-$ZKVM locally
if [ -n "$CACHED_IMAGE_TAG" ] \
    && docker image pull "$CACHED_BASE_IMAGE" \
    && docker image pull "$CACHED_BASE_ZKVM_IMAGE";
then
    echo "Tagging ere-base from cache"
    docker tag "$CACHED_BASE_IMAGE" "$BASE_IMAGE"
    echo "Tagging ere-base-$ZKVM from cache"
    docker tag "$CACHED_BASE_ZKVM_IMAGE" "$BASE_ZKVM_IMAGE"
else
    echo "Building base images using build-image.sh"
    "$SCRIPT_DIR/build-image.sh" \
        --zkvm "$ZKVM" \
        --registry "$IMAGE_REGISTRY" \
        --tag "$IMAGE_TAG" \
        --base
fi
