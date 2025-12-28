#!/bin/bash
set -e

# Define names
CRATE_NAME="my_eq"
PLUGIN_NAME="MyEq"
TARGET_DIR="target/release"
VST3_DIR="$PLUGIN_NAME.vst3"
ARCH_DIR="Contents/x86_64-linux"

# Ensure release build exists
cargo build --release

# Create directory structure
mkdir -p "$VST3_DIR/$ARCH_DIR"

# Copy and rename the binary
cp "$TARGET_DIR/lib$CRATE_NAME.so" "$VST3_DIR/$ARCH_DIR/$PLUGIN_NAME.so"

# Create a CLAP bundle too (simple single file usually, but follows structure)
CLAP_DIR="$PLUGIN_NAME.clap"
mkdir -p "$CLAP_DIR"
cp "$TARGET_DIR/lib$CRATE_NAME.so" "$CLAP_DIR/$PLUGIN_NAME.clap"

echo "Packaged plugin into:"
echo "  - $VST3_DIR"
echo "  - $CLAP_DIR"
