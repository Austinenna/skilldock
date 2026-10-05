#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_NAME="SkillDock"
VERSION="$(node -e "const fs = require('fs'); process.stdout.write(JSON.parse(fs.readFileSync('package.json', 'utf8')).version)")"
TARGET="${SKILLDOCK_TARGET:-}"
SIGNING_IDENTITY="${SKILLDOCK_SIGNING_IDENTITY:-${APPLE_SIGNING_IDENTITY:-}}"
NOTARY_PROFILE="${SKILLDOCK_NOTARY_PROFILE:-${APPLE_NOTARYTOOL_PROFILE:-}}"
FORCE_ADHOC="${SKILLDOCK_FORCE_ADHOC:-0}"

case "$TARGET" in
  universal-apple-darwin)
    ARTIFACT_ARCH="universal"
    ;;
  aarch64-apple-darwin)
    ARTIFACT_ARCH="aarch64"
    ;;
  x86_64-apple-darwin)
    ARTIFACT_ARCH="x64"
    ;;
  "")
    case "$(uname -m)" in
      arm64) ARTIFACT_ARCH="aarch64" ;;
      x86_64) ARTIFACT_ARCH="x64" ;;
      *) ARTIFACT_ARCH="$(uname -m)" ;;
    esac
    ;;
  *)
    ARTIFACT_ARCH="$(echo "$TARGET" | sed 's/-apple-darwin//')"
    ;;
esac

BUNDLE_DIR="$ROOT_DIR/src-tauri/target/release/bundle"
MACOS_BUNDLE_DIR="$BUNDLE_DIR/macos"
APP_PATH="$MACOS_BUNDLE_DIR/$APP_NAME.app"
DMG_DIR="$BUNDLE_DIR/dmg"
DMG_PATH="$DMG_DIR/${APP_NAME}_${VERSION}_${ARTIFACT_ARCH}.dmg"
CHECKSUM_PATH="$DMG_PATH.sha256"

TAURI_BUILD_ARGS=(tauri build --bundles app --ci --no-sign)
if [[ -n "$TARGET" ]]; then
  TAURI_BUILD_ARGS+=(--target "$TARGET")
fi

echo "==> Building $APP_NAME $VERSION for ${TARGET:-native macOS}"
pnpm "${TAURI_BUILD_ARGS[@]}"

if [[ ! -d "$APP_PATH" ]]; then
  echo "Missing app bundle: $APP_PATH" >&2
  exit 1
fi

if [[ -z "$SIGNING_IDENTITY" || "$FORCE_ADHOC" == "1" ]]; then
  SIGNING_IDENTITY="-"
  SIGNING_LABEL="ad-hoc"
else
  SIGNING_LABEL="$SIGNING_IDENTITY"
fi

echo "==> Signing app with $SIGNING_LABEL"
if [[ "$SIGNING_IDENTITY" == "-" ]]; then
  codesign --force --deep --options runtime --sign - "$APP_PATH"
else
  codesign --force --deep --options runtime --timestamp --sign "$SIGNING_IDENTITY" "$APP_PATH"
fi
codesign --verify --deep --strict --verbose=2 "$APP_PATH"

mkdir -p "$DMG_DIR"
rm -f "$DMG_PATH" "$CHECKSUM_PATH"

STAGING_DIR="$(mktemp -d "${TMPDIR:-/tmp}/skilldock-dmg.XXXXXX")"
cleanup() {
  rm -rf "$STAGING_DIR"
}
trap cleanup EXIT

echo "==> Creating DMG from signed app"
ditto "$APP_PATH" "$STAGING_DIR/$APP_NAME.app"
ln -s /Applications "$STAGING_DIR/Applications"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGING_DIR" -ov -format UDZO "$DMG_PATH"

echo "==> Signing DMG with $SIGNING_LABEL"
if [[ "$SIGNING_IDENTITY" == "-" ]]; then
  codesign --force --sign - "$DMG_PATH"
else
  codesign --force --timestamp --sign "$SIGNING_IDENTITY" "$DMG_PATH"
fi
codesign --verify --deep --strict --verbose=2 "$DMG_PATH"
hdiutil verify "$DMG_PATH"

if [[ "$SIGNING_IDENTITY" != "-" && -n "$NOTARY_PROFILE" ]]; then
  echo "==> Notarizing DMG with keychain profile $NOTARY_PROFILE"
  xcrun notarytool submit "$DMG_PATH" --keychain-profile "$NOTARY_PROFILE" --wait
  xcrun stapler staple "$DMG_PATH"
  spctl --assess --type open --context context:primary-signature --verbose=2 "$DMG_PATH"
elif [[ "$SIGNING_IDENTITY" != "-" ]]; then
  echo "==> Skipping notarization because SKILLDOCK_NOTARY_PROFILE is not set"
else
  echo "==> Ad-hoc signed build: good for local testing, not Gatekeeper-trusted distribution"
fi

shasum -a 256 "$DMG_PATH" > "$CHECKSUM_PATH"

echo
echo "Release artifacts:"
echo "  App:      $APP_PATH"
echo "  DMG:      $DMG_PATH"
echo "  SHA-256:  $CHECKSUM_PATH"
