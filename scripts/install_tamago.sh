#!/bin/bash

set -e  # Exit on error

GO_BRANCH="tamago1.25.2-zkvm-dev"

# Configuration
INSTALL_DIR="$HOME/.tamago" # that can't be empty nor "/"!
TEMP_DIR=$(mktemp -d)
GO_BRANCH="${GO_BRANCH:-master}"  # Default to master, can override with env var

echo "Building Go compiler..."
echo "Installation directory: $INSTALL_DIR"
echo "Version/branch: $GO_BRANCH"
echo ""

# Clean up temp directory on exit
trap "rm -rf $TEMP_DIR" EXIT

# Check if git is installed
if ! command -v git &> /dev/null; then
    echo "Error: git is not installed. Please install git first."
    exit 1
fi

# Clone Go repository
echo "Cloning Go repository..."
cd "$TEMP_DIR"
git clone --depth 1 --branch $GO_BRANCH https://github.com/eth-act/tamago-go.git go

cd go

# Remove .git directory to save space
echo "Removing .git directory..."
rm -rf .git

# Build Go
echo "Building Go compiler..."
cd src
GOROOT_FINAL="$INSTALL_DIR" ./make.bash

# Move built Go to installation directory
echo "Installing to $INSTALL_DIR..."
cd ../..
rm -rf "$INSTALL_DIR"
mv go "$INSTALL_DIR"

echo ""
echo "Go compiler successfully built and installed to: $INSTALL_DIR"
echo ""
echo "To use this Go installation, add the following to your shell profile:"
echo ""
echo "  export GOROOT=$INSTALL_DIR"
echo "  export PATH=\$GOROOT/bin:\$PATH"
echo ""
echo "Then run: source ~/.bashrc  (or ~/.zshrc, depending on your shell)"
echo ""
echo "Verify installation with: go version"
