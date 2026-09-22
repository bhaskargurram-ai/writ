#!/usr/bin/env sh
# Install writ into $HOME/.writ/bin.
#
# Release mode downloads a prebuilt artifact from GitHub releases. Until the
# first release exists, this script can build from the checked-out source tree
# when WRIT_FROM_SOURCE=1 is set. The GitHub composite action uses this path
# during pre-release development.
set -eu

BIN_DIR="${WRIT_BIN_DIR:-$HOME/.writ/bin}"
mkdir -p "$BIN_DIR"

repo="${WRIT_REPO:-bhaskargurram-ai/writ}"
version="${WRIT_VERSION:-latest}"
os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"

case "$arch" in
  x86_64|amd64) arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *) echo "unsupported arch: $arch" >&2; exit 1 ;;
esac

case "$os" in
  linux) target="$arch-unknown-linux-gnu" ;;
  darwin) target="$arch-apple-darwin" ;;
  mingw*|msys*|cygwin*) target="x86_64-pc-windows-msvc" ;;
  *) echo "unsupported os: $os" >&2; exit 1 ;;
esac

if [ "${WRIT_FROM_SOURCE:-0}" = "1" ]; then
  echo "building writ from source..." >&2
  cargo build --release -p writ-cli
  cp "target/release/writ${EXE_SUFFIX:-}" "$BIN_DIR/writ${EXE_SUFFIX:-}"
else
  tag="$version"
  if [ "$tag" = "latest" ]; then
    url="https://github.com/$repo/releases/latest/download/writ-$target"
  else
    url="https://github.com/$repo/releases/download/$tag/writ-$target"
  fi
  echo "downloading $url" >&2
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$BIN_DIR/writ"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$BIN_DIR/writ" "$url"
  else
    echo "need curl or wget" >&2; exit 1
  fi
fi

chmod +x "$BIN_DIR/writ" 2>/dev/null || true
echo "installed: $BIN_DIR/writ"