use std::{collections::HashMap, sync::Arc};

use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use crossterm::event::KeyCode;
use serde::Deserialize;

use crate::keybinds::BindingMap;

use super::{Command, KeyBinding};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ParseKeybinding {
    Command(String),
    Group {
        map: HashMap<String, ParseKeybinding>,
        name: String,
    },
    Template {
        template: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct Config {
    template: Option<HashMap<String, ParseKeybinding>>,
    #[serde(flatten)]
    maps: HashMap<String, HashMap<String, ParseKeybinding>>,
}

impl Config {
    pub fn parse<T: Command>(&self, name: &str, strict: bool) -> Result<BindingMap<T>> {
        let empty_template = HashMap::with_capacity(0);
        let template = self.template.as_ref().unwrap_or(&empty_template);
        let map: Result<HashMap<KeyCode, KeyBinding<T>>> = self
            .maps
            .get(name)
            .ok_or_else(|| eyre!("missing map '{name}'"))?
            .iter()
            .map(|(key_name, binding)| -> Result<(KeyCode, KeyBinding<T>)> {
                let key = parse_key_code(key_name.as_str())
                    .ok_or_else(|| eyre!("key code '{key_name}' is invalid"))?;
                let binding = do_parse_binding(template, &Seen::Empty, binding, strict)
                    .with_context(|| format!("key '{key_name}'"))?;
                Ok((key, binding))
            })
            .collect();
        Ok(Arc::new(map?))
    }
}

fn parse_key_code(name: &str) -> Option<KeyCode> {
    match name {
        "backspace" => KeyCode::Backspace.into(),
        "enter" => KeyCode::Enter.into(),
        "left" => KeyCode::Left.into(),
        "right" => KeyCode::Right.into(),
        "up" => KeyCode::Up.into(),
        "down" => KeyCode::Down.into(),
        "tab" => KeyCode::Tab.into(),
        "back-tab" => KeyCode::BackTab.into(),
        "delete" => KeyCode::Delete.into(),
        "insert" => KeyCode::Insert.into(),
        "esc" => KeyCode::Esc.into(),
        code => {
            let mut chars = code.chars();
            if let Some(first) = chars.next() {
                if chars.next().is_none() {
                    KeyCode::Char(first).into()
                } else if first == 'f' {
                    if let Ok(n) = code[1..].parse() {
                        KeyCode::F(n).into()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}

enum Seen<'s> {
    Empty,
    Item { name: &'s str, next: &'s Seen<'s> },
}

fn do_parse_binding<T: Command>(
    templates: &HashMap<String, ParseKeybinding>,
    seen: &Seen,
    binding: &ParseKeybinding,
    strict: bool,
) -> Result<KeyBinding<T>> {
    match binding {
        ParseKeybinding::Command(name) => {
            if let Some(cmd) = T::from_name(name) {
                Ok(KeyBinding::Command(cmd))
            } else if strict {
                Err(eyre!("unknown command {name}"))
            } else {
                Ok(KeyBinding::Invalid(name.to_string()))
            }
        }
        ParseKeybinding::Group { map, name } => {
            let map: Result<HashMap<KeyCode, KeyBinding<T>>> = map
                .iter()
                .map(|(key_name, binding)| -> Result<(KeyCode, KeyBinding<T>)> {
                    let key = parse_key_code(key_name.as_str())
                        .ok_or_else(|| eyre!("key code '{key_name}' is invalid"))?;
                    let binding = do_parse_binding(templates, seen, binding, strict)
                        .with_context(|| format!("key '{key_name}'"))?;
                    Ok((key, binding))
                })
                .collect();
            Ok(KeyBinding::Group {
                map: Arc::new(map?),
                name: name.to_owned(),
            })
        }
        ParseKeybinding::Template { template } => {
            let replace = templates
                .get(template)
                .ok_or_else(|| eyre!("unknown template '{template}'"))?;
            {
                let mut current = seen;
                while let Seen::Item { name, next } = current {
                    if name == template {
                        return Err(eyre!("infinite recursion in template {name}"));
                    }
                    current = next;
                }
            }
            let seen = Seen::Item {
                name: template,
                next: seen,
            };
            do_parse_binding(templates, &seen, replace, strict)
                .with_context(|| format!("in template '{template}'"))
        }
    }
}

