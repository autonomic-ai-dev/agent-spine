#!/usr/bin/env bash
set -euo pipefail

REPO="aeswibon/agent-spine"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

detect_platform() {
    local os arch
    os="$(uname -s | tr '[:upper:]' '[:lower:]')"
    arch="$(uname -m)"

    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) echo "unsupported architecture: $arch" >&2; exit 1 ;;
    esac

    case "$os" in
        linux)
            echo "${arch}-unknown-linux-gnu"
            ;;
        darwin)
            echo "${arch}-apple-darwin"
            ;;
        mingw*|msys*|cygwin*)
            echo "${arch}-pc-windows-msvc"
            ;;
        *)
            echo "unsupported OS: $os" >&2; exit 1 ;;
    esac
}

get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" |
        grep '"tag_name":' |
        sed 's/.*"tag_name": "v//;s/".*//'
}

download_url() {
    local version="$1" target="$2"
    echo "https://github.com/${REPO}/releases/download/v${version}/agent-spine-${target}"
}

main() {
    local target version url bin_name="agent-spine"

    target="$(detect_platform)"
    echo "detected target: ${target}"

    version="${AGENT_SPINE_VERSION:-$(get_latest_version)}"
    echo "latest version: v${version}"

    url="$(download_url "${version}" "${target}")"

    if [[ "$target" == *windows* ]]; then
        bin_name="agent-spine.exe"
        url="${url}.exe"
    fi

    echo "downloading ${url} ..."
    curl -fsSL "${url}" -o "/tmp/${bin_name}"

    chmod +x "/tmp/${bin_name}"

    echo "installing to ${INSTALL_DIR}/${bin_name} ..."
    if [[ -w "${INSTALL_DIR}" ]]; then
        mv "/tmp/${bin_name}" "${INSTALL_DIR}/${bin_name}"
    else
        sudo mv "/tmp/${bin_name}" "${INSTALL_DIR}/${bin_name}"
    fi

    echo "agent-spine v${version} installed to ${INSTALL_DIR}/${bin_name}"
    agent-spine --help
}

main "$@"
