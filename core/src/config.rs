// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! The configuration file, `stratagram.conf` in the user's config folder
//! (e.g. `~/.config/stratagram.conf`). The interface keeps its settings there
//! (section `[ui]`, written by Qt); you can add sections of your own, such
//! as the defaults for frames:
//!
//! ```ini
//! [frame]
//! auto=true
//! template=site
//! author=Shin
//! company=ACME Networks
//! logo=/home/shin/Pictures/acme.png
//!
//! [frame-classification]
//! level=confidential
//! ```
//!
//! It is read when needed, so changes apply without a restart.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The system a folder is chosen for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum System {
    Linux,
    Windows,
    MacOs,
}

const SYSTEM: System = if cfg!(windows) {
    System::Windows
} else if cfg!(target_os = "macos") {
    System::MacOs
} else {
    System::Linux
};

/// Where the file is: on Linux (and other Unix) `$XDG_CONFIG_HOME/stratagram.conf`,
/// else `~/.config/stratagram.conf`; on Windows `%APPDATA%\stratagram.conf`; on
/// macOS `~/Library/Application Support/stratagram.conf`. Saved logos go
/// into `stratagram/logos/` beside it.
pub fn path() -> PathBuf {
    config_dir(SYSTEM, |k| std::env::var_os(k)).join("stratagram.conf")
}

/// The folder for configuration files on `system`, from environment
/// variables `var` (the current folder if none is set).
fn config_dir(system: System, var: impl Fn(&str) -> Option<OsString>) -> PathBuf {
    let set = |k: &str| var(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let home = || set("HOME").or_else(|| set("USERPROFILE"));
    match system {
        System::Windows => set("APPDATA").or_else(|| home().map(|h| h.join("AppData").join("Roaming"))),
        System::MacOs => home().map(|h| h.join("Library").join("Application Support")),
        System::Linux => set("XDG_CONFIG_HOME").or_else(|| home().map(|h| h.join(".config"))),
    }
    .unwrap_or_else(|| PathBuf::from("."))
}

/// Move the settings of the app's former name (netdiag) over, once: the
/// configuration file (`netdiag.conf`) and the folder with saved logos
/// (`netdiag/`), with the logo paths in the file updated. Nothing happens
/// if the new file exists already or there is nothing to move.
pub fn migrate_from_netdiag() {
    if let Some(dir) = path().parent() {
        migrate_in(dir);
    }
}

/// [`migrate_from_netdiag`] in the config folder `dir`.
fn migrate_in(dir: &Path) {
    let new = dir.join("stratagram.conf");
    let old = dir.join("netdiag.conf");
    if new.exists() || !old.exists() {
        return;
    }
    let (old_dir, new_dir) = (dir.join("netdiag"), dir.join("stratagram"));
    if old_dir.is_dir() && !new_dir.exists() && std::fs::rename(&old_dir, &new_dir).is_err() {
        return;
    }
    let Ok(text) = std::fs::read_to_string(&old) else { return };
    // The logo paths in it, with either separator (\\ on Windows).
    let mut moved = text;
    for sep in ['/', '\\'] {
        moved = moved.replace(&format!("{}{sep}", old_dir.display()), &format!("{}{sep}", new_dir.display()));
    }
    if std::fs::write(&new, moved).is_ok() {
        let _ = std::fs::remove_file(&old);
    }
}

/// `path` shortened with ~ for the home folder, for showing it.
pub fn shown(path: &Path) -> String {
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty()).or_else(|| std::env::var_os("USERPROFILE"));
    home.and_then(|h| path.strip_prefix(h).ok().map(|rest| Path::new("~").join(rest).display().to_string()))
        .unwrap_or_else(|| path.display().to_string())
}

/// The sections of an INI file, in order, with their keys and values.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Config {
    sections: Vec<(String, Vec<(String, String)>)>,
}

