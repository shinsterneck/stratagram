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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{editor::Editor, model::Pt};

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
