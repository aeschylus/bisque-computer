#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Read version from Cargo.toml
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo "Building Bisque Computer v${VERSION}..."

# 1. Build release binary
echo "==> cargo build --release"
cargo build --release

# 2. Set up .app bundle structure
APP="target/release/BisqueComputer.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
mkdir -p "$APP/Contents/Resources"

# 3. Copy binary
cp target/release/bisque-computer "$APP/Contents/MacOS/bisque-computer"

# 4. Copy app icon
if [ -f assets/AppIcon.icns ]; then
    cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
else
    echo "Warning: assets/AppIcon.icns not found, skipping icon"
fi

# 5. Find and copy whisper model from cargo build artifacts
MODEL_PATH=$(find target/release/build/bisque-computer-*/out -name "ggml-base.en.bin" 2>/dev/null | head -1)
if [ -n "$MODEL_PATH" ]; then
    cp "$MODEL_PATH" "$APP/Contents/Resources/ggml-base.en.bin"
    echo "==> Bundled whisper model from $MODEL_PATH"
else
    echo "Error: ggml-base.en.bin not found in build artifacts"
    exit 1
fi

# 6. Create Info.plist
/usr/libexec/PlistBuddy -c "Clear dict" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleName string 'Bisque Computer'" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDisplayName string 'Bisque Computer'" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleExecutable string 'bisque-computer'" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleIdentifier string 'com.fullyparsed.bisque-computer'" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleVersion string '${VERSION}'" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string '${VERSION}'" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundlePackageType string 'APPL'" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleInfoDictionaryVersion string '6.0'" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string 'AppIcon'" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :NSHighResolutionCapable bool true" "$APP/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Add :NSSupportsAutomaticGraphicsSwitching bool true" "$APP/Contents/Info.plist"

# 7. Create PkgInfo
echo -n "APPL????" > "$APP/Contents/PkgInfo"

# 8. Ad-hoc codesign
echo "==> Codesigning..."
codesign --force --deep --sign - "$APP"

echo ""
echo "Done! Built: $APP"
echo ""
echo "To install, drag to /Applications:"
echo "  open target/release/"
echo ""
echo "Or launch directly:"
echo "  open $APP"
