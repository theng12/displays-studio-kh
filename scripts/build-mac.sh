#!/usr/bin/env bash
# Build, sign and package Displays Studio KH for this Mac.
# This Mac builds; the artifact goes to the app's own GitHub repo (App Publisher), then the appcast.
#
#   bash scripts/build-mac.sh            # signed .app + DMG into dist/
#   bash scripts/build-mac.sh --check    # versions, tools and identity only
#
# Launching helpers may start this with zsh; re-exec under bash so bash syntax keeps working.
[ -n "${BASH_VERSION:-}" ] || exec bash "$0" "$@"
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$1"; }
fail() { printf '\nStopped: %s\n' "$1" >&2; exit 1; }

for p in /opt/homebrew/bin /opt/homebrew/opt/rustup/bin "$HOME/.cargo/bin" /usr/local/bin; do
  [ -d "$p" ] && [[ ":$PATH:" != *":$p:"* ]] && export PATH="$p:$PATH"
done

command -v cargo >/dev/null || fail "Rust (cargo) is missing."
command -v python3 >/dev/null || fail "Python 3 is missing."
command -v hdiutil >/dev/null || fail "hdiutil is missing."
[ -d src-tauri/vendor/khsparkle/Sparkle.framework ] || fail "the vendored Sparkle.framework is missing"
[ -f src-tauri/vendor/khsparkle/libkhsparkle.a ] || fail "the vendored libkhsparkle.a is missing"

# The app bundle is staged here rather than by the Tauri CLI: this repo has no npm project, and the
# CLI that used to come from another checkout's node_modules is gone. The layout below is the one
# Tauri produced for 0.1.0, so the product is the same shape without the extra tool.

step "Checking versions agree"
version="$(python3 -c "import json;print(json.load(open('src-tauri/tauri.conf.json'))['version'])")"
[ "$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -n1)" = "$version" ] || fail "Cargo.toml version differs from tauri.conf.json"
[ "$(awk '/^## / { print $2; exit }' CHANGELOG.md)" = "$version" ] || fail "CHANGELOG.md's newest entry is not $version"
[ "$(python3 -c "import json;print(json.load(open('ui-shell/whats-new.json'))[0]['version'])")" = "$version" ] || fail "whats-new.json's newest entry is not $version"
printf 'Version %s\n' "$version"

step "Unlocking the shared signing keychain"
security find-identity -p codesigning 2>/dev/null | grep -F '"KH Studio Local Signing"' >/dev/null \
  || fail "the shared 'KH Studio Local Signing' identity is missing; do not create a replacement"
security unlock-keychain -p "$(cat "$HOME/.kh-studio-signing.pass")" \
  "$HOME/Library/Keychains/kh-studio-signing.keychain-db" || fail "could not unlock the signing keychain"

if [ "${1:-}" = "--check" ]; then
  step "Check passed"
  exit 0
fi

step "Building the release binary"
# The public release scan refuses an artifact that names the owner's home directory, and Rust
# bakes absolute source paths (the cargo registry, this workspace) into panic metadata. Remap
# them to a neutral prefix so the shipped binary carries no local path.
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME=/build"
(cd src-tauri && cargo build --release --locked --features custom-protocol) \
  || fail "the release build failed"
binary="src-tauri/target/release/displays-studio-kh"
[ -x "$binary" ] || fail "the release binary was not produced at $binary"

step "Staging and signing the app bundle"
app="src-tauri/target/release/bundle/macos/Displays Studio KH.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/displays-studio-kh"
chmod 0755 "$app/Contents/MacOS/displays-studio-kh"
cp src-tauri/icons/icon.icns "$app/Contents/Resources/icon.icns"
python3 - "$app/Contents/Info.plist" "$version" src-tauri/Info.plist <<'PY'
import plistlib, sys
plist = {
    "CFBundleDevelopmentRegion": "English",
    "CFBundleDisplayName": "Displays Studio KH",
    "CFBundleExecutable": "displays-studio-kh",
    "CFBundleIconFile": "icon.icns",
    "CFBundleIdentifier": "app.displaysstudiokh.desktop",
    "CFBundleInfoDictionaryVersion": "6.0",
    "CFBundleName": "Displays Studio KH",
    "CFBundlePackageType": "APPL",
    "CFBundleShortVersionString": sys.argv[2],
    "CFBundleVersion": sys.argv[2],
    "LSMinimumSystemVersion": "13.0",
    "LSRequiresCarbon": True,
    "LSUIElement": True,
    # One menu-bar icon only: a second launch activates the running copy (RunEvent::Reopen shows
    # the panel) instead of starting another instance with its own tap and its own DDC writes.
    "LSMultipleInstancesProhibited": True,
    "NSHighResolutionCapable": True,
}
# The Tauri CLI is not used here, so nothing else merges src-tauri/Info.plist: a key written there
# would otherwise exist in the repo and never in the bundle. It is the one source for app-specific keys.
plist.update(plistlib.load(open(sys.argv[3], "rb")))
with open(sys.argv[1], "wb") as stream:
    plistlib.dump(plist, stream)
PY
# The embedded frontend lives in the binary (custom-protocol); the one thing beside it is Sparkle.
mkdir -p "$app/Contents/Frameworks"
cp -R src-tauri/vendor/khsparkle/Sparkle.framework "$app/Contents/Frameworks/" \
  || fail "could not copy the vendored Sparkle.framework into Contents/Frameworks"
