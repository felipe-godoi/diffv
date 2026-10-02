#!/usr/bin/env bash
# ==============================================================================
# diffv Installer for macOS and Linux
# https://github.com/felipe-godoi/diffv
# ==============================================================================

set -e

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

info() {
    echo -e "${CYAN}==>${NC} ${BOLD}$1${NC}"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

warn() {
    echo -e "${YELLOW}!${NC} $1"
}

error() {
    echo -e "${RED}✗ Error:${NC} $1" >&2
    exit 1
}

# Print banner
echo -e "${BLUE}"
echo "    ____  _ ________     "
echo "   / __ \(_) __/ __/   __"
echo "  / / / / / /_/ /_ | | / /"
echo " / /_/ / / __/ __/ | |/ / "
echo "/_____/_/_/ /_/    |___/  "
echo -e "${NC}"
echo -e "${BOLD}High-Performance Terminal Diff Viewer (macOS & Linux)${NC}\n"

# Parse arguments
INSTALL_CHANNEL="stable"
DO_UNINSTALL=0

for arg in "$@"; do
    case "$arg" in
        --uninstall|-U)
            DO_UNINSTALL=1
            ;;
        --beta|-b)
            INSTALL_CHANNEL="beta"
            ;;
        --help|-h)
            echo "Usage: ./install.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -b, --beta         Install latest beta build from main branch"
            echo "  -U, --uninstall    Uninstall diffv and remove binaries and aliases"
            echo "  -h, --help         Show this help message"
            exit 0
            ;;
    esac
done

# Handle uninstall flag
if [ "$DO_UNINSTALL" -eq 1 ]; then
    info "Uninstalling diffv..."
    REMOVED=0
    for target in "$HOME/.local/bin/diffv" "$HOME/.local/bin/dv" "$HOME/.cargo/bin/diffv" "$HOME/.cargo/bin/dv" "/usr/local/bin/diffv" "/usr/local/bin/dv"; do
        if [ -f "$target" ] || [ -L "$target" ]; then
            rm -f "$target"
            success "Removed $target"
            REMOVED=1
        fi
    done
    if [ -d "$HOME/.config/diffv" ]; then
        rm -rf "$HOME/.config/diffv"
        success "Removed $HOME/.config/diffv"
    fi
    if [ "$REMOVED" -eq 1 ]; then
        success "diffv successfully uninstalled."
    else
        warn "No diffv installation found."
    fi
    exit 0
fi

# Detect OS and Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin)
        OS_NAME="macOS"
        ;;
    Linux)
        OS_NAME="Linux"
        ;;
    *)
        error "Unsupported operating system: $OS. diffv supports macOS and Linux."
        ;;
esac

case "$ARCH" in
    x86_64|amd64)
        ARCH_NAME="x86_64"
        ;;
    arm64|aarch64)
        ARCH_NAME="arm64"
        ;;
    *)
        warn "Architecture $ARCH might need to be compiled directly from source."
        ARCH_NAME="$ARCH"
        ;;
esac

info "Detected Platform: ${BOLD}$OS_NAME ($ARCH_NAME)${NC}"

# Check for Rust / Cargo toolchain
HAS_CARGO=0
if command -v cargo >/dev/null 2>&1; then
    HAS_CARGO=1