impl Config {
    /// The file at `path`; empty if it is missing or unreadable.
    pub fn load(path: &Path) -> Config {
        std::fs::read_to_string(path).map(|t| Config::parse(&t)).unwrap_or_default()
    }

    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        let mut current: Option<usize> = None;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                c.sections.push((name.trim().to_string(), vec![]));
                current = Some(c.sections.len() - 1);
                continue;
            }
            let (Some(i), Some((k, v))) = (current, line.split_once('=')) else { continue };
            c.sections[i].1.push((k.trim().to_string(), unquote(v.trim())));
        }
        c
    }

    /// The keys and values of `section` (the last one wins for repeats).
    pub fn section(&self, name: &str) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = vec![];
        for (k, v) in self.sections.iter().filter(|(n, _)| n == name).flat_map(|(_, kv)| kv) {
            out.retain(|(x, _)| x != k);
            out.push((k.clone(), v.clone()));
        }
        out
    }

    pub fn get(&self, section: &str, key: &str) -> Option<String> {
        self.section(section).into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// `key` read as a yes/no setting.
    pub fn flag(&self, section: &str, key: &str) -> bool {
        self.get(section, key).is_some_and(|v| matches!(v.to_lowercase().as_str(), "true" | "yes" | "1" | "on"))
    }
}

