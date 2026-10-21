#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
[[ $(uname -s) == Linux ]] || { echo 'Build this package on Linux.' >&2; exit 1; }
case $(uname -m) in
  x86_64) arch=x64; deb_arch=amd64 ;;
  aarch64) arch=arm64; deb_arch=arm64 ;;
  *) echo 'Unsupported Linux architecture.' >&2; exit 1 ;;
esac
version=$(sed -n 's/^version = "\([0-9.]*\)"/\1/p' Cargo.toml | head -1)
[[ -n "$version" ]] || { echo 'Missing Cargo version.' >&2; exit 1; }
if [[ ${1:-} != --skip-build ]]; then
  cargo build --locked --release --bin vey
fi
binary=${CARGO_TARGET_DIR:-$root/target}/release/vey
[[ -x "$binary" ]] || { echo 'Release binary is missing.' >&2; exit 1; }
dependencies=$(ldd "$binary")
if grep -q 'not found' <<< "$dependencies"; then
  printf '%s\n' "$dependencies" >&2; exit 1
fi
mkdir -p dist
stage=$(mktemp -d "$root/dist/.linux-package.XXXXXXXX")
trap 'case "$stage" in "$root"/dist/.linux-package.*) rm -rf -- "$stage" ;; esac' EXIT
name=Vey-$version-linux-$arch
portable=$stage/$name
mkdir -p "$portable"
install -m755 "$binary" "$portable/vey"
cp docs/LINUX-RELEASE.md "$portable/README.md"
cp web/braccato/LICENSES.txt "$portable/THIRD-PARTY-LICENSES.txt"
if git rev-parse HEAD > "$portable/SOURCE-COMMIT.txt" 2>/dev/null; then :
elif [[ -f .release-source-commit ]]; then cp .release-source-commit "$portable/SOURCE-COMMIT.txt"
else printf 'Source version %s\n' "$version" > "$portable/SOURCE-COMMIT.txt"; fi
tar -C "$stage" -czf "dist/$name.tar.gz" "$name"
deb=$stage/deb
mkdir -p "$deb/DEBIAN" "$deb/usr/bin" "$deb/usr/share/applications" "$deb/usr/share/pixmaps" "$deb/usr/share/doc/vey"
install -m755 "$binary" "$deb/usr/bin/vey"
cp assets/vey-logo-transparent.png "$deb/usr/share/pixmaps/vey.png"
cp "$portable/README.md" "$portable/THIRD-PARTY-LICENSES.txt" "$portable/SOURCE-COMMIT.txt" "$deb/usr/share/doc/vey/"
cat > "$deb/usr/share/applications/vey.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Vey
Comment=Music player with synchronized lyrics
Exec=vey
Icon=vey
Terminal=false
Categories=AudioVideo;Audio;Player;
DESKTOP
cat > "$deb/DEBIAN/control" <<CONTROL
Package: vey
Version: $version
Architecture: $deb_arch
Maintainer: Vey contributors
Section: sound
Priority: optional
Installed-Size: $(du -sk "$deb/usr" | cut -f1)
Depends: libc6 (>= 2.39), libgcc-s1, libstdc++6, libasound2t64, libfontconfig1, libfreetype6, libxkbcommon0, libxkbcommon-x11-0, libdbus-1-3, libx11-6, libxcursor1, libxi6, libxrandr2, libwayland-client0, libegl1, libgl1
Recommends: ffmpeg, xdg-desktop-portal
Homepage: https://github.com/superstilist/Vey
Description: Native music player with synchronized lyrics
 Vey provides local audio, public YouTube Music and synchronized lyrics.
CONTROL
chmod 755 "$deb/DEBIAN"
chmod 644 "$deb/DEBIAN/control"
dpkg-deb --root-owner-group -Zxz -z6 --build "$deb" "dist/$name.deb"
dpkg-deb --info "dist/$name.deb"
(cd dist && sha256sum "$name.tar.gz" > "$name.tar.gz.sha256" && sha256sum "$name.deb" > "$name.deb.sha256")
printf 'Packages: dist/%s.tar.gz and dist/%s.deb\n' "$name" "$name"
