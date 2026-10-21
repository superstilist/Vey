#!/usr/bin/env bash
set -euo pipefail
# Optional Mint/Ubuntu build headers in the user's cache; never invokes sudo.
prefix=${1:-$HOME/.cache/vey-release/deps}
mkdir -p "$prefix/packages" "$prefix/sysroot"
cd "$prefix/packages"
apt-get download libasound2-dev libasound2t64 libfontconfig-dev libfontconfig1 \
  libfreetype-dev libfreetype6 libxkbcommon-dev libxkbcommon0 libdbus-1-dev \
  libdbus-1-3 libexpat1-dev libexpat1 libbrotli-dev libbrotli1 libbz2-dev \
  libbz2-1.0 libpng-dev libpng16-16t64 zlib1g-dev zlib1g uuid-dev libuuid1
for package in ./*.deb; do dpkg-deb -x "$package" "$prefix/sysroot"; done
arch=$(dpkg-architecture -qDEB_HOST_MULTIARCH)
cat > "$prefix/env.sh" <<ENV
export PKG_CONFIG_SYSROOT_DIR='$prefix/sysroot'
export PKG_CONFIG_LIBDIR='$prefix/sysroot/usr/lib/$arch/pkgconfig:$prefix/sysroot/usr/share/pkgconfig'
export PKG_CONFIG_PATH=''
ENV
printf 'Build environment: source %s/env.sh\n' "$prefix"
