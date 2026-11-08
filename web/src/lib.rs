// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Stratagram's editor in the browser: [`Editor`] behind a small JavaScript
//! interface, like the desktop app's `DiagramController` (src/bridge.rs).
//! Every call changes the editor; the page then reads what it shows from
//! [`Stratagram::scene`], [`Stratagram::selection`], [`Stratagram::layers`]
//! and the rest, as JSON or plain values. Files, the clipboard and the
//! configuration file are the page's business: the editor works on text.

use std::path::Path;

use stratagram_core::editor::Editor;
use stratagram_core::model::{Id, Pt, Rect, Side};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Stratagram {
    editor: Editor,
}

impl Default for Stratagram {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Stratagram {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Stratagram {
        Stratagram { editor: Editor::default() }
    }

    /// The program version.
    pub fn version() -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    // output

    pub fn scene(&self) -> String {
        self.editor.scene_json()
    }
    pub fn selection(&self) -> String {
        self.editor.selection_json()
    }
    pub fn layers(&self) -> String {
        self.editor.layers_json()
    }
    /// The sheets: [{"name", "active", "empty"}].
    pub fn sheets(&self) -> String {
        self.editor.sheets_json()
    }
    #[wasm_bindgen(js_name = editBox)]
    pub fn edit_box(&self) -> String {
        self.editor.edit_box_json()
    }
    pub fn title(&self) -> String {
        self.editor.title()
    }
    pub fn status(&self) -> String {
        self.editor.status.clone()
    }
    #[wasm_bindgen(js_name = setStatus)]
    pub fn set_status(&mut self, text: &str) {
        self.editor.status = text.into();
    }
    pub fn dirty(&self) -> bool {
        self.editor.dirty
    }
    #[wasm_bindgen(js_name = canUndo)]
    pub fn can_undo(&self) -> bool {
        self.editor.can_undo()
    }
    #[wasm_bindgen(js_name = canRedo)]
    pub fn can_redo(&self) -> bool {
        self.editor.can_redo()
    }
    /// The diagram's file name ("" before it was saved).
    #[wasm_bindgen(js_name = fileName)]
    pub fn file_name(&self) -> String {
        self.editor.file_name()
    }

    // view

    #[wasm_bindgen(js_name = setView)]
    pub fn set_view(&mut self, zoom: f64, snap: bool) {
        self.editor.zoom = zoom;
        self.editor.snap = snap;
        self.editor.zoom_changed();
    }
    #[wasm_bindgen(js_name = setDeepZoom)]
    pub fn set_deep_zoom(&mut self, on: bool) {
        let e = &mut self.editor;
        e.deep = on;
        e.status = if on {
            "Deep zoom: zoom into a device as far as you like and add things inside it; they are made \
             at that zoom's scale and fade out when you zoom back out. Ctrl+I turns it off."
                .into()
        } else {
            String::new()
        };
    }

    // pointer and keys

    pub fn press(&mut self, x: f64, y: f64, shift: bool, ctrl: bool) {
        self.editor.press(Pt::new(x, y), shift, ctrl);
    }
    /// `ctrl` is read on every move, so holding or letting go of Ctrl
    /// mid-drag switches alignment on and off.
    pub fn drag(&mut self, x: f64, y: f64, ctrl: bool) {
        self.editor.align = ctrl;
        self.editor.drag_to(Pt::new(x, y));
    }
    /// Returns the cursor to show (see [`Stratagram::hover`]).
    pub fn release(&mut self, x: f64, y: f64) -> i32 {
        self.editor.release(Pt::new(x, y));
        self.editor.hover(Pt::new(x, y))
    }
    /// The cursor to show: 0 arrow, 2 cross, 8 resize, 9 move, 13 hand.
    pub fn hover(&mut self, x: f64, y: f64) -> i32 {
        self.editor.hover(Pt::new(x, y))
    }
    /// The pointer left the canvas.
    #[wasm_bindgen(js_name = hoverOut)]
    pub fn hover_out(&mut self) {
        self.editor.hover_out()
    }
    #[wasm_bindgen(js_name = doubleClick)]
    pub fn double_click(&mut self, x: f64, y: f64) {
        self.editor.double_click(Pt::new(x, y));
    }
    #[wasm_bindgen(js_name = beginEditSelection)]
    pub fn begin_edit_selection(&mut self) -> bool {
        self.editor.begin_edit_selection()
    }
    #[wasm_bindgen(js_name = commitEdit)]
    pub fn commit_edit(&mut self, text: &str, w: f64, h: f64) {
        self.editor.commit_edit_sized(text, w, h);
    }
    #[wasm_bindgen(js_name = cancelEdit)]
    pub fn cancel_edit(&mut self) {
        self.editor.cancel_edit();
    }
    pub fn cancel(&mut self) {
        self.editor.cancel();
    }
    pub fn nudge(&mut self, dx: f64, dy: f64) {
        self.editor.nudge(dx, dy);
    }