# Nested code first, the app last (INTEGRATION.md §3A). A nested binary signed after the app is what
# breaks `codesign --verify --deep --strict` — which is a pilot assertion.
ID="KH Studio Local Signing"
FW="$app/Contents/Frameworks/Sparkle.framework/Versions/B"
for nested in "$FW/XPCServices/Downloader.xpc" "$FW/XPCServices/Installer.xpc" "$FW/Updater.app" "$FW/Autoupdate"; do
  [ -e "$nested" ] || fail "the vendored Sparkle framework has no $(basename "$nested")"
  codesign --force --sign "$ID" --timestamp=none --options runtime "$nested" \
    || fail "could not sign $(basename "$nested")"
done
codesign --force --sign "$ID" --timestamp=none --options runtime "$FW" \
  || fail "could not sign the Sparkle framework"
codesign --force --sign "$ID" --timestamp=none --options runtime \
  --entitlements src-tauri/entitlements.plist "$app" || fail "could not sign the app"
codesign --verify --strict "$app" || fail "the staged app failed its own signature check"
FWCHECK="$app/Contents/Frameworks/Sparkle.framework"
[ -d "$FWCHECK" ] || fail "the staged app carries no Sparkle.framework"
otool -l "$app/Contents/MacOS/displays-studio-kh" | grep -q '@executable_path/../Frameworks' \
  || fail "the binary carries no @executable_path/../Frameworks rpath"

step "Packaging the DMG"
vol="Displays Studio KH $version"
dmg_source="src-tauri/target/release/bundle/dmg/Displays Studio KH_${version}_aarch64.dmg"
mkdir -p src-tauri/target/release/bundle/dmg
rm -f "$dmg_source"
staging="$(mktemp -d /tmp/displays-dmg-src.XXXXXX)"
trap 'rm -rf "$staging"' EXIT
cp -R "$app" "$staging/Displays Studio KH.app"
hdiutil create -volname "$vol" -srcfolder "$staging" -format UDZO -ov \
  "$dmg_source" >/dev/null || fail "hdiutil could not build the DMG"

step "Verifying the bundle"
python3 - "$app" "$version" <<'PY'
import plistlib, sys
path, version = sys.argv[1], sys.argv[2]
plist = plistlib.load(open(path + "/Contents/Info.plist", "rb"))
assert plist.get("CFBundleIdentifier") == "app.displaysstudiokh.desktop", plist.get("CFBundleIdentifier")
assert plist.get("CFBundleShortVersionString") == version, plist.get("CFBundleShortVersionString")
assert plist.get("CFBundleName") == "Displays Studio KH", plist.get("CFBundleName")
assert plist.get("LSUIElement") is True, "LSUIElement must be true (menu-bar app)"
assert plist.get("LSMultipleInstancesProhibited") is True, "a second launch must not start a second instance"
assert plist.get("SUFeedURL") == "https://theng12.github.io/displays-studio-kh/appcast.xml", "SUFeedURL missing from the bundle"
assert plist.get("KHHoldURL", "").endswith(":8650/api/maintenance/hold"), "KHHoldURL missing from the bundle"
print("Info.plist: id, version, name, LSUIElement, single-instance, SUFeedURL, KHHoldURL ok")
PY
codesign --verify --deep --strict "$app" || fail "the app's signature is broken"

step "Keeping the DMG in dist/"
mkdir -p dist
case "$(uname -m)" in
  arm64) dmg_arch=aarch64 ;;
  x86_64) dmg_arch=x64 ;;
  *) fail "unsupported DMG architecture: $(uname -m)" ;;
esac
dmg="dist/Displays Studio KH-${version}.dmg"
[ ! -e "$dmg" ] || fail "$dmg already exists; one version = one artifact, so choose a new version"
cp "$dmg_source" "$dmg"

mount="$(mktemp -d /tmp/displays-studio-dmg.XXXXXX)"
trap 'hdiutil detach "$mount" >/dev/null 2>&1 || true; rmdir "$mount" 2>/dev/null || true' EXIT
hdiutil attach -nobrowse -mountpoint "$mount" "$dmg" >/dev/null
[ -d "$mount/Displays Studio KH.app" ] || fail "the DMG does not contain Displays Studio KH.app"
codesign --verify --strict "$mount/Displays Studio KH.app" || fail "the app inside the DMG is unsigned"
hdiutil detach "$mount" >/dev/null
rmdir "$mount"
trap - EXIT

sha="$(shasum -a 256 "$dmg" | awk '{print $1}')"
bytes="$(stat -f %z "$dmg")"
python3 - "$dmg" "$version" "$sha" "$bytes" <<'PY'
import datetime, json, sys
path, version, sha, size = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
receipt = {
    "app": "Displays Studio KH",
    "bundleID": "app.displaysstudiokh.desktop",
    "version": version,
    "artifact": path,
    "sha256": sha,
    "bytes": size,
    "builtAt": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
}
with open(f"dist/receipt-{version}.json", "w") as stream:
    json.dump(receipt, stream, indent=2)
print(json.dumps(receipt, indent=2))
PY
printf '\nRelease ready: %s\n' "$dmg"