elif [ -f "$HOME/.cargo/bin/cargo" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
    HAS_CARGO=1
fi

# Determine source directory (if installer is executed from within repo clone)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
IS_LOCAL_REPO=0
if [ -f "$SCRIPT_DIR/Cargo.toml" ] && grep -q 'name = "diffv"' "$SCRIPT_DIR/Cargo.toml" 2>/dev/null; then
    IS_LOCAL_REPO=1
fi

# Determine destination bin directory
INSTALL_DIR="$HOME/.local/bin"
if [ ! -d "$INSTALL_DIR" ]; then
    mkdir -p "$INSTALL_DIR"
fi

# Build or Install
if [ "$IS_LOCAL_REPO" -eq 1 ]; then
    if [ "$HAS_CARGO" -eq 0 ]; then
        warn "Cargo was not found in your PATH."
        read -p "Would you like to install Rust & Cargo via rustup? (y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            info "Installing Rust toolchain..."
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            source "$HOME/.cargo/env"
            HAS_CARGO=1
        else
            error "Rust toolchain is required to build diffv from source. Install it at https://rustup.rs"
        fi
    fi

    info "Building diffv in release mode..."
    cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"

    BINARY_SOURCE="$SCRIPT_DIR/target/release/diffv"
    if [ ! -f "$BINARY_SOURCE" ]; then
        error "Build output $BINARY_SOURCE not found."
    fi

    info "Installing binary to $INSTALL_DIR/diffv..."
    cp "$BINARY_SOURCE" "$INSTALL_DIR/diffv"
    chmod +x "$INSTALL_DIR/diffv"
else
    # Public release install: Rust and GitHub authentication are not required.
    case "$OS" in
        Darwin) TARGET_OS="apple-darwin" ;;
        Linux) TARGET_OS="unknown-linux-gnu" ;;
    esac
    TARGET_ARCH="$ARCH_NAME"
    [ "$TARGET_ARCH" = "arm64" ] && TARGET_ARCH="aarch64"
    ASSET="diffv-$TARGET_ARCH-$TARGET_OS"
    if [ "$INSTALL_CHANNEL" = "beta" ]; then
        RELEASE_URL="https://github.com/felipe-godoi/diffv/releases/download/beta/$ASSET"
        info "Downloading latest diffv beta build (from main)..."
    else
        RELEASE_URL="https://github.com/felipe-godoi/diffv/releases/latest/download/$ASSET"
        info "Downloading latest stable diffv release..."
    fi
    DOWNLOAD_DIR=$(mktemp -d)
    trap 'rm -rf "$DOWNLOAD_DIR"' EXIT
    if curl --proto '=https' --proto-redir '=https' --connect-timeout 5 --max-time 90 -fsSL "$RELEASE_URL" -o "$DOWNLOAD_DIR/$ASSET"; then
        curl --proto '=https' --proto-redir '=https' --connect-timeout 5 --max-time 15 -fsSL "$RELEASE_URL.sha256" -o "$DOWNLOAD_DIR/$ASSET.sha256"
        if command -v sha256sum >/dev/null 2>&1; then
            (cd "$DOWNLOAD_DIR" && sha256sum -c "$ASSET.sha256")
        else
            (cd "$DOWNLOAD_DIR" && shasum -a 256 -c "$ASSET.sha256")
        fi
        STAGED_BINARY=$(mktemp "$INSTALL_DIR/.diffv-install.XXXXXX")
        cp "$DOWNLOAD_DIR/$ASSET" "$STAGED_BINARY"
        chmod +x "$STAGED_BINARY"
        mv -f "$STAGED_BINARY" "$INSTALL_DIR/diffv"
    else
        warn "No prebuilt release available for this platform; building from source."
        if [ "$HAS_CARGO" -eq 0 ]; then
            error "Install Rust from https://rustup.rs or choose a supported release binary."
        fi
        if [ "$INSTALL_CHANNEL" = "beta" ]; then
            cargo install --git https://github.com/felipe-godoi/diffv.git --branch main --locked --force
        else
            cargo install --git https://github.com/felipe-godoi/diffv.git --locked --force
        fi
        ln -sf "$HOME/.cargo/bin/diffv" "$INSTALL_DIR/diffv"
    fi
fi

# Verify binary and create alias
if [ -x "$INSTALL_DIR/diffv" ]; then
    ln -sf "$INSTALL_DIR/diffv" "$INSTALL_DIR/dv"
    VERSION="$("$INSTALL_DIR/diffv" --version 2>/dev/null || echo "diffv")"
    success "Successfully installed $VERSION to $INSTALL_DIR/diffv (alias: $INSTALL_DIR/dv)"
else
    error "Installation failed: executable not found at $INSTALL_DIR/diffv"
fi

# Check PATH
PATH_OK=0
if [[ ":$PATH:" == *":$INSTALL_DIR:"* ]]; then
    PATH_OK=1
fi

if [ "$PATH_OK" -eq 0 ]; then
    warn "$INSTALL_DIR is not currently in your \$PATH."

    SHELL_NAME="$(basename "$SHELL")"
    RC_FILE=""

    case "$SHELL_NAME" in
        zsh)
            RC_FILE="$HOME/.zshrc"
            ;;
        bash)
            if [ "$OS" = "Darwin" ]; then
                RC_FILE="$HOME/.bash_profile"
                [ ! -f "$RC_FILE" ] && RC_FILE="$HOME/.bashrc"
            else
                RC_FILE="$HOME/.bashrc"
            fi
            ;;
        fish)
            RC_FILE="$HOME/.config/fish/config.fish"
            ;;
        *)
            RC_FILE="$HOME/.profile"
            ;;
    esac

    if [ -n "$RC_FILE" ]; then
        info "Adding $INSTALL_DIR to PATH in $RC_FILE..."
        if [ "$SHELL_NAME" = "fish" ]; then
            echo "set -gx PATH \$HOME/.local/bin \$PATH" >> "$RC_FILE"
        else
            echo -e '\n# User local binaries\nexport PATH="$HOME/.local/bin:$PATH"' >> "$RC_FILE"
        fi
        success "Updated $RC_FILE"
        echo -e "${YELLOW}Please reload your shell or run:${NC} source $RC_FILE"
    fi
fi

echo ""
echo -e "${GREEN}${BOLD}🎉 Installation Complete!${NC}"
echo -e "Run ${CYAN}diffv --help${NC} to get started."
echo -e "Try ${CYAN}diffv -w${NC} for live AI companion watch mode!"
