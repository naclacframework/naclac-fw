#!/bin/sh
set -e

# Configuration
REPO_OWNER="naclacframework"
REPO_NAME="naclac-fw"
INSTALL_DIR="$HOME/.local/share/naclac/bin"

# 1. Platform Detection
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)
    TARGET="x86_64-unknown-linux-gnu"
    ;;
  Darwin)
    if [ "$ARCH" = "arm64" ]; then
      TARGET="aarch64-apple-darwin"
    else
      TARGET="x86_64-apple-darwin"
    fi
    ;;
  *)
    echo "❌ Error: Unsupported OS: $OS"
    exit 1
    ;;
esac

# 2. Get the latest release version from GitHub API
echo "🔍 Finding latest release..."
LATEST_RELEASE_URL="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest"
VERSION=$(curl -sSfL "$LATEST_RELEASE_URL" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
  echo "❌ Error: Could not determine latest release version."
  exit 1
fi
echo "📦 Found version $VERSION"

# 3. Download the pre-compiled archive
DOWNLOAD_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/$VERSION/naclac-$TARGET.tar.gz"
TEMP_DIR=$(mktemp -d)
archive="$TEMP_DIR/naclac.tar.gz"

echo "📥 Downloading pre-compiled binary..."
curl -#fL "$DOWNLOAD_URL" -o "$archive"

# 4. Extract and Install
echo "🚚 Installing to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
tar -xzf "$archive" -C "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/naclac"

# Clean up
rm -rf "$TEMP_DIR"

# 5. Success instructions
echo "✅ naclac CLI installed successfully!"
echo ""
echo "Please add the install directory to your PATH by adding this line to your ~/.bashrc or ~/.zshrc file:"
echo "export PATH=\"\$PATH:$INSTALL_DIR\""
echo "Then, run: source ~/.bashrc (or source ~/.zshrc)"