    // adding

    #[wasm_bindgen(js_name = addDevice)]
    pub fn add_device(&mut self, x: f64, y: f64) {
        if self.editor.can_add() {
            self.editor.add_device_near("", Pt::new(x, y));
        }
    }
    /// A device of type `kind` ("" for a plain one), named after its type.
    #[wasm_bindgen(js_name = addDeviceOf)]
    pub fn add_device_of(&mut self, kind: &str, x: f64, y: f64) {
        if self.editor.can_add() {
            self.editor.add_device_near(kind, Pt::new(x, y));
        }
    }
    #[wasm_bindgen(js_name = addNote)]
    pub fn add_note(&mut self, x: f64, y: f64) {
        if self.editor.can_add() {
            self.editor.add_note_at(Pt::new(x, y));
        }
    }
    #[wasm_bindgen(js_name = addPoint)]
    pub fn add_point(&mut self, x: f64, y: f64) {
        if self.editor.can_add() {
            self.editor.add_point_at(Pt::new(x, y));
        }
    }
    /// Devices typed comma-separated ("Firewall, router|edge-01"); returns
    /// how many were added.
    #[wasm_bindgen(js_name = quickAdd)]
    pub fn quick_add(&mut self, spec: &str, x: f64, y: f64) -> u32 {
        let n = self.editor.quick_add(spec, Pt::new(x, y)).len();
        if n > 0 {
            self.editor.status = format!("Added {n} device{}", if n == 1 { "" } else { "s" });
        }
        n as u32
    }
    #[wasm_bindgen(js_name = addPort)]
    pub fn add_port(&mut self, side: &str) {
        self.editor.add_port(Side::parse(side).unwrap_or(Side::Right));
    }

    // editing

    #[wasm_bindgen(js_name = deleteSelected)]
    pub fn delete_selected(&mut self) {
        self.editor.delete_selected();
    }
    pub fn duplicate(&mut self) {
        self.editor.duplicate();
    }
    #[wasm_bindgen(js_name = selectAll)]
    pub fn select_all(&mut self) {
        self.editor.select_all();
    }
    #[wasm_bindgen(js_name = reverseLink)]
    pub fn reverse_link(&mut self) {
        self.editor.reverse_link();
    }
    pub fn arrange(&mut self, op: &str) {
        self.editor.arrange(op);
    }
    #[wasm_bindgen(js_name = autoLayout)]
    pub fn auto_layout(&mut self, kind: &str, gap: f64, tidy_ports: bool, avoid: bool) -> bool {
        self.editor.auto_layout(kind, gap, tidy_ports, avoid)
    }
    #[wasm_bindgen(js_name = setProp)]
    pub fn set_prop(&mut self, key: &str, value: &str) -> bool {
        self.editor.set_prop(key, value)
    }
    /// A frame field; a logo is given as a `data:` URI.
    #[wasm_bindgen(js_name = setFrame)]
    pub fn set_frame(&mut self, key: &str, value: &str) -> bool {
        self.editor.set_frame(key, value)
    }
    #[wasm_bindgen(js_name = setLayerBackground)]
    pub fn set_layer_background(&mut self, layer: f64, key: &str, value: &str) -> bool {
        self.editor.set_layer_background(layer as Id, key, value)
    }
    pub fn undo(&mut self) {
        self.editor.undo();
    }
    pub fn redo(&mut self) {
        self.editor.redo();
    }

