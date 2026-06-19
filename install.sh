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

# 2. Get the latest release version from GitHub API (silent curl is fine here to fetch tag)
LATEST_RELEASE_URL="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest"
VERSION=$(curl -sSfL "$LATEST_RELEASE_URL" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
  echo "❌ Error: Could not determine latest release version."
  exit 1
fi
echo "🔍 Found latest version: $VERSION"

# 3. Download the pre-compiled archive (using default curl progress meter showing MB/KB speed)
DOWNLOAD_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/$VERSION/naclac-$TARGET.tar.gz"
TEMP_DIR=$(mktemp -d)
archive="$TEMP_DIR/naclac.tar.gz"

echo "📥 Downloading pre-compiled binary..."
curl -fL "$DOWNLOAD_URL" -o "$archive"

# 4. Extract and Install
echo "🚚 Extracting binary to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
tar -xzf "$archive" -C "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/naclac"

# Clean up
rm -rf "$TEMP_DIR"

# 5. Automatically configure PATH
shell_profile=""
if [ -n "$SHELL" ]; then
    shell_name=$(basename "$SHELL")
    if [ "$shell_name" = "zsh" ] && [ -f "$HOME/.zshrc" ]; then
        shell_profile="$HOME/.zshrc"
    elif [ "$shell_name" = "bash" ] && [ -f "$HOME/.bashrc" ]; then
        shell_profile="$HOME/.bashrc"
    fi
fi

if [ -z "$shell_profile" ]; then
    if [ -f "$HOME/.bashrc" ]; then
        shell_profile="$HOME/.bashrc"
    elif [ -f "$HOME/.zshrc" ]; then
        shell_profile="$HOME/.zshrc"
    elif [ -f "$HOME/.profile" ]; then
        shell_profile="$HOME/.profile"
    fi
fi

path_added=false
if [ -n "$shell_profile" ]; then
    # check if it already contains the naclac directory
    if ! grep -q "naclac/bin" "$shell_profile"; then
        echo "" >> "$shell_profile"
        echo 'export PATH="$PATH:'"$INSTALL_DIR"'"' >> "$shell_profile"
        path_added=true
        echo "✅ Added PATH configurations to $shell_profile"
    else
        echo "ℹ️  PATH configuration already present in $shell_profile"
    fi
fi

echo ""
echo "✅ naclac CLI installed successfully at: $INSTALL_DIR/naclac"

if [ "$path_added" = true ]; then
    echo ""
    echo "To update your PATH in this session, run:"
    echo "  source $shell_profile"
    echo "  hash -r"
fi
