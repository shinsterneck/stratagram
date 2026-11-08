// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Stratagram without a user interface: the diagram model, the editor that
//! turns pointer and keyboard input into changes, auto layout, device
//! shapes, frames, and SVG and draw.io export and import. The desktop app
//! (Qt) and the browser editor are thin layers over [`editor::Editor`].

pub mod cml;
pub mod config;
pub mod drawio;
pub mod drawio_import;
pub mod editor;
pub mod frame;
pub mod geom;
pub mod layout;
pub mod model;
pub mod netbox;
pub mod shape;
pub mod svg;
pub mod theme;
