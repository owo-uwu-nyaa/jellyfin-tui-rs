use std::{
    collections::{BTreeMap, HashMap},
    ops::Deref,
    sync::Arc,
};

use color_eyre::eyre::{Context, Result, eyre};
use crossterm::event::KeyCode;
use serde::Deserialize;

use super::{BindingMap, Command, Key, KeyBinding};

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ParseKeybinding {
    Command(String),
    Group {
        name: String,
        #[serde(flatten)]
        map: ParseKeybindingsMap,
    },
}

#[derive(Debug, Deserialize)]
pub struct ParseKeybindingsMap {
    #[serde(default)]
    pub template: Vec<String>,
    #[serde(flatten)]
    pub map: HashMap<String, ParseKeybinding>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub help_prefixes: Vec<String>,
    pub template: Option<HashMap<String, ParseKeybindingsMap>>,
    #[serde(flatten)]
    pub maps: HashMap<String, ParseKeybindingsMap>,
}

impl Config {
    pub fn try_parse<T: Command>(&self, name: &str, strict: bool) -> Result<Option<BindingMap<T>>> {
        let empty_template = HashMap::new();
        let template = self.template.as_ref().unwrap_or(&empty_template);
        let map = if let Some(map) = self.maps.get(name) {
            map
        } else {
            return Ok(None);
        };
        let mapping = parse_mapping(strict, template, map, &Seen::Empty)?;
        let mut map: BTreeMap<Key, KeyBinding<_>> = mapping.deref().clone();
        for prefix in &self.help_prefixes {
            let key =
                parse_key_code(prefix).ok_or_else(|| eyre!("key code '{prefix}' is invalid"))?;
            map.insert(
                key,
                KeyBinding::Group {
                    map: mapping.clone(),
                    name: "help-prefix".to_string(),
                },
            );
        }
        Ok(Some(Arc::new(map)))
    }
    pub fn parse<T: Command>(&self, name: &str, strict: bool) -> Result<BindingMap<T>> {
        self.try_parse(name, strict)
            .transpose()
            .ok_or_else(|| eyre!("missing map '{name}'"))?
    }
}

fn parse_mapping<T: Command>(
    strict: bool,
    template: &HashMap<String, ParseKeybindingsMap>,
    map: &ParseKeybindingsMap,
    seen: &Seen,
) -> Result<BindingMap<T>> {
    let mut res = BTreeMap::new();
    insert_each_binding_map(template, seen, map, strict, &mut res)?;
    Ok(Arc::new(res))
}

fn insert_each_binding_map<T: Command>(
    templates: &HashMap<String, ParseKeybindingsMap>,
    seen: &Seen,
    current: &ParseKeybindingsMap,
    strict: bool,
    into: &mut BTreeMap<Key, KeyBinding<T>>,
) -> Result<()> {
    for name in &current.template {
        seen.seen(name)?;
        let template = templates
            .get(name)
            .ok_or_else(|| eyre!("unknown template {name}"))?;
        let seen = Seen::Item { name, next: seen };
        insert_each_binding_map(templates, &seen, template, strict, into)
            .with_context(|| format!("in template {name}"))?;
    }
    for (key, binding) in &current.map {
        let (key, binding) = parse_mapping_item(key, binding, templates, strict, seen)?;
        into.insert(key, binding);
    }

    Ok(())
}

fn parse_mapping_item<T: Command>(
    key_name: &str,
    binding: &ParseKeybinding,
    template: &HashMap<String, ParseKeybindingsMap>,
    strict: bool,
    seen: &Seen,
) -> Result<(Key, KeyBinding<T>)> {
    let key = parse_key_code(key_name).ok_or_else(|| eyre!("key code '{key_name}' is invalid"))?;
    let binding = do_parse_binding(template, seen, binding, strict)
        .with_context(|| format!("key '{key_name}'"))?;
    Ok((key, binding))
}

fn parse_key_code(mut name: &str) -> Option<Key> {
    let mut control = false;
    let mut alt = false;

    while let Some(b'-') = name.as_bytes().get(1) {
        match name.as_bytes()[0] {
            b'C' => {
                if control {
                    return None;
                } else {
                    control = true;
                }
            }
            b'A' => {
                if alt {
                    return None;
                } else {
                    alt = true;
                }
            }
            _ => return None,
        }
        name = &name[2..];
    }

    let key = match name {
        "backspace" => KeyCode::Backspace,
        "space" => KeyCode::Char(' '),
        "enter" => KeyCode::Enter,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "tab" => KeyCode::Tab,
        "back-tab" => KeyCode::BackTab,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "esc" => KeyCode::Esc,
        code => {
            let mut chars = code.chars();
            if let Some(first) = chars.next() {
                if chars.next().is_none() {
                    KeyCode::Char(first)
                } else if first == 'f' {
                    if let Ok(n) = code[1..].parse() {
                        KeyCode::F(n)
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    };
    Some(Key {
        inner: key,
        control,
        alt,
    })
}

enum Seen<'s> {
    Empty,
    Item { name: &'s str, next: &'s Seen<'s> },
}

impl Seen<'_> {
    fn seen(&self, template: &str) -> Result<()> {
        let mut current = self;
        while let Seen::Item { name, next } = current {
            if *name == template {
                return Err(eyre!("infinite recursion in template {name}"));
            }
            current = next;
        }
        Ok(())
    }
}

fn do_parse_binding<T: Command>(
    templates: &HashMap<String, ParseKeybindingsMap>,
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
        ParseKeybinding::Group { map, name } => Ok(KeyBinding::Group {
            map: parse_mapping(strict, templates, map, seen)?,
            name: name.to_owned(),
        }),
    }
}
