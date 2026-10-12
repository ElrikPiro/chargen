#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

machine=$(uname -m)
case "$machine" in
  x86_64|AMD64|amd64) architecture=x86_64 ;;
  aarch64|arm64) architecture=aarch64 ;;
  *) architecture=$machine ;;
esac

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*|Windows_NT)
    target_os=windows
    executable_name=chargen-app.exe
    ;;
  Darwin*)
    target_os=macos
    executable_name=chargen-app
    ;;
  *)
    target_os=linux
    executable_name=chargen-app
    ;;
esac

release_directory=target/release
if [ -n "${CHARGEN_RUST_TARGET:-}" ]; then
  release_directory="target/$CHARGEN_RUST_TARGET/release"
fi
executable="$release_directory/$executable_name"

version=$(awk -F '"' '$1 ~ /^version[[:space:]]*=/ {print $2; exit}' Cargo.toml)
release_version=${CHARGEN_VERSION:-$version}
target=${CHARGEN_TARGET:-$target_os-$architecture}
commit=${CHARGEN_COMMIT:-$(git rev-parse HEAD)}
case "$release_version" in
  ''|*[!A-Za-z0-9._-]*) echo "Invalid release version." >&2; exit 2 ;;
esac
case "$target" in
  ''|*[!A-Za-z0-9._-]*) echo "Invalid release target." >&2; exit 2 ;;
esac
case "$commit" in
  ''|*[!A-Fa-f0-9]*) echo "Invalid commit identifier." >&2; exit 2 ;;
esac

test -f "$executable"
rm -rf dist
source_metadata_dir=$(mktemp -d)
trap 'rm -rf "$source_metadata_dir"' EXIT HUP INT TERM
git status --porcelain=v1 --untracked-files=all > "$source_metadata_dir/status"
git ls-files --cached --others --exclude-standard -z > "$source_metadata_dir/files"
package_name="chargen-${release_version}-${target}"
package_dir="dist/$package_name"
archive="dist/$package_name.tar.gz"
mkdir -p "$package_dir"
cp "$executable" "$package_dir/"
cp LICENSE README.md "$package_dir/"
cp -R docs "$package_dir/"
node scripts/create-package-manifest.mjs "$package_dir" "$release_version" "$version" "$target" "$commit" "$(basename "$executable")" "$source_metadata_dir/status" "$source_metadata_dir/files"
tar -czf "$archive" -C dist "$package_name"
cp "$package_dir/MANIFEST.json" "dist/$package_name.manifest.json"
node scripts/write-checksum.mjs "$archive"
printf 'Created %s\n' "$repo_root/$archive"
