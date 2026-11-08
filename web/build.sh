#!/bin/sh
# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

# Build the browser editor into web/www/pkg (WebAssembly plus its
# JavaScript glue). Needs the wasm32 target and wasm-bindgen-cli in the
# same version as the wasm-bindgen crate (web/Cargo.toml):
#   rustup target add wasm32-unknown-unknown
#   cargo binstall wasm-bindgen-cli@0.2.129
# Serve web/www with any static web server to try it, e.g.
#   python3 -m http.server -d web/www 8000
set -e
cd "$(dirname "$0")/.."
cargo build -p stratagram-web --target wasm32-unknown-unknown --profile wasm-release
wasm-bindgen --target web --no-typescript --out-dir web/www/pkg \
    target/wasm32-unknown-unknown/wasm-release/stratagram_web.wasm
cp packaging/stratagram.svg web/www/pkg/
