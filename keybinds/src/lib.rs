pub mod parse_config;
pub mod stream;
pub mod widget;

use crossterm::event::{EventStream, KeyCode};
use eyre::Result;
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display},
    sync::Arc,
};

///reexport for proc macro
#[doc(hidden)]
pub use eyre as __eyre;

pub use keybinds_derive::{Command, keybind_config};

pub trait Command: Clone + Copy + Debug {
    fn to_name(self) -> &'static str;
    fn from_name(name: &str) -> Option<Self>;
    fn all() -> &'static [&'static str];
}

#[derive(PartialEq, Eq, Clone)]
pub struct Key {
    pub inner: KeyCode,
    pub control: bool,
    pub alt: bool,
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn score_key(k: &Key) -> u8 {
    let mut v = 0;
    if k.control {
        v += 2;
    }
    if k.alt {
        v += 1;
    }
    v
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.inner
            .to_string()
            .cmp(&other.inner.to_string())
            .then_with(|| score_key(self).cmp(&score_key(other)))
    }
}

impl Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self, f)
    }
}
impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.control {
            f.write_str("C-")?;
        }
        if self.alt {
            f.write_str("A")?;
        }
        Display::fmt(&self.inner, f)
    }
}

pub type BindingMap<T> = Arc<BTreeMap<Key, KeyBinding<T>>>;

#[derive(Debug, Clone)]
pub enum KeyBinding<T: Command> {
    Command(T),
    Group { map: BindingMap<T>, name: String },
    Invalid(String),
}

pub enum Text {
    Char(char),
    Str(String),
}

pub enum KeybindEvent<T: Command> {
    Render,
    Command(T),
    Text(Text),
}

pub struct KeybindEvents {
    events: EventStream,
    finished: bool,
}

impl KeybindEvents {
    pub fn new() -> Result<Self> {
        Ok(Self {
            events: EventStream::new(),
            finished: false,
        })
    }
}

pub struct KeybindEventStream<'e, T: Command> {
    inner: &'e mut KeybindEvents,
    top: BindingMap<T>,
    current: Vec<BindingMap<T>>,
    text_input: bool,
    current_view: usize,
    minor: Vec<BindingMap<T>>,
}

impl<'e, T: Command> KeybindEventStream<'e, T> {
    pub fn new(events: &'e mut KeybindEvents, map: BindingMap<T>) -> Self {
        Self {
            inner: events,
            top: map,
            current: Vec::with_capacity(0),
            text_input: false,
            current_view: 0,
            minor: Vec::with_capacity(0),
        }
    }

    pub fn new_with_minor(
        events: &'e mut KeybindEvents,
        map: BindingMap<T>,
        minor: Vec<BindingMap<T>>,
    ) -> Self {
        Self {
            inner: events,
            top: map,
            current: Vec::with_capacity(0),
            text_input: false,
            current_view: 0,
            minor,
        }
    }

    pub fn set_text_input(&mut self, text_input: bool) {
        self.text_input = text_input;
    }

    pub fn get_minor(&self) -> &Vec<BindingMap<T>> {
        &self.minor
    }

    pub fn get_minor_mut(&mut self) -> &mut Vec<BindingMap<T>> {
        &mut self.minor
    }
}

#[doc(hidden)]
pub mod __macro_support {
    use crate::{BindingMap, Command};

    pub fn collect_all_names(names: &[&[&'static str]]) -> &'static [&'static str] {
        let mut out: Vec<_> = names
            .iter()
            .flat_map(|s| s.iter().map(|s| s as &'static str))
            .collect();
        out.sort_unstable();
        out.leak()
    }
    pub fn commands_unique(names: &[&str], ty:&str) {
        let mut iter = names.iter();
        if let Some(mut last) = iter.next() {
            for current in iter {
                assert_ne!(last,current, "name of commands in {ty} is not unique");
                last = current;
            }
        }
    }

    pub trait BindingMapExt {
        type T;
    }
    impl<T:Command> BindingMapExt for BindingMap<T>{
        type T = T;
    }
}
