#!/bin/sh
# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

# Build the browser editor and put it into the plugin's static files, then
# the plugin package (dist/). Needs what web/build.sh needs, Python's build
# module for the package (pip install build) and cargo-about for the
# licenses of the libraries built into the editor (cargo install
# cargo-about).
#
# The editor goes into a folder named after a hash of its files, so
# browsers load new versions instead of what they cached.
set -e
cd "$(dirname "$0")"
../web/build.sh
# The license goes into the package too.
cp ../LICENSE ../NOTICE .
files="../web/www/app.js ../web/www/canvas.js ../web/www/panel.js ../web/www/stratagram.css ../web/www/pkg/stratagram_web.js ../web/www/pkg/stratagram_web_bg.wasm netbox_stratagram/static/netbox_stratagram/netbox.js"
hash=$(cat $files | sha256sum | cut -c1-12)
root=netbox_stratagram/static/netbox_stratagram/editor
rm -rf "$root"
mkdir -p "$root/$hash/pkg"
cp ../web/www/app.js ../web/www/canvas.js ../web/www/panel.js ../web/www/stratagram.css "$root/$hash/"
cp ../web/www/pkg/stratagram_web.js ../web/www/pkg/stratagram_web_bg.wasm "$root/$hash/pkg/"
printf '# Made by build.sh: where the editor build is (static/netbox_stratagram/editor/<this>/).\nEDITOR = "%s"\n' "$hash" > netbox_stratagram/editor_build.py
if [ "$1" != "--static-only" ]; then
    # The licenses of the Rust libraries in the editor (about.toml).
    cargo about generate --frozen -c ../about.toml -m ../web/Cargo.toml \
        --target wasm32-unknown-unknown -o THIRD-PARTY-LICENSES.txt ../about.hbs
    python3 -m build
fi
