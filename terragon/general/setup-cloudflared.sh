#!/bin/bash

# ABOUTME: Environment setup script for installing cloudflared tunneling tool
# ABOUTME: Compatible with Terragon Labs sandbox environment (Ubuntu 24.04)

set -e  # Exit on error

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to detect OS
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
    else
        echo "unknown"
    fi
}

# Function to detect architecture
detect_arch() {
    local arch=$(uname -m)
    case $arch in
        x86_64)
            echo "amd64"
            ;;
        aarch64|arm64)
            echo "arm64"
            ;;
        *)
            echo "unknown"
            ;;
    esac
}

OS=$(detect_os)
ARCH=$(detect_arch)

print_status "Detected OS: $OS, Architecture: $ARCH"

# Function to install cloudflared
install_cloudflared() {
    print_status "Installing Cloudflared..."
    
    if command_exists cloudflared; then
        print_warning "Cloudflared is already installed"
        cloudflared version
        return 0
    fi
    
    if [[ "$OS" == "linux" ]]; then
        # Install via package manager for Linux
        if command_exists apt-get; then
            # Add cloudflare gpg key
            sudo mkdir -p --mode=0755 /usr/share/keyrings
            curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg | sudo tee /usr/share/keyrings/cloudflare-main.gpg >/dev/null
            
            # Add repo to apt sources
            echo "deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/cloudflared.list
            
            # Update and install
            sudo apt-get update -qq
            sudo apt-get install -y cloudflared
        else
            # Direct binary download for other Linux distros
            print_status "Downloading cloudflared binary..."
            CLOUDFLARED_URL="https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${ARCH}"
            sudo wget -q -O /usr/local/bin/cloudflared "$CLOUDFLARED_URL"
            sudo chmod +x /usr/local/bin/cloudflared
        fi
    elif [[ "$OS" == "macos" ]]; then
        if command_exists brew; then
            brew install cloudflared
        else
            print_status "Downloading cloudflared binary for macOS..."
            CLOUDFLARED_URL="https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-${ARCH}.tgz"
            curl -L "$CLOUDFLARED_URL" | tar xz
            sudo mv cloudflared /usr/local/bin/
            sudo chmod +x /usr/local/bin/cloudflared
        fi
    else
        print_error "Unsupported OS for cloudflared installation"
        return 1
    fi
    
    print_success "Cloudflared installed successfully"
    cloudflared version
}

# Function to show usage examples
show_usage() {
    echo
    print_status "=== Installation Complete ==="
    echo
    echo "Cloudflared is ready to use!"
    echo
    echo "Quick tunnel examples (no account required):"
    echo "  HTTP:  cloudflared tunnel --url http://localhost:3000"
    echo "  HTTPS: cloudflared tunnel --url https://localhost:8443"
    echo "  TCP:   cloudflared tunnel --url tcp://localhost:22"
    echo
    echo "The tunnel will provide you with a public URL to access your local server."
    echo
}

# Main installation flow
main() {
    print_status "Starting cloudflared setup..."
    echo
    
    # Check for sudo access (required for installation on Linux)
    if [[ "$OS" == "linux" ]] && ! sudo -n true 2>/dev/null; then
        print_warning "This script requires sudo access for installation"
        sudo true || exit 1
    fi
    
    # Install cloudflared
    install_cloudflared
    
    # Show usage examples
    show_usage
    
    print_success "Setup complete! Cloudflared is ready to use."
}

# Run main function
main