#!/usr/bin/env bash
set -e

echo "=========================================="
echo "      Rift Voice AI - macOS Builder       "
echo "=========================================="
echo ""

# Check for Node.js
if ! command -v node \u0026> /dev/null; then
    echo "❌ Node.js is not installed. Please install Node.js 18+ from https://nodejs.org/"
    exit 1
fi

# Check for Rust
if ! command -v cargo \u0026> /dev/null; then
    echo "❌ Rust is not installed. Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo "📦 Installing npm dependencies..."
npm install

echo "🔨 Building Rift native macOS application and .dmg installer..."
npm run build

echo ""
echo "=========================================="
echo "  ✅ Build Complete!"
echo "  Your installer is ready at:"
echo "  src-tauri/target/release/bundle/dmg/"
echo "=========================================="