    // sheets

    #[wasm_bindgen(js_name = switchSheet)]
    pub fn switch_sheet(&mut self, index: u32) {
        self.editor.switch_sheet(index as usize);
    }
    #[wasm_bindgen(js_name = addSheet)]
    pub fn add_sheet(&mut self) {
        self.editor.add_sheet();
    }
    #[wasm_bindgen(js_name = removeSheet)]
    pub fn remove_sheet(&mut self, index: u32) -> bool {
        self.editor.remove_sheet(index as usize)
    }
    /// Move sheet `from` to position `to` (undoable).
    #[wasm_bindgen(js_name = moveSheet)]
    pub fn move_sheet(&mut self, from: u32, to: u32) {
        self.editor.move_sheet(from as usize, to as usize);
    }
    #[wasm_bindgen(js_name = renameSheet)]
    pub fn rename_sheet(&mut self, index: u32, name: &str) {
        self.editor.rename_sheet(index as usize, name);
    }

    // layers

    #[wasm_bindgen(js_name = addLayer)]
    pub fn add_layer(&mut self) {
        self.editor.add_layer();
    }
    /// A layer under the active one, made the active one.
    #[wasm_bindgen(js_name = addSublayer)]
    pub fn add_sublayer(&mut self) {
        self.editor.add_sublayer();
    }
    /// Move a layer (with those under it) "before" or "after" another, or
    /// "into" it.
    #[wasm_bindgen(js_name = moveLayer)]
    pub fn move_layer(&mut self, id: f64, target: f64, place: &str) -> bool {
        self.editor.move_layer(id as Id, target as Id, place)
    }
    /// Show a layer's sub-layers in the list, or fold them away.
    #[wasm_bindgen(js_name = setLayerExpanded)]
    pub fn set_layer_expanded(&mut self, id: f64, expanded: bool) {
        self.editor.set_layer_expanded(id as Id, expanded);
    }
    #[wasm_bindgen(js_name = removeLayer)]
    pub fn remove_layer(&mut self, id: f64) -> bool {
        self.editor.remove_layer(id as Id)
    }
    #[wasm_bindgen(js_name = setLayerVisible)]
    pub fn set_layer_visible(&mut self, id: f64, visible: bool) {
        self.editor.set_layer_visible(id as Id, visible);
    }
    #[wasm_bindgen(js_name = setLayerLocked)]
    pub fn set_layer_locked(&mut self, id: f64, locked: bool) {
        self.editor.set_layer_locked(id as Id, locked);
    }
    #[wasm_bindgen(js_name = setLayerShowName)]
    pub fn set_layer_show_name(&mut self, id: f64, show: bool) {
        self.editor.set_layer_show_name(id as Id, show);
    }
    #[wasm_bindgen(js_name = renameLayer)]
    pub fn rename_layer(&mut self, id: f64, name: &str) {
        self.editor.rename_layer(id as Id, name);
    }
    #[wasm_bindgen(js_name = setActiveLayer)]
    pub fn set_active_layer(&mut self, id: f64) {
        self.editor.set_active_layer(id as Id);
    }

    // clipboard

    /// The selection (or everything) as draw.io shapes, "" if there is
    /// nothing; [`Stratagram::copy_native`] has the same for pasting here.
    #[wasm_bindgen(js_name = copyDrawio)]
    pub fn copy_drawio(&mut self) -> String {
        self.editor.copy_drawio().unwrap_or_default()
    }
    #[wasm_bindgen(js_name = copyNative)]
    pub fn copy_native(&self) -> String {
        self.editor.copy_native()
    }
    /// The whole sheet with its layers and frame, as text to paste into
    /// Stratagram (here, in another window, in the app).
    #[wasm_bindgen(js_name = copySheet)]
    pub fn copy_sheet(&mut self) -> String {
        self.editor.copy_sheet()
    }
    /// Paste a Stratagram copy, draw.io shapes or text into the visible area.
    pub fn paste(&mut self, native: &str, text: &str, x: f64, y: f64, w: f64, h: f64) -> bool {
        self.editor.paste(native, text, Rect { x, y, w, h })
    }

