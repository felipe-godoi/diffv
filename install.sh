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

# Asks a y/N question on the terminal; returns 0 for yes, 1 for no.
# Reads from /dev/tty, so it also works when this script is piped (curl | bash).
# Default is no: Enter, timeout, no usable terminal, $CI or --non-interactive.
can_prompt() {
    [ "$NON_INTERACTIVE" -eq 0 ] || return 1
    [ -z "${CI:-}" ] || return 1
    # The device node can be readable/writable with no controlling terminal
    # (open fails with ENXIO), so check that it actually opens.
    [ -r /dev/tty ] && [ -w /dev/tty ] && { : </dev/tty >/dev/tty; } 2>/dev/null
}

ask_yes_no() {
    can_prompt || return 1
    local reply=""
    printf '%s' "$1" >/dev/tty
    if ! read -r -n 1 -t "${DIFFV_INSTALL_PROMPT_TIMEOUT:-60}" reply </dev/tty; then
        reply=""
    fi
    printf '\n' >/dev/tty
    [[ $reply =~ ^[Yy]$ ]]
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
NON_INTERACTIVE=0
SKIP_FZF=0

for arg in "$@"; do
    case "$arg" in
        --uninstall|-U)
            DO_UNINSTALL=1
            ;;
        --beta|-b)
            INSTALL_CHANNEL="beta"
            ;;
        --nightly)
            INSTALL_CHANNEL="nightly"
            ;;
        --channel=*)
            INSTALL_CHANNEL="${arg#*=}"
            ;;
        --non-interactive)
            NON_INTERACTIVE=1
            ;;
        --no-fzf)
            SKIP_FZF=1
            ;;
        --help|-h)
            echo "Usage: ./install.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -b, --beta         Install latest beta pre-release"
            echo "      --nightly      Install latest nightly build from main branch"
            echo "      --channel <ch> Select channel: stable, beta, or nightly"
            echo "  -U, --uninstall    Uninstall diffv and remove binaries and configurations"
            echo "      --no-fzf       Do not offer the optional fzf"
            echo "      --non-interactive"
            echo "                     Never prompt; every question takes its default (no)"
            echo "  -h, --help         Show this help message"
            echo ""
            echo "fzf is optional: if it is missing, the installer asks whether to install it"
            echo "(default: no). Questions are read from the terminal (/dev/tty), so they also"
            echo "appear with 'curl ... | bash'; with no terminal (CI, containers) or when \$CI is"
            echo "set, a one-line tip is printed instead. Without fzf, diffv's search uses its"
            echo "built-in picker. Pass options through a pipe with: curl ... | bash -s -- --no-fzf"
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
        if ask_yes_no "Would you like to install Rust & Cargo via rustup? (y/N) "; then
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
    if [ "$INSTALL_CHANNEL" = "nightly" ]; then
        RELEASE_URL="https://github.com/felipe-godoi/diffv/releases/download/nightly/$ASSET"
        info "Downloading latest diffv nightly build (from main)..."
    elif [ "$INSTALL_CHANNEL" = "beta" ]; then
        RELEASE_URL="https://github.com/felipe-godoi/diffv/releases/download/beta/$ASSET"
        info "Downloading latest diffv beta build..."
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

# Verify binary
if [ -x "$INSTALL_DIR/diffv" ]; then
    rm -f "$INSTALL_DIR/dv"
    VERSION="$("$INSTALL_DIR/diffv" --version 2>/dev/null || echo "diffv")"
    success "Successfully installed $VERSION to $INSTALL_DIR/diffv"
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

# Optional extra: fzf. Never blocks or fails the installation.
FZF_URL="https://github.com/junegunn/fzf#installation"

run_as_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo "$@"
    else
        return 1
    fi
}

install_fzf() {
    if command -v brew >/dev/null 2>&1; then
        brew install fzf
    elif command -v apt-get >/dev/null 2>&1; then
        run_as_root apt-get install -y fzf
    elif command -v dnf >/dev/null 2>&1; then
        run_as_root dnf install -y fzf
    elif command -v pacman >/dev/null 2>&1; then
        run_as_root pacman -S --noconfirm fzf
    elif command -v apk >/dev/null 2>&1; then
        run_as_root apk add fzf
    else
        return 1
    fi
}

if [ "$SKIP_FZF" -eq 0 ] && ! command -v fzf >/dev/null 2>&1; then
    if can_prompt; then
        echo ""
        info "Optional: fzf"
        echo "  fzf is optional. With it, diffv's file/text search (Ctrl+p / Ctrl+f) can open in"
        echo "  fzf's full-screen fuzzy finder. Without it, diffv's built-in picker (with preview)"
        echo "  is used and search keeps working."
        if ask_yes_no "Would you like to install the optional fzf? (y/N) "; then
            info "Installing fzf..."
            if install_fzf && command -v fzf >/dev/null 2>&1; then
                success "fzf installed."
            else
                warn "Could not install fzf automatically. Install it manually: $FZF_URL"
            fi
        else
            echo "  Skipped. You can install fzf later: $FZF_URL"
        fi
    else
        echo -e "${CYAN}Tip:${NC} optional fzf not found; diffv search uses its built-in picker (install fzf for its interface: $FZF_URL)."
    fi
fi

echo ""
echo -e "${GREEN}${BOLD}🎉 Installation Complete!${NC}"
echo -e "Run ${CYAN}diffv --help${NC} to get started."
echo -e "Try ${CYAN}diffv -w${NC} for live AI companion watch mode!"
