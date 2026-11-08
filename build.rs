// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("com.stratagram").qml_files([
        "qml/main.qml",
        "qml/DiagramCanvas.qml",
        "qml/PropertiesPanel.qml",
        "qml/ColorPicker.qml",
        "qml/SheetBar.qml",
    ]))
    .qt_module("Quick")
    .qt_module("Svg")
    .files(["src/bridge.rs"])
    .build();
}
