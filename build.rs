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

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_resources();
    }
}

/// Built on Linux or macOS for Windows: without the icon and details.
#[cfg(not(windows))]
fn windows_resources() {}

/// The icon (Explorer, the taskbar and files opened with Stratagram) and
/// the details on the Details tab of stratagram.exe's properties.
#[cfg(windows)]
fn windows_resources() {
    use std::{env, fs, path::PathBuf};

    let dir = env::var("CARGO_MANIFEST_DIR").unwrap().replace('\\', "/");
    let icon = format!("{dir}/packaging/windows/stratagram.ico");
    println!("cargo:rerun-if-changed={icon}");
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numbers = ["MAJOR", "MINOR", "PATCH"]
        .map(|part| env::var(format!("CARGO_PKG_VERSION_{part}")).unwrap())
        .join(",");
    let rc = format!(
        r#"1 ICON "{icon}"
1 VERSIONINFO
FILEVERSION {numbers},0
PRODUCTVERSION {numbers},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Shin Sterneck"
      VALUE "FileDescription", "Stratagram"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "stratagram"
      VALUE "LegalCopyright", "Copyright 2026 Shin Sterneck. Apache License 2.0."
      VALUE "OriginalFilename", "stratagram.exe"
      VALUE "ProductName", "Stratagram"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("stratagram.rc");
    fs::write(&path, rc).unwrap();
    embed_resource::compile(&path, embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
