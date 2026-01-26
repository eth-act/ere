#!/bin/bash

set -euo pipefail

# Default values
ZKVM=""
IMAGE_TAG=""
IMAGE_REGISTRY=""
BUILD_BASE=false
BUILD_COMPILER=false
BUILD_SERVER=false
CUDA=false
CUDA_ARCH=""
RUSTFLAGS=""

usage() {
    echo "Usage: $0 --zkvm <zkvm> --tag <tag> [--base] [--compiler] [--server] [--registry <registry>] [--cuda] [--cuda-arch <arch>] [--rustflags <flags>]"
    echo ""
    echo "Required:"
    echo "  --zkvm <zkvm>       zkVM to build for (e.g., zisk, sp1, risc0)"
    echo "  --tag <tag>         Image tag (e.g., 0.1.3, a8d7bc0, local)"
    echo ""
    echo "Image types (at least one required):"
    echo "  --base              Build the base images"
    echo "  --compiler          Build the compiler image"
    echo "  --server            Build the server image"
    echo ""
    echo "Optional:"
    echo "  --registry <reg>    Registry prefix (e.g., ghcr.io/eth-act/ere)"
    echo "  --cuda              Enable CUDA support (appends -cuda to tag)"
    echo "  --cuda-arch <arch>  Set CUDA architecture (e.g., sm_120)"
    echo "  --rustflags <flags> Pass RUSTFLAGS to build"
    exit 1
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --zkvm)
            ZKVM="$2"
            shift 2
            ;;
        --tag)
            IMAGE_TAG="$2"
            shift 2
            ;;
        --registry)
            IMAGE_REGISTRY="$2"
            shift 2
            ;;
        --base)
            BUILD_BASE=true
            shift
            ;;
        --compiler)
            BUILD_COMPILER=true
            shift
            ;;
        --server)
            BUILD_SERVER=true
            shift
            ;;
        --cuda)
            CUDA=true
            shift
            ;;
        --cuda-arch)
            CUDA_ARCH="$2"
            shift 2
            ;;
        --rustflags)
            RUSTFLAGS="$2"
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

if [ -z "$IMAGE_TAG" ]; then
    echo "Error: --tag is required"
    usage
fi

if [ "$BUILD_BASE" = false ] &&  [ "$BUILD_COMPILER" = false ] && [ "$BUILD_SERVER" = false ]; then
    echo "Error: At least one of --base, --compiler, --server is required"
    usage
fi

# Format tag with optional -cuda suffix
if [ "$CUDA" = true ]; then
    IMAGE_TAG="${IMAGE_TAG}-cuda"
fi

# Format image prefix
if [ -n "$IMAGE_REGISTRY" ]; then
    # Remove trailing slash if present
    IMAGE_REGISTRY="${IMAGE_REGISTRY%/}"
    IMAGE_PREFIX="${IMAGE_REGISTRY}/"
else
    IMAGE_PREFIX=""
fi

# Format image

BASE_IMAGE="${IMAGE_PREFIX}ere-base:${IMAGE_TAG}"
BASE_ZKVM_IMAGE="${IMAGE_PREFIX}ere-base-${ZKVM}:${IMAGE_TAG}"
COMPILER_ZKVM_IMAGE="${IMAGE_PREFIX}ere-compiler-${ZKVM}:${IMAGE_TAG}"
SERVER_ZKVM_IMAGE="${IMAGE_PREFIX}ere-server-${ZKVM}:${IMAGE_TAG}"

# Prepare build arguments

BASE_BUILD_ARGS=()
BASE_ZKVM_BUILD_ARGS=(--build-arg "BASE_IMAGE=$BASE_IMAGE")
COMPILER_ZKVM_BUILD_ARGS=(--build-arg "BASE_ZKVM_IMAGE=$BASE_ZKVM_IMAGE")
SERVER_ZKVM_BUILD_ARGS=(--build-arg "BASE_ZKVM_IMAGE=$BASE_ZKVM_IMAGE")

if [ "$CUDA" = true ]; then
    BASE_BUILD_ARGS+=(--build-arg "CUDA=1")
    BASE_ZKVM_BUILD_ARGS+=(--build-arg "CUDA=1")
    SERVER_ZKVM_BUILD_ARGS+=(--build-arg "CUDA=1")
fi

if [ -n "$CUDA_ARCH" ]; then
    BASE_ZKVM_BUILD_ARGS+=(--build-arg "CUDA_ARCH=$CUDA_ARCH")
    SERVER_ZKVM_BUILD_ARGS+=(--build-arg "CUDA_ARCH=$CUDA_ARCH")
fi

if [ -n "$RUSTFLAGS" ]; then
    BASE_ZKVM_BUILD_ARGS+=(--build-arg "RUSTFLAGS=$RUSTFLAGS")
    SERVER_ZKVM_BUILD_ARGS+=(--build-arg "RUSTFLAGS=$RUSTFLAGS")
fi

# Build images

if [ "$BUILD_BASE" = true ]; then
    echo "Building base image: $BASE_IMAGE"
    docker build \
        --file "docker/Dockerfile.base" \
        --tag "$BASE_IMAGE" \
        "${BASE_BUILD_ARGS[@]}" \
        .

    echo "Building zkvm base image: $BASE_ZKVM_IMAGE"
    docker build \
        --file "docker/${ZKVM}/Dockerfile.base" \
        --tag "$BASE_ZKVM_IMAGE" \
        "${BASE_ZKVM_BUILD_ARGS[@]}" \
        .
fi

if [ "$BUILD_COMPILER" = true ]; then
    echo "Building zkvm compiler image: $COMPILER_ZKVM_IMAGE"
    docker build \
        --file "docker/${ZKVM}/Dockerfile.compiler" \
        --tag "$COMPILER_ZKVM_IMAGE" \
        "${COMPILER_ZKVM_BUILD_ARGS[@]}" \
        .
fi

if [ "$BUILD_SERVER" = true ]; then
    echo "Building zkvm server image: $SERVER_ZKVM_IMAGE"
    docker build \
        --file "docker/${ZKVM}/Dockerfile.server" \
        --tag "$SERVER_ZKVM_IMAGE" \
        "${SERVER_ZKVM_BUILD_ARGS[@]}" \
        .
fi

echo "Build complete!"
