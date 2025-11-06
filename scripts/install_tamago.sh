#!/bin/bash

set -e  # Exit on error

GO_BRANCH="tamago1.25.2-zkvm-dev"

# Configuration
INSTALL_DIR="$HOME/.tamago" # that can't be empty nor "/"!
TEMP_DIR=$(mktemp -d)
GO_BRANCH="${GO_BRANCH:-master}"  # Default to master, can override with env var

# Stock Go configuration
STOCK_GO_VERSION="go1.25.3"
STOCK_GO_DIR="$TEMP_DIR/stock-go"
STOCK_GO_INSTALL=false

echo "Building Go compiler..."
echo "Installation directory: $INSTALL_DIR"
echo "Version/branch: $GO_BRANCH"
echo ""

# Clean up temp directory on exit
trap "rm -rf $TEMP_DIR" EXIT

# Check if Go 1.25.3 is installed
echo "Checking for Go compiler..."
if command -v go &> /dev/null; then
    CURRENT_GO_VERSION=$(go version | awk '{print $3}')
    if [ "$CURRENT_GO_VERSION" = "$STOCK_GO_VERSION" ]; then
        echo "Found Go $STOCK_GO_VERSION already installed"
    else
        echo "Found Go $CURRENT_GO_VERSION, but need $STOCK_GO_VERSION"
        STOCK_GO_INSTALL=true
    fi
else
    echo "Go compiler not found, will install $STOCK_GO_VERSION"
    STOCK_GO_INSTALL=true
fi

# Install stock Go if needed
if [ "$STOCK_GO_INSTALL" = true ]; then
    echo "Installing stock Go $STOCK_GO_VERSION..."

    # Detect OS and architecture
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)

    case "$ARCH" in
        x86_64)
            ARCH="amd64"
            ;;
        aarch64|arm64)
            ARCH="arm64"
            ;;
        *)
            echo "Error: Unsupported architecture: $ARCH"
            exit 1
            ;;
    esac

    GO_ARCHIVE="${STOCK_GO_VERSION}.${OS}-${ARCH}.tar.gz"
    GO_URL="https://go.dev/dl/${GO_ARCHIVE}"

    echo "Downloading from $GO_URL..."
    cd "$TEMP_DIR"

    curl -L -O "$GO_URL"

    echo "Extracting Go archive..."
    tar -xzf "$GO_ARCHIVE"
    mv go "$STOCK_GO_DIR"

    # Add stock Go to PATH
    export PATH="$STOCK_GO_DIR/bin:$PATH"
    export GOROOT="$STOCK_GO_DIR"

    echo "Stock Go installed temporarily at $STOCK_GO_DIR"
    go version
fi

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
