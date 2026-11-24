#!/usr/bin/env bash
# Linux development and headless acceptance dependencies. Run as root.
# Ubuntu 24.04 hosts use apt; RHEL 9 family (glibc 2.34) builders use dnf.
set -euo pipefail
if command -v dnf >/dev/null 2>&1; then
    dnf -y install epel-release dnf-plugins-core
    dnf config-manager --set-enabled crb
    dnf -y install --allowerasing \
        git gcc gcc-c++ clang make cmake ninja-build curl ca-certificates \
        pkgconf-pkg-config patchelf file which findutils tar gzip unzip \
        binutils-gold libcurl-devel libedit-devel libicu-devel libxml2-devel \
        ncurses-devel sqlite-devel libuuid-devel zlib-devel libzstd-devel \
        python3.12 \
        libsecret-devel libsecret-tools gnome-keyring alsa-lib-devel systemd-devel \
        fontconfig-devel freetype-devel gdk-pixbuf2-devel gtk4-devel webp-pixbuf-loader \
        libxkbcommon-devel libxkbcommon-x11-devel wayland-devel libX11-devel \
        libxcb-devel xcb-util-cursor-devel vulkan-headers vulkan-loader-devel libgomp \
        mesa-vulkan-drivers libwebp-devel \
        glib-networking dconf glib2-devel \
        xorg-x11-server-Xvfb xorg-x11-xauth dbus-x11 xdotool xterm ImageMagick \
        google-noto-sans-cjk-fonts dejavu-sans-fonts python3-gobject python3-pyatspi \
        desktop-file-utils xdg-utils rpm-build dpkg squashfs-tools libarchive
    dnf clean all
    rm -rf /var/cache/dnf
    # The build scripts and tooling expect python3 to mean 3.12+.
    ln -sf /usr/bin/python3.12 /usr/local/bin/python3
    exit 0
fi
apt-get update
apt-get install -y --no-install-recommends \
    build-essential clang curl ca-certificates pkg-config patchelf libssl-dev libsecret-tools gnome-keyring \
    libasound2-dev libudev-dev libfontconfig1-dev libfreetype-dev libzstd-dev \
    libgdk-pixbuf-2.0-dev libgtk-4-dev webp-pixbuf-loader \
    libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-xcb-dev \
    libxcb-shape0-dev libxcb-xfixes0-dev libxcb-render0-dev libxcb-randr0-dev \
    libxcb-xinput-dev libxcb-cursor-dev libvulkan-dev mesa-vulkan-drivers \
    cmake ninja-build libwebp-dev \
    xvfb xauth dbus-x11 xdotool xterm imagemagick fonts-noto-cjk python3 python3-pyatspi python3-gi desktop-file-utils xdg-utils
