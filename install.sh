#!/usr/bin/env bash
set -euo pipefail

REPO="97460200/kubectl-dbg"
INSTALL_PATH="${INSTALL_PATH:-/usr/local/bin}"
BINARY_NAME="kubectl-dbg"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

detect_platform() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$os" in
        linux)   os="linux" ;;
        darwin)  os="darwin" ;;
        *)       error "Unsupported OS: $os (only Linux and macOS are supported)" ;;
    esac

    case "$arch" in
        x86_64|amd64)        arch="amd64" ;;
        aarch64|arm64)       arch="arm64" ;;
        *)                   error "Unsupported architecture: $arch (only amd64 and arm64 are supported)" ;;
    esac

    echo "${os}-${arch}"
}

get_latest_tag() {
    local tag
    tag="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null \
        | grep '"tag_name"' \
        | sed -E 's/.*"([^"]+)".*/\1/')"
    if [ -z "$tag" ]; then
        error "Failed to fetch latest release from GitHub. Check your network or the repo: https://github.com/${REPO}/releases"
    fi
    echo "$tag"
}

main() {
    echo ""
    echo "  kubectl-dbg Installer"
    echo "  ====================="
    echo ""

    local tag=""
    local force=false
    local version_only=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --tag|-t)    tag="$2"; shift 2 ;;
            --force|-f)  force=true; shift ;;
            --path|-p)   INSTALL_PATH="$2"; shift 2 ;;
            --version)   version_only=true; shift ;;
            --help|-h)
                echo "Usage: install.sh [OPTIONS]"
                echo ""
                echo "Options:"
                echo "  --tag, -t <tag>     Install a specific version (e.g. v3.0.0)"
                echo "  --path, -p <path>   Install path (default: /usr/local/bin)"
                echo "  --force, -f         Overwrite existing installation"
                echo "  --version           Show current installed version"
                echo "  --help, -h          Show this help"
                exit 0
                ;;
            *) error "Unknown option: $1 (use --help for usage)" ;;
        esac
    done

    if [ "$version_only" = true ]; then
        if command -v "$BINARY_NAME" &>/dev/null; then
            "$BINARY_NAME" --version 2>/dev/null || echo "installed (version unknown)"
        else
            echo "kubectl-dbg is not installed"
        fi
        exit 0
    fi

    local platform
    platform="$(detect_platform)"
    info "Detected platform: ${platform}"

    if [ -z "$tag" ]; then
        tag="$(get_latest_tag)"
        info "Latest version: ${tag}"
    else
        info "Specified version: ${tag}"
    fi

    local artifact_name="kubectl-dbg-${platform}"
    local download_url="https://github.com/${REPO}/releases/download/${tag}/${artifact_name}"

    local target="${INSTALL_PATH}/${BINARY_NAME}"
    if [ -f "$target" ] && [ "$force" = false ]; then
        warn "kubectl-dbg already exists at ${target}"
        warn "Use --force to overwrite, or --tag <version> to install a different version"
        exit 1
    fi

    info "Downloading ${artifact_name} ..."
    if ! curl -fsSL --progress-bar "$download_url" -o "/tmp/${artifact_name}"; then
        error "Download failed. Check the URL: ${download_url}"
    fi

    info "Installing to ${target} ..."
    chmod +x "/tmp/${artifact_name}"

    if [ -w "$INSTALL_PATH" ]; then
        mv "/tmp/${artifact_name}" "$target"
    else
        warn "No write permission to ${INSTALL_PATH}, using sudo..."
        sudo mv "/tmp/${artifact_name}" "$target"
    fi

    if command -v kubectl-dbg &>/dev/null; then
        echo ""
        info "Successfully installed kubectl-dbg ${tag}"
        info "Run 'kubectl-dbg --help' for usage"
        echo ""
        echo "As a kubectl plugin:"
        info "kubectl dbg <pod> --help"
        echo ""
    else
        warn "Installed to ${target}, but ${INSTALL_PATH} may not be in your PATH"
        warn "Add the following to your shell profile:"
        warn "  export PATH=\"${INSTALL_PATH}:\$PATH\""
    fi

    echo ""
}

main "$@"