/// Set keys in `section` of the file at `path` (`None` removes the key),
/// leaving everything else as it is. The section is added if missing; the
/// file is created if there is none.
pub fn set_values(path: &Path, section: &str, values: &[(&str, Option<String>)]) -> std::io::Result<()> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let header = format!("[{section}]");
    let start = lines.iter().position(|l| l.trim() == header);
    let start = match start {
        Some(i) => i,
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(header);
            lines.len() - 1
        }
    };
    // The section runs to the next header.
    let end = |lines: &[String]| {
        lines[start + 1..].iter().position(|l| l.trim_start().starts_with('[')).map_or(lines.len(), |i| start + 1 + i)
    };
    for (key, value) in values {
        let is_key = |l: &String| l.split_once('=').is_some_and(|(k, _)| k.trim() == *key);
        let found = (start + 1..end(&lines)).find(|&i| is_key(&lines[i]));
        match (found, value) {
            (Some(i), Some(v)) => lines[i] = format!("{key}={}", quote(v)),
            (Some(i), None) => {
                lines.remove(i);
            }
            (None, Some(v)) => {
                // After the section's last entry, before blank lines.
                let mut at = end(&lines);
                while at > start + 1 && lines[at - 1].trim().is_empty() {
                    at -= 1;
                }
                lines.insert(at, format!("{key}={}", quote(v)));
            }
            (None, None) => {}
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("conf.tmp~");
    std::fs::write(&tmp, lines.join("\n") + "\n")?;
    std::fs::rename(&tmp, path)
}

/// A value as Qt reads it back as one string: in double quotes when it has
/// commas, quotes, semicolons or spaces at either end.
fn quote(v: &str) -> String {
    let plain = !v.contains([',', '"', ';', '\\', '\n']) && v.trim() == v;
    if plain {
        return v.to_string();
    }
    let escaped = v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
    format!("\"{escaped}\"")
}

/// A value as Qt writes it: possibly in double quotes, with `\"` and `\\`.
fn unquote(v: &str) -> String {
    let Some(inner) = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else { return v.to_string() };
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_is_where_each_system_keeps_settings() {
        fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
            let pairs: Vec<(String, String)> = pairs.iter().map(|(n, v)| (n.to_string(), v.to_string())).collect();
            move |k| pairs.iter().find(|(n, _)| n == k).map(|(_, v)| OsString::from(v))
        }
        let linux = [("HOME", "/home/u"), ("XDG_CONFIG_HOME", "")];
        assert_eq!(config_dir(System::Linux, env(&linux)), PathBuf::from("/home/u/.config"));
        assert_eq!(config_dir(System::Linux, env(&[("HOME", "/home/u"), ("XDG_CONFIG_HOME", "/cfg")])), PathBuf::from("/cfg"));
        let win = [("APPDATA", r"C:\Users\u\AppData\Roaming"), ("USERPROFILE", r"C:\Users\u")];
        assert_eq!(config_dir(System::Windows, env(&win)), PathBuf::from(r"C:\Users\u\AppData\Roaming"));
        assert_eq!(config_dir(System::Windows, env(&[("USERPROFILE", "/u")])), PathBuf::from("/u").join("AppData").join("Roaming"));
        assert_eq!(config_dir(System::MacOs, env(&[("HOME", "/Users/u")])), PathBuf::from("/Users/u/Library/Application Support"));
        assert_eq!(config_dir(System::Linux, env(&[])), PathBuf::from("."), "nothing set");
    }

    #[test]
    fn reads_sections_as_qt_writes_them() {
        let c = Config::parse(
            "[ui]\npanelVisible=true\nlayoutGap=60\n\n; a comment\n[frame]\nauto = true\nauthor=Shin\n\
             address=\"Hauptstr. 1, Berlin\"\ncolor=#1e3a8a\nauthor=Kim\n\n[frame-site]\ndoc_id=BER-01\n",
        );
        assert_eq!(c.get("ui", "layoutGap").as_deref(), Some("60"));
        assert!(c.flag("frame", "auto") && !c.flag("frame", "missing"));
        assert_eq!(c.get("frame", "address").as_deref(), Some("Hauptstr. 1, Berlin"));
        assert_eq!(c.get("frame", "color").as_deref(), Some("#1e3a8a"), "a # inside a value is kept");
        assert_eq!(c.get("frame", "author").as_deref(), Some("Kim"), "the last one wins");
        assert_eq!(c.section("frame-site"), vec![("doc_id".to_string(), "BER-01".to_string())]);
        assert!(c.section("nothing").is_empty());
        assert_eq!(unquote(r#""say \"hi\"\\""#), r#"say "hi"\"#);
    }

    #[test]
    fn settings_move_over_from_the_former_name() {
        let dir = std::env::temp_dir().join(format!("stratagram-migrate-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("netdiag").join("logos")).unwrap();
        std::fs::write(dir.join("netdiag").join("logos").join("frame-logo.png"), b"png").unwrap();
        let logo = dir.join("netdiag").join("logos").join("frame-logo.png");
        std::fs::write(dir.join("netdiag.conf"), format!("[ui]\nshowGrid=false\n\n[frame]\nauthor=Shin\nlogo={}\n", logo.display())).unwrap();
        migrate_in(&dir);
        let c = Config::load(&dir.join("stratagram.conf"));
        assert_eq!(c.get("ui", "showGrid").as_deref(), Some("false"));
        let moved = dir.join("stratagram").join("logos").join("frame-logo.png");
        assert_eq!(c.get("frame", "logo").map(std::path::PathBuf::from), Some(moved.clone()));
        assert!(moved.exists() && !dir.join("netdiag.conf").exists() && !dir.join("netdiag").exists());
        // Once only: an existing new file is left alone.
        std::fs::write(dir.join("netdiag.conf"), "[ui]\nshowGrid=true\n").unwrap();
        migrate_in(&dir);
        assert_eq!(Config::load(&dir.join("stratagram.conf")).get("ui", "showGrid").as_deref(), Some("false"));
        std::fs::remove_dir_all(&dir).ok();
        // Logo paths written with Windows separators move over too.
        let dir = std::env::temp_dir().join(format!("stratagram-migrate-win-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("netdiag")).unwrap();
        let old = format!("{}\\logos\\a.png", dir.join("netdiag").display());
        std::fs::write(dir.join("netdiag.conf"), format!("[frame]\nlogo={old}\n")).unwrap();
        migrate_in(&dir);
        let new = format!("{}\\logos\\a.png", dir.join("stratagram").display());
        assert_eq!(Config::load(&dir.join("stratagram.conf")).get("frame", "logo"), Some(new));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn writes_keys_into_one_section_and_keeps_the_rest() {
        let dir = std::env::temp_dir().join(format!("stratagram-ini-{}", std::process::id()));
        let path = dir.join("stratagram.conf");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, "[frame]\nauthor=Shin\nnote=old\n\n[ui]\nshowGrid=true\n").unwrap();
        set_values(&path, "frame", &[("note", None), ("company", Some("ACME, Inc.".into())), ("author", Some("Kim".into()))]).unwrap();
        set_values(&path, "frame-site", &[("doc_id", Some("BER-01".into()))]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text, "[frame]\nauthor=Kim\ncompany=\"ACME, Inc.\"\n\n[ui]\nshowGrid=true\n\n[frame-site]\ndoc_id=BER-01\n");
        let c = Config::parse(&text);
        assert_eq!(c.get("frame", "company").as_deref(), Some("ACME, Inc."));
        assert_eq!(c.get("ui", "showGrid").as_deref(), Some("true"));
        // A new file is created, with its folder.
        let fresh = dir.join("sub").join("stratagram.conf");
        set_values(&fresh, "frame", &[("author", Some("Lee".into()))]).unwrap();
        assert_eq!(std::fs::read_to_string(&fresh).unwrap(), "[frame]\nauthor=Lee\n");
        std::fs::remove_dir_all(&dir).ok();
    }
}
