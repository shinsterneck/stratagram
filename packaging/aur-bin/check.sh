#!/bin/sh
# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

# Checks the stratagram-bin PKGBUILD against the Linux download made by
# packaging/linux/bundle.sh, before a release has it: makepkg builds the
# package from the local file, pacman installs it and the app answers.
# Runs as root on Arch Linux (the Build workflow's container); makepkg
# itself runs as an unprivileged user. From the repository's root:
#   packaging/aur-bin/check.sh
set -eu
cd "$(dirname "$0")/../.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
file="stratagram-$version-linux-x86_64.tar.gz"
dir=$(mktemp -d)
cp packaging/aur-bin/PKGBUILD "dist/$file" "$dir/"
sed -i -e "s|^pkgver=.*|pkgver=$version|" \
       -e "s|^source=.*|source=(\"$file\")|" \
       -e "s|^sha256sums=.*|sha256sums=('SKIP')|" "$dir/PKGBUILD"
# What the package needs at runtime, as pacman would install it for users.
pacman -S --needed --noconfirm $(bash -c ". '$dir/PKGBUILD'; echo \"\${depends[@]}\"")
id -u builder >/dev/null 2>&1 || useradd -m builder
chown -R builder "$dir"
su builder -c "cd '$dir' && makepkg -f --noconfirm"
pacman -U --noconfirm "$dir"/stratagram-bin-*.pkg.tar.zst
pacman -Ql stratagram-bin
test "$(stratagram --version)" = "stratagram $version"
echo "stratagram-bin $version: built, installed and runs"
