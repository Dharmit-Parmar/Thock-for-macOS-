#!/bin/bash
set -e

echo "⌨️  Welcome to Thock Setup!"
echo "============================="
echo "Cloning the latest repository..."
git clone https://github.com/Dharmit-Parmar/Thock-for-macOS-.git ~/.thock-source || {
    echo "Updating existing clone..."
    cd ~/.thock-source
    git pull
}

cd ~/.thock-source

echo "📦 Compiling Thock (this might take a moment)..."
cargo build --release

echo "🚀 Setting up the global 'thock' command..."
mkdir -p ~/.cargo/bin
cat << 'SCRIPT' > ~/.cargo/bin/thock
#!/bin/bash
cd ~/.thock-source || {
    echo "Error: Thock directory not found."
    exit 1
}

echo ""
echo "⌨️  Thock Menu"
echo "=================="
echo "1) CLI Mode (thock-cli) [Recommended - Lightweight]"
echo "2) GUI App (thock-app)  [Visual]"
echo ""
read -p "Enter choice [1 or 2]: " choice

case $choice in
    1)
        cargo run --release --bin thock-cli -- "$@"
        ;;
    2)
        cargo run --release --bin thock-app -- "$@"
        ;;
    *)
        echo "Invalid choice. Exiting."
        exit 1
        ;;
esac
SCRIPT
chmod +x ~/.cargo/bin/thock

echo "✅ Setup complete! You can now type 'thock' anywhere in your terminal."
