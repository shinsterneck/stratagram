#!/bin/sh
# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

# The prebuilt Linux download: dist/stratagram-X.Y.Z-linux-x86_64.tar.gz,
# laid out like /usr (bin/, share/...) with the app, its menu entry, icon,
# file type, software center details and licenses. The Build workflow makes
# it on Arch Linux (against the Qt Arch ships), and the AUR package
# stratagram-bin installs it. Needs Rust, Qt 6 and cargo-about. From the
# repository's root:
#   packaging/linux/bundle.sh
set -eu
cd "$(dirname "$0")/../.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
name="stratagram-$version-linux-x86_64"
out="dist/$name"
id=app.stratagram.Stratagram

QMAKE="${QMAKE:-$(command -v qmake6 || command -v qmake)}" cargo build --release --locked -p stratagram

rm -rf "$out" "$out.tar.gz"
install -Dm755 target/release/stratagram "$out/bin/stratagram"
strip --strip-unneeded "$out/bin/stratagram"
install -Dm644 "packaging/$id.desktop" "$out/share/applications/$id.desktop"
install -Dm644 packaging/stratagram.svg "$out/share/icons/hicolor/scalable/apps/$id.svg"
install -Dm644 "packaging/$id.mime.xml" "$out/share/mime/packages/$id.xml"
install -Dm644 "packaging/$id.metainfo.xml" "$out/share/metainfo/$id.metainfo.xml"
install -Dm644 -t "$out/share/licenses/stratagram" LICENSE NOTICE
cargo about generate --locked -c about.toml -m Cargo.toml --target x86_64-unknown-linux-gnu \
    -o "$out/share/licenses/stratagram/THIRD-PARTY-LICENSES.txt" about.hbs
install -Dm644 README.md "$out/share/doc/stratagram/README.md"

tar -C dist --owner=0 --group=0 -czf "$out.tar.gz" "$name"
echo "$out.tar.gz"
