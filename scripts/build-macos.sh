#!/usr/bin/env bash
# Build the macOS .app and a .dmg installer.
#
# Codesigning + notarization are optional. If DEVELOPER_ID and
# NOTARIZATION_PROFILE are set, we sign and notarize. Otherwise we produce an
# unsigned .app — useful for local testing, but Gatekeeper will block it on
# other machines unless they right-click → Open.
#
# Required env (optional):
#   DEVELOPER_ID            "Developer ID Application: Your Name (TEAMID)"
#   NOTARIZATION_PROFILE    a `xcrun notarytool store-credentials` profile name
#
# Usage:
#   ./scripts/build-macos.sh [--release|--debug]
#
set -euo pipefail

CONFIG="${1:-Release}"
case "$CONFIG" in
  --debug|debug|Debug) CONFIG="Debug" ;;
  --release|release|Release|"") CONFIG="Release" ;;
  *) echo "unknown config: $CONFIG" >&2; exit 2 ;;
esac

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP_NAME="TaskListener"
DIST="$ROOT/dist"
mkdir -p "$DIST"

echo "==> Building Rust core ($CONFIG)"
cd "$ROOT"
if [ "$CONFIG" = "Release" ]; then
  cargo build -p tasklistener-ffi --release
else
  cargo build -p tasklistener-ffi
fi

if ! xcode-select -p 2>/dev/null | grep -q Xcode.app; then
  cat >&2 <<EOF
==> Full Xcode is required to build the .app bundle.
   Currently 'xcode-select -p' returns: $(xcode-select -p 2>/dev/null || echo "(not set)")
   Install Xcode from the App Store, then:
     sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
   Or build via CI (.github/workflows/release.yml) which has Xcode pre-installed.
EOF
  exit 1
fi

echo "==> Generating Xcode project with xcodegen"
cd "$ROOT/apps/macos"
if ! command -v xcodegen >/dev/null; then
  echo "xcodegen not installed. Run: brew install xcodegen" >&2
  exit 1
fi
xcodegen generate

echo "==> Building $APP_NAME with xcodebuild ($CONFIG)"
DERIVED="$ROOT/build/derived"
xcodebuild \
  -project "$APP_NAME.xcodeproj" \
  -scheme "$APP_NAME" \
  -configuration "$CONFIG" \
  -derivedDataPath "$DERIVED" \
  -destination "generic/platform=macOS" \
  CODE_SIGN_IDENTITY="${DEVELOPER_ID:-}" \
  CODE_SIGN_STYLE="${DEVELOPER_ID:+Manual}" \
  build

APP_PATH="$DERIVED/Build/Products/$CONFIG/$APP_NAME.app"
if [ ! -d "$APP_PATH" ]; then
  echo "expected $APP_PATH after build" >&2
  exit 1
fi

if [ -n "${DEVELOPER_ID:-}" ]; then
  echo "==> Codesigning"
  codesign --force --deep --options runtime \
    --sign "$DEVELOPER_ID" \
    "$APP_PATH/Contents/Frameworks/libtasklistener.dylib"
  codesign --force --deep --options runtime \
    --sign "$DEVELOPER_ID" \
    "$APP_PATH"
  codesign --verify --deep --strict --verbose=2 "$APP_PATH"
else
  echo "==> Skipping codesigning (DEVELOPER_ID not set)"
fi

DMG_PATH="$DIST/$APP_NAME-$CONFIG.dmg"
rm -f "$DMG_PATH"
echo "==> Building DMG: $DMG_PATH"
if command -v create-dmg >/dev/null; then
  create-dmg \
    --volname "$APP_NAME" \
    --window-size 540 360 \
    --icon "$APP_NAME.app" 140 180 \
    --app-drop-link 400 180 \
    --hdiutil-quiet \
    "$DMG_PATH" \
    "$APP_PATH" \
    || true   # create-dmg returns non-zero on cosmetic failures we can ignore
else
  echo "create-dmg not installed, falling back to hdiutil"
  STAGING="$(mktemp -d)"
  cp -R "$APP_PATH" "$STAGING/"
  ln -s /Applications "$STAGING/Applications"
  hdiutil create -volname "$APP_NAME" -srcfolder "$STAGING" \
    -ov -format UDZO "$DMG_PATH"
  rm -rf "$STAGING"
fi

if [ -n "${DEVELOPER_ID:-}" ]; then
  codesign --force --sign "$DEVELOPER_ID" "$DMG_PATH"
fi

if [ -n "${NOTARIZATION_PROFILE:-}" ] && [ -n "${DEVELOPER_ID:-}" ]; then
  echo "==> Notarising"
  xcrun notarytool submit "$DMG_PATH" \
    --keychain-profile "$NOTARIZATION_PROFILE" --wait
  xcrun stapler staple "$DMG_PATH"
else
  echo "==> Skipping notarisation (NOTARIZATION_PROFILE not set)"
fi

echo
echo "Built: $DMG_PATH"
ls -lh "$DMG_PATH"
