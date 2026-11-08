#!/usr/bin/env python3
# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

# The Rust crates of Cargo.lock as Flatpak sources (cargo-sources.json), so
# the Flatpak builds without network access, as Flathub requires: each
# crate from crates.io with its checksum, unpacked into cargo/vendor, and a
# Cargo configuration that takes them from there. Run after Cargo.lock
# changes, from the repository's root:
#   packaging/flatpak/cargo-sources.py > packaging/flatpak/cargo-sources.json
import json
import sys
import tomllib

CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"

with open(sys.argv[1] if len(sys.argv) > 1 else "Cargo.lock", "rb") as f:
    lock = tomllib.load(f)

sources = []
for package in lock["package"]:
    source = package.get("source")
    if source is None:
        continue  # Stratagram's own crates
    if source != CRATES_IO:
        sys.exit(f"{package['name']}: only crates from crates.io are supported, not {source}")
    name, version, checksum = package["name"], package["version"], package["checksum"]
    dest = f"cargo/vendor/{name}-{version}"
    sources.append({
        "type": "archive",
        "archive-type": "tar-gzip",
        "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
        "sha256": checksum,
        "dest": dest,
    })
    sources.append({
        "type": "inline",
        "contents": json.dumps({"package": checksum, "files": {}}),
        "dest": dest,
        "dest-filename": ".cargo-checksum.json",
    })

sources.append({
    "type": "inline",
    "contents": '[source.vendored-sources]\ndirectory = "cargo/vendor"\n\n'
                '[source.crates-io]\nreplace-with = "vendored-sources"\n',
    "dest": "cargo",
    "dest-filename": "config.toml",
})
json.dump(sources, sys.stdout, indent=4)
print()
