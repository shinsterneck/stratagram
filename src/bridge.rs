// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! QObject exposed to QML. Thin wrapper: every call is forwarded to the
//! Qt-independent `Editor`, then the derived properties are refreshed.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;

        include!("stratagram/cpp/clipboard.h");
        fn set_clipboard(text: &QString, native: &QString);
        fn clipboard_native() -> QString;
        fn clipboard_text() -> QString;

        include!("stratagram/cpp/app.h");
        fn setup_app();

        include!("stratagram/cpp/png.h");
        fn svg_to_png(svg: &QString, path: &QString, scale: f64) -> QString;
    }

    #[auto_cxx_name]
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, scene)]
        #[qproperty(QString, selection)]
        #[qproperty(QString, layers)]
        #[qproperty(QString, sheets)]
        #[qproperty(QString, title)]
        #[qproperty(QString, status)]
        #[qproperty(QString, edit_box)]
        #[qproperty(bool, dirty)]
        #[qproperty(bool, can_undo)]
        #[qproperty(bool, can_redo)]
        #[qproperty(bool, has_file)]
        #[qproperty(f64, zoom)]
        #[qproperty(bool, snap)]
        #[qproperty(i32, cursor)]
        type DiagramController = super::DiagramControllerRust;

        #[qinvokable]
        fn press(self: Pin<&mut DiagramController>, x: f64, y: f64, shift: bool, ctrl: bool);
        #[qinvokable]
        fn drag(self: Pin<&mut DiagramController>, x: f64, y: f64, ctrl: bool);
        #[qinvokable]
        fn release(self: Pin<&mut DiagramController>, x: f64, y: f64);
        #[qinvokable]
        fn hover(self: Pin<&mut DiagramController>, x: f64, y: f64);
        #[qinvokable]
        fn hover_out(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn double_click(self: Pin<&mut DiagramController>, x: f64, y: f64);
        #[qinvokable]
        fn begin_edit_selection(self: Pin<&mut DiagramController>) -> bool;
        #[qinvokable]
        fn commit_edit(self: Pin<&mut DiagramController>, text: &QString, w: f64, h: f64);
        #[qinvokable]
        fn cancel_edit(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn cancel(self: Pin<&mut DiagramController>);

        #[qinvokable]
        fn add_device(self: Pin<&mut DiagramController>, x: f64, y: f64);
        #[qinvokable]
        fn add_device_of(self: Pin<&mut DiagramController>, kind: &QString, x: f64, y: f64);
        #[qinvokable]
        fn add_note(self: Pin<&mut DiagramController>, x: f64, y: f64);
        #[qinvokable]
        fn add_point(self: Pin<&mut DiagramController>, x: f64, y: f64);
        #[qinvokable]
        fn quick_add(self: Pin<&mut DiagramController>, spec: &QString, x: f64, y: f64) -> i32;
        #[qinvokable]
        fn add_port(self: Pin<&mut DiagramController>, side: &QString);
        #[qinvokable]
        fn set_deep_zoom(self: Pin<&mut DiagramController>, on: bool);
        #[qinvokable]
        fn view_changed(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn copy_drawio(self: Pin<&mut DiagramController>) -> bool;
        #[qinvokable]
        fn copy_sheet(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn cut(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn paste(self: Pin<&mut DiagramController>, x: f64, y: f64, w: f64, h: f64) -> bool;
        #[qinvokable]
        fn switch_sheet(self: Pin<&mut DiagramController>, index: i32);
        #[qinvokable]
        fn add_sheet(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn remove_sheet(self: Pin<&mut DiagramController>, index: i32) -> bool;
        #[qinvokable]
        fn rename_sheet(self: Pin<&mut DiagramController>, index: i32, name: &QString);
        #[qinvokable]
        fn move_sheet(self: Pin<&mut DiagramController>, from: i32, to: i32);
        #[qinvokable]
        fn add_layer(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn add_sublayer(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn move_layer(
            self: Pin<&mut DiagramController>,
            id: i32,
            target: i32,
            place: &QString,
        ) -> bool;
        #[qinvokable]
        fn set_layer_expanded(self: Pin<&mut DiagramController>, id: i32, expanded: bool);
        #[qinvokable]
        fn remove_layer(self: Pin<&mut DiagramController>, id: i32) -> bool;
        #[qinvokable]
        fn set_layer_visible(self: Pin<&mut DiagramController>, id: i32, visible: bool);
        #[qinvokable]
        fn set_layer_locked(self: Pin<&mut DiagramController>, id: i32, locked: bool);
        #[qinvokable]
        fn set_layer_show_name(self: Pin<&mut DiagramController>, id: i32, show: bool);
        #[qinvokable]
        fn rename_layer(self: Pin<&mut DiagramController>, id: i32, name: &QString);
        #[qinvokable]
        fn set_active_layer(self: Pin<&mut DiagramController>, id: i32);
        #[qinvokable]
        fn auto_layout(
            self: Pin<&mut DiagramController>,
            kind: &QString,
            gap: f64,
            tidy_ports: bool,
            avoid: bool,
        ) -> bool;
        #[qinvokable]
        fn arrange(self: Pin<&mut DiagramController>, op: &QString);
        #[qinvokable]
        fn delete_selected(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn duplicate(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn select_all(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn nudge(self: Pin<&mut DiagramController>, dx: f64, dy: f64);
        #[qinvokable]
        fn reverse_link(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn set_prop(self: Pin<&mut DiagramController>, key: &QString, value: &QString) -> bool;
        #[qinvokable]
        fn set_frame(self: Pin<&mut DiagramController>, key: &QString, value: &QString) -> bool;
        #[qinvokable]
        fn set_layer_background(
            self: Pin<&mut DiagramController>,
            layer: i64,
            key: &QString,
            value: &QString,
        ) -> bool;
        #[qinvokable]
        fn save_frame_defaults(self: Pin<&mut DiagramController>, all_templates: bool) -> bool;
        #[qinvokable]
        fn undo(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn redo(self: Pin<&mut DiagramController>);

        #[qinvokable]
        fn new_diagram(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn open(self: Pin<&mut DiagramController>, url: &QUrl) -> bool;
        #[qinvokable]
        fn save(self: Pin<&mut DiagramController>) -> bool;
        #[qinvokable]
        fn save_as(self: Pin<&mut DiagramController>, url: &QUrl) -> bool;
        #[qinvokable]
        fn export_svg(self: Pin<&mut DiagramController>, url: &QUrl) -> bool;
        #[qinvokable]
        fn set_theme(self: Pin<&mut DiagramController>, name: &QString);
        #[qinvokable]
        fn export_drawio(self: Pin<&mut DiagramController>, url: &QUrl) -> bool;
        #[qinvokable]
        fn export_png(self: Pin<&mut DiagramController>, url: &QUrl) -> bool;
        #[qinvokable]
        fn export_cml(self: Pin<&mut DiagramController>, url: &QUrl) -> bool;
        #[qinvokable]
        fn startup(self: Pin<&mut DiagramController>);
        #[qinvokable]
        fn file_folder(self: &DiagramController) -> QUrl;
        #[qinvokable]
        fn file_base_name(self: &DiagramController) -> QString;
        #[qinvokable]
        fn version(self: &DiagramController) -> QString;
        #[qinvokable]
        fn config_path(self: &DiagramController) -> QString;
    }
}

use crate::editor::Editor;

pub const PNG_SCALE: f64 = 2.0;

/// Write the diagram's SVG export as a PNG picture, `scale` pixels per unit.
pub fn export_png(svg: &str, path: &std::path::Path, scale: f64) -> Result<(), String> {
    let err = qobject::svg_to_png(
        &QString::from(svg),
        &QString::from(path.display().to_string().as_str()),
        scale,
    );
    if err.is_empty() {
        Ok(())
    } else {
        Err(err.to_string())
    }
}
use crate::model::{Id, Pt, Side};
use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QString, QUrl};
use std::path::PathBuf;

pub struct DiagramControllerRust {
    scene: QString,
    selection: QString,
    layers: QString,
    sheets: QString,
    title: QString,
    status: QString,
    edit_box: QString,
    dirty: bool,
    can_undo: bool,
    can_redo: bool,
    has_file: bool,
    zoom: f64,
    snap: bool,
    cursor: i32,
    editor: Editor,
}

impl Default for DiagramControllerRust {
    fn default() -> Self {
        let mut editor = Editor::default();
        editor.config_path = Some(crate::config::path());
        DiagramControllerRust {
            scene: QString::from(editor.scene_json().as_str()),
            selection: QString::from(editor.selection_json().as_str()),
            layers: QString::from(editor.layers_json().as_str()),
            sheets: QString::from(editor.sheets_json().as_str()),
            title: QString::from(editor.title().as_str()),
            status: QString::default(),
            edit_box: QString::default(),
            dirty: false,
            can_undo: false,
            can_redo: false,
            has_file: false,
            zoom: 1.0,
            snap: true,
            cursor: 0,
            editor,
        }
    }
}

fn url_to_path(url: &QUrl) -> Option<PathBuf> {
    if let Some(local) = url
        .to_local_file()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
    {
        return Some(PathBuf::from(local));
    }
    let text = url.to_qstring().to_string();
    let has_scheme = text.split_once(':').is_some_and(|(scheme, _)| {
        scheme.len() > 1
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    if text.is_empty() || has_scheme {
        None
    } else {
        Some(PathBuf::from(text))
    }
}

impl qobject::DiagramController {
    fn edit<R>(mut self: Pin<&mut Self>, f: impl FnOnce(&mut Editor) -> R) -> R {
        let zoom = *self.zoom();
        let snap = *self.snap();
        let r = {
            let mut rust = self.as_mut().rust_mut();
            let ed = &mut rust.editor;
            ed.zoom = zoom;
            ed.snap = snap;
            f(ed)
        };
        self.refresh();
        r
    }

    fn refresh(mut self: Pin<&mut Self>) {
        let ed = &self.rust().editor;
        let scene = QString::from(ed.scene_json().as_str());
        let edit_box = QString::from(ed.edit_box_json().as_str());
        let selection = QString::from(ed.selection_json().as_str());
        let layers = QString::from(ed.layers_json().as_str());
        let sheets = QString::from(ed.sheets_json().as_str());
        let title = QString::from(ed.title().as_str());
        let status = QString::from(ed.status.as_str());
        let (dirty, can_undo, can_redo, has_file) =
            (ed.dirty, ed.can_undo(), ed.can_redo(), ed.path.is_some());
        self.as_mut().set_scene(scene);
        if *self.edit_box() != edit_box {
            self.as_mut().set_edit_box(edit_box);
        }
        if *self.selection() != selection {
            self.as_mut().set_selection(selection);
        }
        if *self.layers() != layers {
            self.as_mut().set_layers(layers);
        }
        if *self.sheets() != sheets {
            self.as_mut().set_sheets(sheets);
        }
        if *self.title() != title {
            self.as_mut().set_title(title);
        }
        if *self.status() != status {
            self.as_mut().set_status(status);
        }
        self.as_mut().set_dirty(dirty);
        self.as_mut().set_can_undo(can_undo);
        self.as_mut().set_can_redo(can_redo);
        self.as_mut().set_has_file(has_file);
    }

    fn set_status_text(mut self: Pin<&mut Self>, msg: String) {
        self.as_mut().rust_mut().editor.status = msg;
        self.refresh();
    }

    pub fn press(self: Pin<&mut Self>, x: f64, y: f64, shift: bool, ctrl: bool) {
        self.edit(|e| e.press(Pt::new(x, y), shift, ctrl));
    }

    pub fn drag(self: Pin<&mut Self>, x: f64, y: f64, ctrl: bool) {
        self.edit(|e| {
            e.align = ctrl;
            e.drag_to(Pt::new(x, y))
        });
    }

    pub fn release(mut self: Pin<&mut Self>, x: f64, y: f64) {
        let cursor = self.as_mut().edit(|e| {
            e.release(Pt::new(x, y));
            e.hover(Pt::new(x, y))
        });
        self.set_cursor(cursor);
    }

    pub fn hover(mut self: Pin<&mut Self>, x: f64, y: f64) {
        let cursor = self.as_mut().edit(|e| e.hover(Pt::new(x, y)));
        self.set_cursor(cursor);
    }

    pub fn hover_out(self: Pin<&mut Self>) {
        self.edit(|e| e.hover_out())
    }

    pub fn double_click(self: Pin<&mut Self>, x: f64, y: f64) {
        self.edit(|e| e.double_click(Pt::new(x, y)))
    }

    pub fn begin_edit_selection(self: Pin<&mut Self>) -> bool {
        self.edit(|e| e.begin_edit_selection())
    }

    pub fn commit_edit(self: Pin<&mut Self>, text: &QString, w: f64, h: f64) {
        let text = text.to_string();
        self.edit(|e| e.commit_edit_sized(&text, w, h));
    }

    pub fn cancel_edit(self: Pin<&mut Self>) {
        self.edit(|e| e.cancel_edit());
    }

    pub fn cancel(self: Pin<&mut Self>) {
        self.edit(|e| e.cancel());
    }

    pub fn add_device_of(self: Pin<&mut Self>, kind: &QString, x: f64, y: f64) {
        let kind = kind.to_string();
        self.edit(|e| {
            if e.can_add() {
                e.add_device_near(&kind, Pt::new(x, y));
            }
        });
    }

    pub fn add_device(self: Pin<&mut Self>, x: f64, y: f64) {
        self.edit(|e| {
            if e.can_add() {
                e.add_device_near("", Pt::new(x, y));
            }
        });
    }

    pub fn add_point(self: Pin<&mut Self>, x: f64, y: f64) {
        self.edit(|e| {
            if e.can_add() {
                e.add_point_at(Pt::new(x, y));
            }
        });
    }

    pub fn add_note(self: Pin<&mut Self>, x: f64, y: f64) {
        self.edit(|e| {
            if e.can_add() {
                e.add_note_at(Pt::new(x, y));
            }
        });
    }

    pub fn quick_add(mut self: Pin<&mut Self>, spec: &QString, x: f64, y: f64) -> i32 {
        let spec = spec.to_string();
        let n = self
            .as_mut()
            .edit(|e| e.quick_add(&spec, Pt::new(x, y)).len());
        if n > 0 {
            self.set_status_text(format!("Added {n} device{}", if n == 1 { "" } else { "s" }));
        }
        n as i32
    }

    /// Put the selection (or whole diagram) on the clipboard as draw.io
    /// shapes. False if there was nothing to copy.
    pub fn copy_drawio(self: Pin<&mut Self>) -> bool {
        match self.edit(|e| e.copy_drawio().map(|xml| (xml, e.copy_native()))) {
            Some((xml, native)) => {
                qobject::set_clipboard(
                    &QString::from(xml.as_str()),
                    &QString::from(native.as_str()),
                );
                true
            }
            None => false,
        }
    }

    pub fn copy_sheet(self: Pin<&mut Self>) {
        let json = self.edit(|e| e.copy_sheet());
        qobject::set_clipboard(&QString::from(json.as_str()), &QString::from(json.as_str()));
    }

    /// Copy, then delete the selection. Nothing happens without one.
    pub fn set_deep_zoom(self: Pin<&mut Self>, on: bool) {
        self.edit(|e| {
            e.deep = on;
            e.status = if on {
                "Deep zoom: zoom into a device as far as you like and add things inside it; they are made \
                 at that zoom's scale and fade out when you zoom back out. Ctrl+I turns it off."
                    .into()
            } else {
                String::new()
            };
        });
    }

    pub fn view_changed(self: Pin<&mut Self>) {
        self.edit(|e| e.zoom_changed());
    }

    pub fn cut(mut self: Pin<&mut Self>) {
        if self.as_mut().rust_mut().editor.selection == crate::editor::Selection::None {
            self.set_status_text("Select something to cut.".into());
            return;
        }
        if self.as_mut().copy_drawio() {
            self.edit(|e| {
                e.delete_selected();
                e.status = "Cut: paste it with Ctrl+V, here or in draw.io.".into();
            });
        }
    }

    /// Paste into the visible area `x, y, w, h` (world coordinates).
    pub fn paste(self: Pin<&mut Self>, x: f64, y: f64, w: f64, h: f64) -> bool {
        let native = qobject::clipboard_native().to_string();
        let text = qobject::clipboard_text().to_string();
        self.edit(|e| e.paste(&native, &text, crate::model::Rect { x, y, w, h }))
    }

    pub fn switch_sheet(self: Pin<&mut Self>, index: i32) {
        self.edit(|e| e.switch_sheet(index.max(0) as usize));
    }

    pub fn add_sheet(self: Pin<&mut Self>) {
        self.edit(|e| {
            e.add_sheet();
        });
    }

    pub fn remove_sheet(self: Pin<&mut Self>, index: i32) -> bool {
        self.edit(|e| e.remove_sheet(index.max(0) as usize))
    }

    pub fn rename_sheet(self: Pin<&mut Self>, index: i32, name: &QString) {
        let name = name.to_string();
        self.edit(|e| {
            e.rename_sheet(index.max(0) as usize, &name);
        });
    }

    pub fn move_sheet(self: Pin<&mut Self>, from: i32, to: i32) {
        self.edit(|e| {
            e.move_sheet(from.max(0) as usize, to.max(0) as usize);
        });
    }

    pub fn add_layer(self: Pin<&mut Self>) {
        self.edit(|e| {
            e.add_layer();
        });
    }

    pub fn add_sublayer(self: Pin<&mut Self>) {
        self.edit(|e| {
            e.add_sublayer();
        });
    }

    pub fn move_layer(self: Pin<&mut Self>, id: i32, target: i32, place: &QString) -> bool {
        let place = place.to_string();
        self.edit(|e| e.move_layer(id as Id, target as Id, &place))
    }

    pub fn set_layer_expanded(self: Pin<&mut Self>, id: i32, expanded: bool) {
        self.edit(|e| e.set_layer_expanded(id as Id, expanded));
    }

    pub fn remove_layer(self: Pin<&mut Self>, id: i32) -> bool {
        self.edit(|e| e.remove_layer(id as Id))
    }

    pub fn set_layer_visible(self: Pin<&mut Self>, id: i32, visible: bool) {
        self.edit(|e| e.set_layer_visible(id as Id, visible));
    }

    pub fn set_layer_locked(self: Pin<&mut Self>, id: i32, locked: bool) {
        self.edit(|e| e.set_layer_locked(id as Id, locked));
    }

    pub fn set_layer_show_name(self: Pin<&mut Self>, id: i32, show: bool) {
        self.edit(|e| e.set_layer_show_name(id as Id, show));
    }

    pub fn rename_layer(self: Pin<&mut Self>, id: i32, name: &QString) {
        let name = name.to_string();
        self.edit(|e| {
            e.rename_layer(id as Id, &name);
        });
    }

    pub fn set_active_layer(self: Pin<&mut Self>, id: i32) {
        self.edit(|e| e.set_active_layer(id as Id));
    }

    pub fn auto_layout(
        self: Pin<&mut Self>,
        kind: &QString,
        gap: f64,
        tidy_ports: bool,
        avoid: bool,
    ) -> bool {
        let kind = kind.to_string();
        self.edit(|e| e.auto_layout(&kind, gap, tidy_ports, avoid))
    }

    pub fn arrange(self: Pin<&mut Self>, op: &QString) {
        let op = op.to_string();
        self.edit(|e| {
            e.arrange(&op);
        });
    }

    pub fn add_port(self: Pin<&mut Self>, side: &QString) {
        let side = Side::parse(&side.to_string()).unwrap_or(Side::Right);
        self.edit(|e| {
            e.add_port(side);
        });
    }

    pub fn delete_selected(self: Pin<&mut Self>) {
        self.edit(|e| e.delete_selected());
    }

    pub fn duplicate(self: Pin<&mut Self>) {
        self.edit(|e| e.duplicate());
    }

    pub fn select_all(self: Pin<&mut Self>) {
        self.edit(|e| e.select_all());
    }

    pub fn nudge(self: Pin<&mut Self>, dx: f64, dy: f64) {
        self.edit(|e| e.nudge(dx, dy));
    }

    pub fn reverse_link(self: Pin<&mut Self>) {
        self.edit(|e| e.reverse_link());
    }

    pub fn set_prop(self: Pin<&mut Self>, key: &QString, value: &QString) -> bool {
        let (k, v) = (key.to_string(), value.to_string());
        self.edit(|e| e.set_prop(&k, &v))
    }

    pub fn set_frame(self: Pin<&mut Self>, key: &QString, value: &QString) -> bool {
        let (k, v) = (key.to_string(), value.to_string());
        self.edit(|e| e.set_frame(&k, &v))
    }

    pub fn set_layer_background(
        self: Pin<&mut Self>,
        layer: i64,
        key: &QString,
        value: &QString,
    ) -> bool {
        let (k, v) = (key.to_string(), value.to_string());
        self.edit(|e| e.set_layer_background(layer as Id, &k, &v))
    }

    pub fn save_frame_defaults(self: Pin<&mut Self>, all_templates: bool) -> bool {
        self.edit(|e| e.save_frame_defaults(all_templates))
    }

    pub fn undo(self: Pin<&mut Self>) {
        self.edit(|e| e.undo());
    }

    pub fn redo(self: Pin<&mut Self>) {
        self.edit(|e| e.redo());
    }

    pub fn new_diagram(self: Pin<&mut Self>) {
        self.edit(|e| e.new_diagram());
    }

    pub fn open(self: Pin<&mut Self>, url: &QUrl) -> bool {
        let Some(path) = url_to_path(url) else {
            self.set_status_text("Only local files can be opened".into());
            return false;
        };
        self.open_path(path)
    }

    fn open_path(mut self: Pin<&mut Self>, path: PathBuf) -> bool {
        match self.as_mut().edit(|e| e.load(&path)) {
            Ok(()) => true,
            Err(msg) => {
                self.set_status_text(msg);
                false
            }
        }
    }

    pub fn save(mut self: Pin<&mut Self>) -> bool {
        let Some(path) = self.rust().editor.path.clone() else {
            return false;
        };
        match self.as_mut().edit(|e| e.save(&path)) {
            Ok(()) => true,
            Err(msg) => {
                self.set_status_text(msg);
                false
            }
        }
    }

    pub fn save_as(mut self: Pin<&mut Self>, url: &QUrl) -> bool {
        let Some(path) = url_to_path(url) else {
            self.set_status_text("Only local files can be saved".into());
            return false;
        };
        // Qt's own dialog does not always add the suffix itself.
        let path = crate::editor::with_file_ext(&path);
        match self.as_mut().edit(|e| e.save(&path)) {
            Ok(()) => true,
            Err(msg) => {
                self.set_status_text(msg);
                false
            }
        }
    }

    pub fn export_svg(self: Pin<&mut Self>, url: &QUrl) -> bool {
        let ed = &self.rust().editor;
        let svg = crate::svg::export_themed(&ed.doc, &ed.file_name(), ed.theme);
        self.write_export(url, svg)
    }

    pub fn set_theme(mut self: Pin<&mut Self>, name: &QString) {
        let theme = stratagram_core::theme::Theme::parse(&name.to_string()).unwrap_or_default();
        if self.rust().editor.theme != theme {
            self.as_mut().rust_mut().editor.theme = theme;
            self.refresh();
        }
    }

    pub fn export_png(self: Pin<&mut Self>, url: &QUrl) -> bool {
        let Some(path) = url_to_path(url) else {
            self.set_status_text("Only local files can be written".into());
            return false;
        };
        let path = if path.extension().is_none() {
            path.with_extension("png")
        } else {
            path
        };
        let ed = &self.rust().editor;
        let svg = crate::svg::export_themed(&ed.doc, &ed.file_name(), ed.theme);
        match export_png(&svg, &path, PNG_SCALE) {
            Ok(()) => {
                self.set_status_text(format!("Exported {}", path.display()));
                true
            }
            Err(e) => {
                self.set_status_text(format!("PNG export failed: {e}"));
                false
            }
        }
    }

    pub fn export_drawio(self: Pin<&mut Self>, url: &QUrl) -> bool {
        let title = self.file_base_name().to_string();
        let ed = &self.rust().editor;
        let xml = ed.export_drawio(&title);
        self.write_export(url, xml)
    }

    pub fn export_cml(mut self: Pin<&mut Self>, url: &QUrl) -> bool {
        let title = self.file_base_name().to_string();
        let (yaml, skipped) = stratagram_core::cml::export(&self.rust().editor.doc, &title);
        let ok = self.as_mut().write_export(url, yaml);
        let left_out = skipped.summary();
        if ok && !left_out.is_empty() {
            let status = format!(
                "{}; {left_out} (not cables in a lab).",
                self.rust().editor.status
            );
            self.set_status_text(status);
        }
        ok
    }

    fn write_export(self: Pin<&mut Self>, url: &QUrl, content: String) -> bool {
        let Some(path) = url_to_path(url) else {
            self.set_status_text("Only local files can be written".into());
            return false;
        };
        let msg = match std::fs::write(&path, content) {
            Ok(()) => format!("Exported {}", path.display()),
            Err(e) => format!("cannot write {}: {e}", path.display()),
        };
        let ok = msg.starts_with("Exported");
        self.set_status_text(msg);
        ok
    }

    /// Open the file given on the command line, if any.
    pub fn startup(mut self: Pin<&mut Self>) {
        if let Some(arg) = std::env::args().nth(1) {
            let path = PathBuf::from(&arg);
            if path.exists() {
                self.open_path(path);
            } else {
                // Start a new diagram that will be saved under that name.
                let mut rust = self.as_mut().rust_mut();
                rust.editor.path = Some(path);
                rust.editor.auto_frame();
                self.as_mut().refresh();
                self.set_status_text(format!("New file {arg}"));
            }
        } else {
            self.as_mut().rust_mut().editor.auto_frame();
            self.as_mut().refresh();
            self.set_status_text(
                "Double-click to add a device. Drag from a device border or port to another device to connect.".into(),
            );
        }
    }

    pub fn config_path(&self) -> QString {
        QString::from(crate::config::shown(&crate::config::path()).as_str())
    }

    pub fn file_folder(&self) -> QUrl {
        let ed = &self.rust().editor;
        let dir = ed
            .path
            .as_ref()
            .and_then(|p| std::path::absolute(p).ok())
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .or_else(|| {
                ed.last_dir
                    .as_ref()
                    .and_then(|d| std::path::absolute(d).ok())
            })
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        QUrl::from_local_file(&QString::from(dir.to_string_lossy().as_ref()))
    }

    pub fn version(&self) -> QString {
        QString::from(env!("CARGO_PKG_VERSION"))
    }

    pub fn file_base_name(&self) -> QString {
        let stem = self
            .rust()
            .editor
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "diagram".into());
        QString::from(stem.as_str())
    }
}
