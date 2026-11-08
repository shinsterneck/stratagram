#!/bin/sh
# This file is part of Stratagram: https://github.com/shinsterneck/stratagram
# Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
# SPDX-License-Identifier: Apache-2.0

# Render the homepage's diagram pictures from site/examples with the
# release build (cargo build --release first). The app and editor
# screenshots are taken by hand from the running app:
# - desktop-app, rear-drag, deep-zoom-out/-in: of site/examples/berlin-hq,
#   rear-drag and deep-zoom.stratagram (rear-drag: at 259 %, a line from
#   srv-03 dragged to the panel's edge under socket 7, not yet dropped;
#   deep-zoom-in: with Zoom into on, zoomed into esx-01 at 577 %).
# - theme-light/-dark/-neon: berlin-hq in a 1400x900 window in each theme,
#   cropped to the frame (1000x672+48+102).
# - netbox-*: from NetBox 4.6 (netbox-docker with the plugin's wheel) with
#   netbox-seed.py loaded, its Diagram tab in a 1920x1200 window: editor
#   (light, fw-01 selected), dark and neon (NetBox in dark mode), layers
#   (Switches opened), e2e-shown and follow (the layer Patch panels shown, and
#   hidden with the line to srv-03 clicked and pointed at; cropped
#   700x380+595+735).
# - follow-cable: vienna-dc in a 1400x900 window, the core-01 uplink
#   selected and the pointer on the line to srv-04 (cropped 845x480+230+320).
set -e
cd "$(dirname "$0")"
bin=../target/release/stratagram
$bin --export-png examples/berlin-hq.stratagram shots/hero-diagram.png 2
$bin --export-png examples/stencils.stratagram shots/stencils.png 2
$bin --export-png examples/front-panels.stratagram shots/front-panels.png 2
$bin --export-png examples/end-to-end.stratagram shots/e2e-shown.png 2
# The same with the layer "Patch panels" hidden: the cabling end to end.
tmp=$(mktemp --suffix=.stratagram)
sed 's/"name": "Patch panels"/"name": "Patch panels", "visible": false/' examples/end-to-end.stratagram > "$tmp"
$bin --export-png "$tmp" shots/e2e-hidden.png 2
rm -f "$tmp"
# Side by side on the page: as wide as the other.
w=$(magick identify -format %w shots/e2e-shown.png)
h=$(magick identify -format %h shots/e2e-shown.png)
magick shots/e2e-hidden.png -background white -gravity center -extent "${w}x${h}" shots/e2e-hidden.png
