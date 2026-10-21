#!/bin/bash
set -e

echo "thock · mechanical keyboard acoustic engine"
echo "────────────────────────────────────────────"
echo "Cloning latest repository..."
if [ -d ~/.thock-source/.git ]; then
    echo "Updating existing clone..."
    cd ~/.thock-source
    git pull
else
    git clone https://github.com/Dharmit-Parmar/Thock-for-macOS-.git ~/.thock-source
    cd ~/.thock-source
fi

echo "Compiling thock..."
cargo build --release

echo "Setting up global 'thock' command..."
mkdir -p ~/.cargo/bin
cat << 'SCRIPT' > ~/.cargo/bin/thock
#!/bin/bash
cd ~/.thock-source || {
    echo "Error: Thock directory not found."
    exit 1
}

# ── Update command ─────────────────────────────────────────────────────────────
if [ "$1" = "update" ] || [ "$1" = "--update" ] || [ "$1" = "-u" ]; then
    echo ""
    echo "thock · self-updater"
    echo "────────────────────────────────────────────"
    echo "Pulling latest changes from git..."
    git pull origin main
    echo ""
    echo "Rebuilding release binaries..."
    cargo build --release
    mkdir -p "$HOME/.cargo/bin"
    cp target/release/thock-cli "$HOME/.cargo/bin/thock-cli" 2>/dev/null || true
    if [ -d "Thock.app" ]; then
        cp target/release/thock-app Thock.app/Contents/MacOS/Thock 2>/dev/null || true
        cp target/release/thock-cli Thock.app/Contents/MacOS/thock-cli 2>/dev/null || true
    fi
    echo ""
    echo "✓ Thock updated successfully to $(git log -1 --format='%h (%cd)')."
    echo "Run 'thock' to start."
    exit 0
fi

launch_app() {
    if [ -x "./target/release/thock-app" ]; then
        exec "./target/release/thock-app" "$@"
    else
        exec cargo run --release --bin thock-app -- "$@"
    fi
}

launch_cli() {
    if [ -x "./target/release/thock-cli" ]; then
        exec "./target/release/thock-cli" "$@"
    else
        exec cargo run --release --bin thock-cli -- "$@"
    fi
}

if [ "$1" = "app" ] || [ "$1" = "gui" ] || [ "$1" = "1" ]; then
    launch_app "${@:2}"
elif [ "$1" = "cli" ] || [ "$1" = "2" ]; then
    launch_cli "${@:2}"
fi

echo ""
echo "thock · mechanical keyboard acoustic engine"
echo "────────────────────────────────────────────"
echo "1  GUI App   (desktop floating app)"
echo "2  CLI Mode  (terminal interactive)"
echo ""
read -n 1 -r -p "Press 1 or 2: " choice
echo ""

case "$choice" in
    1|a|A|gui|app)
        launch_app "$@"
        ;;
    2|c|C|cli)
        launch_cli "$@"
        ;;
    *)
        exit 0
        ;;
esac
SCRIPT
chmod +x ~/.cargo/bin/thock

# Ensure ~/.cargo/bin is in PATH for future shell sessions
if [[ ":$PATH:" != *":$HOME/.cargo/bin:"* ]]; then
    for profile in "$HOME/.zshrc" "$HOME/.bash_profile" "$HOME/.bashrc"; do
        if [ -f "$profile" ] && ! grep -q '\.cargo/bin' "$profile"; then
            echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> "$profile"
        fi
    done
fi

echo "✓ Setup complete. Run 'thock' from any terminal."
