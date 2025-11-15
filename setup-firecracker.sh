#!/bin/bash

# Setup script for Safe Coder Firecracker dependencies
# This script downloads and installs Firecracker, kernel, and rootfs

set -e

echo "🔥 Safe Coder - Firecracker Setup Script"
echo "========================================="
echo ""

# Check if running on Linux
if [[ "$(uname -s)" != "Linux" ]]; then
    echo "❌ Error: Firecracker requires Linux"
    echo "   You are running: $(uname -s)"
    exit 1
fi

# Check for KVM
if [[ ! -e /dev/kvm ]]; then
    echo "❌ Error: /dev/kvm not found"
    echo "   Firecracker requires KVM support"
    exit 1
fi

echo "✓ Running on Linux with KVM support"
echo ""

# Detect architecture
ARCH="$(uname -m)"
if [[ "$ARCH" != "x86_64" && "$ARCH" != "aarch64" ]]; then
    echo "❌ Error: Unsupported architecture: $ARCH"
    echo "   Supported: x86_64, aarch64"
    exit 1
fi

echo "✓ Architecture: $ARCH"
echo ""

# Check if running as root for installation
if [[ $EUID -ne 0 ]]; then
    echo "⚠️  Note: Some operations require sudo privileges"
    SUDO="sudo"
else
    SUDO=""
fi

# Create temporary directory
TMP_DIR=$(mktemp -d)
cd "$TMP_DIR"

echo "📦 Step 1: Installing Firecracker"
echo "-----------------------------------"

# Download Firecracker
echo "Downloading Firecracker..."
RELEASE_URL="https://github.com/firecracker-microvm/firecracker/releases"
LATEST=$(basename $(curl -fsSLI -o /dev/null -w %{url_effective} ${RELEASE_URL}/latest))

curl -L ${RELEASE_URL}/download/${LATEST}/firecracker-${LATEST}-${ARCH}.tgz | tar -xz

# Install Firecracker
echo "Installing Firecracker to /usr/local/bin..."
$SUDO mv release-${LATEST}-${ARCH}/firecracker-${LATEST}-${ARCH} /usr/local/bin/firecracker
$SUDO chmod +x /usr/local/bin/firecracker

echo "✓ Firecracker installed: $(firecracker --version)"
echo ""

echo "📦 Step 2: Setting up VM assets"
echo "--------------------------------"

# Create directory for VM assets
$SUDO mkdir -p /var/lib/safe-coder

# Download kernel
echo "Downloading Linux kernel for VMs..."
if [[ "$ARCH" == "x86_64" ]]; then
    KERNEL_URL="https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/kernels/vmlinux.bin"
else
    KERNEL_URL="https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/aarch64/kernels/vmlinux.bin"
fi

curl -fsSL -o vmlinux "$KERNEL_URL"
$SUDO mv vmlinux /var/lib/safe-coder/vmlinux

echo "✓ Kernel installed"

# Download rootfs
echo "Downloading root filesystem for VMs..."
if [[ "$ARCH" == "x86_64" ]]; then
    ROOTFS_URL="https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/rootfs/bionic.rootfs.ext4"
else
    ROOTFS_URL="https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/aarch64/rootfs/bionic.rootfs.ext4"
fi

curl -fsSL -o rootfs.ext4 "$ROOTFS_URL"
$SUDO mv rootfs.ext4 /var/lib/safe-coder/rootfs.ext4

echo "✓ Root filesystem installed"
echo ""

# Set up KVM permissions
echo "📦 Step 3: Setting up KVM permissions"
echo "--------------------------------------"

# Check if user is in kvm group
if ! groups | grep -q kvm; then
    echo "Adding current user to 'kvm' group..."
    $SUDO usermod -aG kvm $USER
    echo "⚠️  You need to log out and log back in for group changes to take effect"
    echo "   Or run: newgrp kvm"
fi

# Set KVM device permissions
$SUDO chmod 666 /dev/kvm

echo "✓ KVM permissions configured"
echo ""

# Cleanup
cd - > /dev/null
rm -rf "$TMP_DIR"

echo "✅ Setup complete!"
echo ""
echo "Next steps:"
echo "  1. Build Safe Coder:"
echo "     cargo build --release"
echo ""
echo "  2. Configure your API key:"
echo "     ./target/release/safe-coder config --api-key YOUR_API_KEY"
echo ""
echo "  3. Start coding:"
echo "     ./target/release/safe-coder chat"
echo ""
echo "For more information, see README.md"
