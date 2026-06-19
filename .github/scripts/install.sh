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
LATEST_RELEASE_URL="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest"
VERSION=$(curl -sSfL "$LATEST_RELEASE_URL" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
  echo "❌ Error: Could not determine latest release version."
  exit 1
fi
echo "🔍 Found latest version: $VERSION"

# 3. Download the pre-compiled archive with custom spinner and progress bar
DOWNLOAD_URL="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/$VERSION/naclac-$TARGET.tar.gz"
TEMP_DIR=$(mktemp -d)
archive="$TEMP_DIR/naclac.tar.gz"

# Fetch content length by following redirects
total_bytes=$(curl -sIL "$DOWNLOAD_URL" | grep -i "content-length" | tail -n 1 | awk '{print $2}' | tr -d '\r\n ')
if [ -z "$total_bytes" ]; then
    total_bytes=0
fi

# Run download in background
curl -sL "$DOWNLOAD_URL" -o "$archive" &
PID=$!

# Spinner configuration
spin="⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
spin_idx=0
bar_width=30

while kill -0 $PID 2>/dev/null; do
    if [ -f "$archive" ]; then
        current_bytes=$(wc -c < "$archive" | tr -d ' ')
    else
        current_bytes=0
    fi

    # Rotate spinner
    spin_char=$(echo "$spin" | cut -c $(( (spin_idx % 10) + 1 )))
    spin_idx=$((spin_idx + 1))

    if [ "$total_bytes" -gt 0 ]; then
        pct=$((current_bytes * 100 / total_bytes))
        filled=$((pct * bar_width / 100))
        empty=$((bar_width - filled))

        bar_str=""
        i=0
        while [ $i -lt $filled ]; do
            bar_str="${bar_str}="
            i=$((i + 1))
        done

        if [ $empty -gt 0 ]; then
            bar_str="${bar_str}>"
            empty=$((empty - 1))
        fi

        i=0
        while [ $i -lt $empty ]; do
            bar_str="${bar_str} "
            i=$((i + 1))
        done

        mb_read=$(awk -v cb="$current_bytes" 'BEGIN {printf "%.2f", cb/1048576}')
        mb_total=$(awk -v tb="$total_bytes" 'BEGIN {printf "%.2f", tb/1048576}')

        printf "\r%s 🚚 Downloading [%s] %d%% (%s MiB / %s MiB)   " "$spin_char" "$bar_str" "$pct" "$mb_read" "$mb_total"
    else
        mb_read=$(awk -v cb="$current_bytes" 'BEGIN {printf "%.2f", cb/1048576}')
        printf "\r%s 🚚 Downloading (%s MiB)..." "$spin_char" "$mb_read"
    fi

    sleep 0.1
done

# Wait for the background process to finish and check its exit code
wait $PID

# Ensure we print final 100% state
if [ "$total_bytes" -gt 0 ]; then
    bar_str=""
    i=0
    while [ $i -lt $bar_width ]; do
        bar_str="${bar_str}="
        i=$((i + 1))
    done
    mb_total=$(awk -v tb="$total_bytes" 'BEGIN {printf "%.2f", tb/1048576}')
    printf "\r✅ 🚚 Downloading [%s] 100%% (%s MiB / %s MiB)   \n" "$bar_str" "$mb_total" "$mb_total"
else
    printf "\r✅ Download complete!\n"
fi

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
