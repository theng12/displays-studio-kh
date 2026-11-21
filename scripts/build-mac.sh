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

# shortcut: this repo has no npm project, so the Tauri CLI comes from an installed app's
# node_modules. Upgrade by setting TAURI_CLI when that worktree goes away.
TAURI="${TAURI_CLI:-}"
if [ -z "$TAURI" ]; then
  for candidate in "$(command -v tauri 2>/dev/null || true)" \
    "$HOME/Developer/_worktrees/wire-studio-kh/021-fix/node_modules/.bin/tauri"; do
    [ -x "$candidate" ] && { TAURI="$candidate"; break; }
  done
fi
[ -n "$TAURI" ] && [ -x "$TAURI" ] || fail "no Tauri CLI found; set TAURI_CLI to one"

step "Checking versions agree"
version="$(python3 -c "import json;print(json.load(open('src-tauri/tauri.conf.json'))['version'])")"
[ "$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -n1)" = "$version" ] || fail "Cargo.toml version differs from tauri.conf.json"
[ "$(awk '/^## / { print $2; exit }' CHANGELOG.md)" = "$version" ] || fail "CHANGELOG.md's newest entry is not $version"
[ "$(python3 -c "import json;print(json.load(open('ui-shell/whats-new.json'))[0]['version'])")" = "$version" ] || fail "whats-new.json's newest entry is not $version"
printf 'Version %s (Tauri CLI %s)\n' "$version" "$("$TAURI" --version)"

step "Unlocking the shared signing keychain"
security find-identity -p codesigning 2>/dev/null | grep -F '"KH Studio Local Signing"' >/dev/null \
  || fail "the shared 'KH Studio Local Signing' identity is missing; do not create a replacement"
security unlock-keychain -p "$(cat "$HOME/.kh-studio-signing.pass")" \
  "$HOME/Library/Keychains/kh-studio-signing.keychain-db" || fail "could not unlock the signing keychain"

if [ "${1:-}" = "--check" ]; then
  step "Check passed"
  exit 0
fi

step "Building and signing the app and DMG (release)"
"$TAURI" build --bundles app,dmg \
  --config '{"bundle":{"macOS":{"signingIdentity":"KH Studio Local Signing","hardenedRuntime":false}}}' \
  -- --locked

app="src-tauri/target/release/bundle/macos/Displays Studio KH.app"
[ -d "$app" ] || fail "the app bundle was not produced at $app"

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
dmg_source="src-tauri/target/release/bundle/dmg/Displays Studio KH_${version}_${dmg_arch}.dmg"
[ -f "$dmg_source" ] || fail "the packager did not produce the exact release DMG: $dmg_source"
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
