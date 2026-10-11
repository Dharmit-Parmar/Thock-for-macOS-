#!/bin/bash
set -e

echo "thock · mechanical keyboard acoustic engine"
echo "────────────────────────────────────────────"
echo "Cloning latest repository..."
git clone https://github.com/Dharmit-Parmar/Thock-for-macOS-.git ~/.thock-source || {
    echo "Updating existing clone..."
    cd ~/.thock-source
    git pull
}

cd ~/.thock-source

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

if [ "$1" = "cli" ] || [ "$1" = "1" ]; then
    exec cargo run --release --bin thock-cli -- "${@:2}"
elif [ "$1" = "app" ] || [ "$1" = "gui" ] || [ "$1" = "2" ]; then
    exec cargo run --release --bin thock-app -- "${@:2}"
fi

echo ""
echo "thock · mechanical keyboard acoustic engine"
echo "────────────────────────────────────────────"
echo "1  CLI Mode  (terminal interactive)"
echo "2  GUI App   (desktop floating app)"
echo ""
read -p "choice [1/2]: " choice

case $choice in
    1)
        exec cargo run --release --bin thock-cli -- "$@"
        ;;
    2)
        exec cargo run --release --bin thock-app -- "$@"
        ;;
    *)
        exit 0
        ;;
esac
SCRIPT
chmod +x ~/.cargo/bin/thock

echo "✓ Setup complete. Run 'thock' from any terminal."
