// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

// On Windows a window application, so opening a diagram from Explorer
// shows no console window next to the editor.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod bridge;

use stratagram_core::{config, editor, model, svg};

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

const USAGE: &str = "usage: stratagram [FILE.stratagram]
       stratagram --version
       stratagram --export-svg IN.stratagram OUT.svg
       stratagram --export-drawio IN.stratagram OUT.drawio     (all sheets, as pages)
       stratagram --export-cml IN.stratagram OUT.yaml          (a Cisco Modeling Labs lab)
       stratagram --export-png IN.stratagram OUT.png [SCALE]   (pixels per unit, default 2)
       stratagram --layout KIND IN.stratagram OUT.stratagram [SPACING]
                  KIND: hierarchy, hierarchy-lr, radial, circle, grid, force or tidy
                  (every sheet; OUT may be IN; spacing default 60)
SVG and PNG show the first sheet. Add --theme light|dark|neon to export them
in that theme's colors (default light).";

/// Headless conversions, so diagrams can be produced from scripts.
fn run_cli(args: &[String]) -> Option<i32> {
    // `--theme NAME`, anywhere: the colors of SVG and PNG exports.
    let mut args = args.to_vec();
    let mut theme = stratagram_core::theme::Theme::Light;
    if let Some(i) = args.iter().position(|a| a == "--theme") {
        let name = args.get(i + 1).cloned().unwrap_or_default();
        let Some(t) = stratagram_core::theme::Theme::parse(&name) else {
            eprintln!("stratagram: unknown theme {name:?} (light, dark or neon)\n{USAGE}");
            return Some(2);
        };
        theme = t;
        args.drain(i..(i + 2).min(args.len()));
    }
    let args = &args[..];
    let mode = args.get(1)?.as_str();
    if mode == "-h" || mode == "--help" {
        println!("{USAGE}");
        return Some(0);
    }
    if mode == "-V" || mode == "--version" {
        println!("stratagram {}", env!("CARGO_PKG_VERSION"));
        return Some(0);
    }
    if mode == "--layout" {
        return Some(run_layout(args));
    }
    if !mode.starts_with("--export-") {
        return None;
    }
    let (Some(input), Some(output)) = (args.get(2), args.get(3)) else {
        eprintln!("{USAGE}");
        return Some(2);
    };
    let result = std::fs::read_to_string(input)
        .map_err(|e| format!("cannot read {input}: {e}"))
        .and_then(|text| {
            let mut ed = editor::Editor::default();
            ed.load_json(&text, std::path::Path::new(input))?;
            Ok(ed)
        })
        .and_then(|ed| {
            let doc = &ed.doc;
            let title = std::path::Path::new(input)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "diagram".into());
            let file = std::path::Path::new(input)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let write = |out: String| {
                std::fs::write(output, out).map_err(|e| format!("cannot write {output}: {e}"))
            };
            match mode {
                "--export-svg" => write(svg::export_themed(doc, &file, theme)),
                "--export-drawio" => write(ed.export_drawio(&title)),
                "--export-cml" => {
                    let (yaml, skipped) = stratagram_core::cml::export(doc, &title);
                    let left_out = skipped.summary();
                    if !left_out.is_empty() {
                        eprintln!("stratagram: {left_out} (not cables in a lab)");
                    }
                    write(yaml)
                }
                "--export-png" => {
                    let scale = match args.get(4).map(|s| s.parse::<f64>()) {
                        None => bridge::PNG_SCALE,
                        Some(Ok(s)) if s > 0.0 && s.is_finite() => s,
                        Some(_) => {
                            return Err(format!("the scale must be a positive number\n{USAGE}"));
                        }
                    };
                    bridge::export_png(
                        &svg::export_themed(doc, &file, theme),
                        std::path::Path::new(output),
                        scale,
                    )
                }
                _ => Err(format!("unknown option {mode}\n{USAGE}")),
            }
        });
    match result {
        Ok(()) => Some(0),
        Err(e) => {
            eprintln!("stratagram: {e}");
            Some(1)
        }
    }
}

/// `--layout KIND IN OUT [SPACING]`: auto layout for a file written by a
/// script, saved as OUT.
fn run_layout(args: &[String]) -> i32 {
    let (Some(kind), Some(input), Some(output)) = (args.get(2), args.get(3), args.get(4)) else {
        eprintln!("{USAGE}");
        return 2;
    };
    let gap = match args.get(5).map(|s| s.parse::<f64>()) {
        None => 60.0,
        Some(Ok(g)) if g.is_finite() && g > 0.0 => g,
        Some(_) => {
            eprintln!("stratagram: the spacing must be a positive number\n{USAGE}");
            return 2;
        }
    };
    let result = std::fs::read_to_string(input)
        .map_err(|e| format!("cannot read {input}: {e}"))
        .and_then(|text| {
            let mut ed = editor::Editor::default();
            ed.load_json(&text, std::path::Path::new(input))?;
            let n = ed.layout_document(kind, gap)?;
            std::fs::write(output, ed.document_json())
                .map_err(|e| format!("cannot write {output}: {e}"))?;
            Ok(n)
        });
    match result {
        Ok(n) => {
            println!("arranged {n} devices ({kind}) into {output}");
            0
        }
        Err(e) => {
            eprintln!("stratagram: {e}");
            1
        }
    }
}

/// The command line modes print to the console they are started from
/// (on Windows a window application has none of its own).
#[cfg(windows)]
fn attach_console() {
    unsafe extern "system" {
        fn AttachConsole(process: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    // SAFETY: a plain Win32 call; failing (no console to attach to, or
    // output already redirected) leaves nothing to undo.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

#[cfg(not(windows))]
fn attach_console() {}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a.starts_with('-')) {
        attach_console();
    }
    if let Some(code) = run_cli(&args) {
        std::process::exit(code);
    }

    // Fusion gives a consistent, compact look on every desktop.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        // SAFETY: single-threaded at this point, before Qt starts.
        unsafe { std::env::set_var("QT_QUICK_CONTROLS_STYLE", "Fusion") };
    }
    // The desktop's own open and save dialogs, through the XDG desktop
    // portal (GNOME, KDE, Hyprland, Sway, ...), instead of Qt's built-in
    // ones. Qt falls back to its own if no portal answers.
    if cfg!(target_os = "linux") && std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
        // SAFETY: single-threaded at this point, before Qt starts.
        unsafe { std::env::set_var("QT_QPA_PLATFORMTHEME", "xdgdesktopportal") };
    }

    // Settings from before the rename move over, before Qt reads them.
    config::migrate_from_netdiag();

    let mut app = QGuiApplication::new();
    bridge::qobject::setup_app();
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/com/stratagram/qml/main.qml"));
    }
    if let Some(app) = app.as_mut() {
        std::process::exit(app.exec());
    }
}
