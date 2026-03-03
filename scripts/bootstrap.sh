#!/usr/bin/env bash
# Idempotent bootstrap for a bare Linux VM (Codespaces, Devin, Codex, Claude
# cloud, a fresh Ubuntu box). Local mise users can skip this and run
# `mise trust && mise run setup-core` (or `setup`).
#
#   ./scripts/bootstrap.sh         # rust + ruby + vendor/rbs (check.sh)
#   ./scripts/bootstrap.sh --full  # also node, wasi-sdk, wasm, npm, Playwright
set -euo pipefail

cd "$(dirname "$0")/.."

FULL=0
case "${1:-}" in
  "" ) ;;
  --full ) FULL=1 ;;
  -h | --help )
    sed -n '2,9p' "$0"
    exit 0
    ;;
  * )
    echo "usage: $0 [--full]" >&2
    exit 2
    ;;
esac

have() { command -v "$1" >/dev/null 2>&1; }

apt_install() {
  if [ "$(id -u)" -eq 0 ]; then
    apt-get update -qq
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "$@"
  elif have sudo; then
    sudo apt-get update -qq
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "$@"
  else
    echo "error: need root or sudo to apt-get install $*" >&2
    exit 1
  fi
}

# bindgen (rbs-sys / ruby-prism-sys) needs libclang.
if ! have clang; then
  if have apt-get; then
    apt_install clang libclang-dev
  else
    echo "error: clang not found (bindgen). Install clang + libclang." >&2
    exit 1
  fi
fi

if ! have curl; then
  if have apt-get; then
    apt_install curl ca-certificates
  else
    echo "error: curl not found (needed to install mise)." >&2
    exit 1
  fi
fi

if ! have mise; then
  curl -fsSL https://mise.run | sh
  export PATH="${HOME}/.local/bin:${PATH}"
fi
if ! have mise; then
  echo "error: mise not on PATH after install (expected ${HOME}/.local/bin)" >&2
  exit 1
fi

mise trust
if [ "$FULL" -eq 1 ]; then
  mise run setup
else
  mise run setup-core
fi

echo "bootstrap ok. next: ./scripts/check.sh"