    // NetBox

    /// A new diagram of a NetBox site: `json` has the site's `devices`,
    /// `interfaces` and `cables` as NetBox's REST API gives them (and the
    /// `site`), `options` how to map them (JSON, "" for the defaults).
    /// Returns "" or what went wrong.
    #[wasm_bindgen(js_name = importNetbox)]
    pub fn import_netbox(&mut self, json: &str, options: &str) -> String {
        self.editor.import_netbox(json, options).err().unwrap_or_default()
    }

    /// Bring the diagram up to date with the site in NetBox (undoable).
    /// Returns "" or what went wrong; the status says what changed.
    #[wasm_bindgen(js_name = refreshNetbox)]
    pub fn refresh_netbox(&mut self, json: &str, options: &str) -> String {
        self.editor.refresh_netbox(json, options).err().unwrap_or_default()
    }

    /// Show the sheet standing for `source` ("dcim.location:4"); false if
    /// there is none.
    #[wasm_bindgen(js_name = showSheetOf)]
    pub fn show_sheet_of(&mut self, source: &str) -> bool {
        self.editor.show_sheet_of(source)
    }

    // files

    /// A new, empty diagram.
    #[wasm_bindgen(js_name = newDiagram)]
    pub fn new_diagram(&mut self) {
        self.editor.new_diagram();
    }
    /// Open a diagram from its text, under `name`. Returns "" or what went wrong.
    pub fn load(&mut self, json: &str, name: &str) -> String {
        match self.editor.load_json(json, Path::new(name)) {
            Ok(()) => String::new(),
            Err(e) => {
                self.editor.status = format!("Cannot open {name}: {e}");
                e
            }
        }
    }
    /// The document (all sheets) as a .stratagram file.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self) -> String {
        self.editor.document_json()
    }
    /// The diagram was stored under `name`: no unsaved changes any more.
    #[wasm_bindgen(js_name = savedAs)]
    pub fn saved_as(&mut self, name: &str) {
        self.editor.saved_as(name);
    }
    /// The sheet in view as SVG, in the theme's colors.
    #[wasm_bindgen(js_name = exportSvg)]
    pub fn export_svg(&self) -> String {
        stratagram_core::svg::export_themed(&self.editor.doc, &self.editor.file_name(), self.editor.theme)
    }
    /// Draw (and export) in a theme: "light", "deep" or "neon". False for
    /// an unknown name (the theme stays).
    /// How new lines run: "straight", "orthogonal" or "around". False for
    /// an unknown name.
    #[wasm_bindgen(js_name = setLineRouting)]
    pub fn set_line_routing(&mut self, name: &str) -> bool {
        match stratagram_core::model::Routing::parse(name) {
            Some(r) => {
                self.editor.link_defaults.routing = r;
                true
            }
            None => false,
        }
    }

    #[wasm_bindgen(js_name = setTheme)]
    pub fn set_theme(&mut self, name: &str) -> bool {
        match stratagram_core::theme::Theme::parse(name) {
            Some(t) => {
                self.editor.theme = t;
                true
            }
            None => false,
        }
    }
    pub fn theme(&self) -> String {
        self.editor.theme.as_str().into()
    }
    /// The sheet in view as a Cisco Modeling Labs lab (YAML); what was
    /// left out (tunnels...) goes to the status.
    #[wasm_bindgen(js_name = exportCml)]
    pub fn export_cml(&mut self, title: &str) -> String {
        let (yaml, skipped) = stratagram_core::cml::export(&self.editor.doc, title);
        let left_out = skipped.summary();
        self.editor.status = if left_out.is_empty() { String::new() } else { format!("{left_out} (not cables in a lab).") };
        yaml
    }
    #[wasm_bindgen(js_name = exportDrawio)]
    pub fn export_drawio(&self, title: &str) -> String {
        self.editor.export_drawio(title)
    }
}
