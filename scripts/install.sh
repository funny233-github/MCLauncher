#!/bin/bash
set -euo pipefail

REPO="funny233-github/MCLauncher"
BINARY_NAME="gluon"
INSTALL_DIR="${HOME}/.local/bin"
PLATFORM=""
TAG_NAME=""
ASSET_NAME=""
DOWNLOAD_URL=""
UNINSTALL=false
TEMP_FILES=()

cleanup() { rm -f "${TEMP_FILES[@]+"${TEMP_FILES[@]}"}"; }
trap cleanup EXIT INT TERM

usage() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Install or uninstall Gluon Minecraft Launcher.

Options:
    --uninstall    Remove Gluon from the system
    -h, --help     Show this help message

Install path: ${INSTALL_DIR}/${BINARY_NAME}
EOF
}

for arg in "$@"; do
  case "$arg" in
  --uninstall) UNINSTALL=true ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    echo "Unknown option: $arg"
    usage
    exit 1
    ;;
  esac
done

info() { printf "\033[1;34m[INFO]\033[0m  %s\n" "$1"; }
warn() { printf "\033[1;33m[WARN]\033[0m  %s\n" "$1"; }
error() {
  printf "\033[1;31m[ERROR]\033[0m %s\n" "$1" >&2
  exit 1
}

check_conflict() {
  if command -v "${BINARY_NAME}" &>/dev/null; then
    local existing
    existing="$(command -v "${BINARY_NAME}")"
    if [[ "${existing}" == "${HOME}/.cargo/bin/${BINARY_NAME}" ]]; then
      error "Found ${BINARY_NAME} at ${existing} (installed via cargo). \
Run 'cargo uninstall ${BINARY_NAME}' first to avoid conflicts."
    elif [[ "${existing}" != "${INSTALL_DIR}/${BINARY_NAME}" ]]; then
      warn "Found ${BINARY_NAME} at ${existing}, which is not the script-managed path."
    fi
  fi
}

ensure_install_dir() {
  mkdir -p "${INSTALL_DIR}"
}



uninstall() {
  local target="${INSTALL_DIR}/${BINARY_NAME}"
  if [[ -f "${target}" ]]; then
    rm -f "${target}"
    info "Removed ${target}"
    info "You may also want to remove '${INSTALL_DIR}' from your PATH if it was added by this script."
  else
    warn "Gluon is not installed at ${target}"
  fi
  exit 0
}

detect_platform() {
  local os arch
  case "$(uname -s)" in
  Linux) os="linux" ;;
  Darwin) os="darwin" ;;
  *) error "Unsupported OS: $(uname -s)" ;;
  esac

  case "$(uname -m)" in
  x86_64 | amd64) arch="amd64" ;;
  aarch64 | arm64) arch="arm64" ;;
  *) error "Unsupported architecture: $(uname -m)" ;;
  esac

  PLATFORM="${os}_${arch}"
}

fetch_latest_release() {
  local api_url="https://api.github.com/repos/${REPO}/releases/latest"
  local tmp
  tmp="$(mktemp)"
  TEMP_FILES+=("${tmp}")

  if ! curl -fsSL -o "${tmp}" "${api_url}" 2>/dev/null; then
    error "Failed to fetch latest release info from GitHub."
  fi

  TAG_NAME="$(grep -m1 '"tag_name"' "${tmp}" | sed -E 's/.*"tag_name"\s*:\s*"([^"]+)".*/\1/')"
  if [[ -z "${TAG_NAME}" ]]; then
    error "Could not parse tag name from GitHub API response."
  fi

  ASSET_NAME="${BINARY_NAME}-${PLATFORM}"
  DOWNLOAD_URL="$(grep -m1 "\"browser_download_url\".*${ASSET_NAME}\"" "${tmp}" | sed -E 's/.*"browser_download_url"\s*:\s*"([^"]+)".*/\1/')"
  if [[ -z "${DOWNLOAD_URL}" ]]; then
    error "Could not find asset '${ASSET_NAME}' in latest release ${TAG_NAME}."
  fi
}

install() {
  check_conflict
  detect_platform
  fetch_latest_release
  ensure_install_dir

  local target="${INSTALL_DIR}/${BINARY_NAME}"
  local tmp_file
  tmp_file="$(mktemp)"
  TEMP_FILES+=("${tmp_file}")

  info "Downloading Gluon ${TAG_NAME} for ${PLATFORM}..."
  if ! curl -fSL --progress-bar -o "${tmp_file}" "${DOWNLOAD_URL}"; then
    error "Download failed."
  fi

  chmod +x "${tmp_file}"
  mv "${tmp_file}" "${target}"

  info "Installed Gluon to ${target}"

  if ! echo ":${PATH}:" | grep -q ":${INSTALL_DIR}:"; then
    warn "${INSTALL_DIR} is not in your PATH."
    info "Add it by running:"
    info "  echo 'export PATH=\"\${HOME}/.local/bin:\${PATH}\"' >> ~/.bashrc"
    info "  source ~/.bashrc"
  fi

  info "Run '${BINARY_NAME} --help' to get started."
}

if [[ "${UNINSTALL}" == true ]]; then
  uninstall
else
  install
fi