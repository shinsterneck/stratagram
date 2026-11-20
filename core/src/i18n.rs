// SPDX-License-Identifier: Apache-2.0
use std::{collections::HashMap, sync::LazyLock};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Language {
    #[default]
    English,
    Chinese,
}

type Catalog = HashMap<String, String>;
static EN: LazyLock<Catalog> = LazyLock::new(|| serde_json::from_str(include_str!("../locales/en.json")).unwrap());
static ZH: LazyLock<Catalog> = LazyLock::new(|| serde_json::from_str(include_str!("../locales/zh-CN.json")).unwrap());

impl Language {
    pub fn from_tag(tag: &str) -> Self {
        match tag.split(['-', '_', '.', '@']).next() {
            Some(base) if base.eq_ignore_ascii_case("zh") => Self::Chinese,
            _ => Self::English,
        }
    }

    pub fn lookup(self, key: &str) -> Option<&'static str> {
        let catalog = match self { Self::English => &EN, Self::Chinese => &ZH };
        catalog.get(key).or_else(|| EN.get(key)).map(String::as_str)
    }

    pub fn text(self, key: &'static str) -> &'static str {
        self.lookup(key).unwrap_or(key)
    }

    pub fn numbered(self, key: &'static str, number: usize) -> String {
        self.format(key, &[("number", &number.to_string())])
    }

    /// Substitute each token once. Braces in filenames and user text are literal.
    pub fn format(self, key: &'static str, args: &[(&str, &str)]) -> String {
        let mut rest = self.text(key);
        let mut out = String::with_capacity(rest.len());
        while let Some((prefix, token)) = rest.split_once('{') {
            out.push_str(prefix);
            let Some((name, tail)) = token.split_once('}') else {
                out.push('{');
                rest = token;
                break;
            };
            if let Some((_, value)) = args.iter().find(|(k, _)| *k == name) {
                out.push_str(value);
            } else {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
            rest = tail;
        }
        out.push_str(rest);
        out
    }
}

/// A message retained for redisplay after the user changes languages.
#[derive(Debug)]
pub(crate) struct Diagnostic {
    key: &'static str,
    args: Vec<(&'static str, String)>,
}

impl Diagnostic {
    pub(crate) fn new<const N: usize>(key: &'static str, args: [(&'static str, String); N]) -> Self {
        Self { key, args: args.into() }
    }

    pub(crate) fn render(&self, lang: Language) -> String {
        let args: Vec<_> = self.args.iter().map(|(key, value)| (*key, value.as_str())).collect();
        lang.format(self.key, &args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{editor::Editor, model::Pt};

    #[test]
    fn cml_export_retranslates_the_summary_without_reinterpreting_the_filename() {
        let mut editor = Editor::default();
        let name = "Berlin; {details} $&.yaml";
        editor.report_cml_export(Some(name.into()), crate::cml::Skipped { tunnels: 3, ..Default::default() });
        let english = format!("Exported {name}; left out: 3 tunnel lines (not cables in a lab)");
        assert_eq!(editor.status, english);
        editor.set_language(Language::Chinese);
        assert_eq!(editor.status, format!("已导出 {name}；已跳过：3 条隧道链路（无法转换为实验拓扑中的线缆）"));
        editor.set_language(Language::English);
        assert_eq!(editor.status, english);
        editor.report_cml_export(Some(name.into()), Default::default());
        editor.set_language(Language::Chinese);
        assert_eq!(editor.status, format!("已导出 {name}"));
        editor.report_cml_export(None, Default::default());
        assert!(editor.status.is_empty());
    }

    #[test]
    fn switching_languages_rerenders_file_errors_without_parsing_values() {
        let mut editor = Editor::default();
        let filename = std::path::Path::new("a: {error}.stratagram");
        editor.set_language(Language::Chinese);
        assert!(editor.load_json(r#"{"format":"bad: {name}","version":1}"#, filename).is_err());
        assert_eq!(editor.status, "无法打开 a: {error}.stratagram：文件格式应为 stratagram，实际为“bad: {name}”");
        editor.set_language(Language::English);
        assert_eq!(editor.status, r#"Cannot open a: {error}.stratagram: not a stratagram file (format "bad: {name}")"#);
        editor.set_language(Language::Chinese);
        assert!(editor.status.contains("文件格式应为 stratagram"));
        editor.status = "Saved ok.stratagram".to_owned();
        editor.set_language(Language::English);
        assert_eq!(editor.status, "Saved ok.stratagram");
    }

    #[test]
    fn bad_logo_status_retains_the_error_key_across_language_changes() {
        let mut editor = Editor::default();
        editor.set_frame("template", "iso");
        assert!(!editor.set_frame("logo", "data:image/png;base64,QkM="));
        assert_eq!(editor.status, "Logo not set: the logo must be a PNG or JPEG image.");
        editor.set_language(Language::Chinese);
        assert_eq!(editor.status, "设置标志失败：标志图像必须为 PNG 或 JPEG 格式。");
        editor.set_language(Language::English);
        assert_eq!(editor.status, "Logo not set: the logo must be a PNG or JPEG image.");
    }

    #[test]
    fn catalogs_agree_on_keys_and_parameters() {
        let parameters = |text: &str| {
            let mut names = text.split('{').skip(1)
                .map(|part| part.split_once('}').expect("closed placeholder").0.to_owned())
                .collect::<Vec<_>>();
            names.sort();
            names
        };
        assert_eq!(EN.len(), ZH.len());
        for (key, english) in EN.iter() {
            assert_eq!(parameters(english), parameters(&ZH[key]), "{key}");
        }
        assert_eq!(Language::Chinese.format("cli.read", &[("path", "网络{error}"), ("error", "EACCES")]), "无法读取 网络{error}：EACCES");
    }

    #[test]
    fn changing_language_changes_generated_captions_and_preserves_document_data() {
        let mut ed = Editor::default();
        ed.language = Language::Chinese;
        ed.new_diagram();
        let id = ed.add_device_of("router", Pt::default());
        assert_eq!(ed.doc.device(id).unwrap().name, "路由器 1");
        ed.add_device_of("自定义设备", Pt::default());
        ed.set_frame("template", "iso");
        ed.set_frame("title", "DRAWING NO. {name}");
        assert!(ed.export_drawio("test").contains("图号"));
        let saved = ed.document_json();
        ed.language = Language::English;
        assert_eq!(ed.document_json(), saved);
        assert!(ed.export_drawio("test").contains("DRAWING NO."));
        ed.language = Language::Chinese;
        ed.load_json(&saved, std::path::Path::new("网络图.stratagram")).unwrap();
        assert_eq!(ed.doc.frame().unwrap().title, "DRAWING NO. {name}");
        assert!(ed.scene_json().contains("图号"));
    }
}
