#!/usr/bin/env bash
# Build, sign and package Displays Studio KH for this Mac.
# This Mac builds; publishing happens on 1301 with packaging/release-push.sh.
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
python3 - "$app/Contents/Info.plist" "$version" <<'PY'
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
    "NSHighResolutionCapable": True,
}
with open(sys.argv[1], "wb") as stream:
    plistlib.dump(plist, stream)
PY
# The embedded frontend lives in the binary (custom-protocol), so the bundle needs nothing else.
codesign --force --sign "KH Studio Local Signing" "$app/Contents/MacOS/displays-studio-kh" \
  || fail "could not sign the binary"
codesign --force --sign "KH Studio Local Signing" "$app" || fail "could not sign the app"
codesign --verify --strict "$app" || fail "the staged app failed its own signature check"

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
print("Info.plist: id, version, name, LSUIElement ok")
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
