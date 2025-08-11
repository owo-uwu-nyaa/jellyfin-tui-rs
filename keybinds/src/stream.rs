use std::task::Poll;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::{Stream, StreamExt, stream::FusedStream};
use tracing::{debug, warn};

use crate::Key;

use super::{Command, KeyBinding, KeybindEvent, KeybindEventStream, Text};
use eyre::Result;

impl<T: Command> FusedStream for KeybindEventStream<'_, T> {
    fn is_terminated(&self) -> bool {
        self.inner.finished
    }
}

fn if_non_empty<T>(v: &Vec<T>) -> Option<&Vec<T>> {
    if v.is_empty() { None } else { Some(v) }
}

impl<T: Command> Stream for KeybindEventStream<'_, T> {
    type Item = Result<KeybindEvent<T>>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let e = this.span.enter();
        if this.inner.finished {
            Poll::Ready(None)
        } else {
            let event = 'outer: loop {
                let event = std::task::ready!(this.inner.events.poll_next_unpin(cx));
                debug!(?event, "received event from terminal");
                match event {
                    None
                    | Some(Ok(Event::Key(KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: _,
                    }))) => {
                        debug!("finished");
                        this.inner.finished = true;
                        break None;
                    }
                    Some(Err(e)) => break Some(Err(e.into())),
                    Some(Ok(Event::Key(KeyEvent {
                        code: KeyCode::Right,
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: _,
                    }))) => {
                        debug!("moving keybind help page");
                        this.current_view = this.current_view.saturating_add(1);
                        break Some(Ok(KeybindEvent::Render));
                    }
                    Some(Ok(Event::Key(KeyEvent {
                        code: KeyCode::Left,
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: _,
                    }))) => {
                        debug!("moving keybind help page");
                        this.current_view = this.current_view.saturating_sub(1);
                        break Some(Ok(KeybindEvent::Render));
                    }
                    Some(Ok(Event::Key(KeyEvent {
                        code,
                        modifiers,
                        kind: KeyEventKind::Press,
                        state: _,
                    }))) => {
                        if this.text_input
                            && let KeyCode::Char(c) = code
                            && this.next_maps.is_empty()
                            && modifiers
                                .intersection(KeyModifiers::CONTROL | KeyModifiers::ALT)
                                .is_empty()
                        {
                            debug!("keyboard press in text field");
                            break Some(Ok(KeybindEvent::Text(Text::Char(c))));
                        }
                        let current_map = std::mem::take(&mut this.next_maps);
                        let (top, minor) = (&this.top, &this.minor);
                        debug!(?current_map, "matching on active keymaps");

                        for c in if_non_empty(current_map.as_ref())
                            .map(|v| either::Right(v.iter()))
                            .unwrap_or_else(|| either::Left(std::iter::once(top).chain(minor)))
                        {
                            match c.get(&Key {
                                inner: code,
                                control: modifiers.contains(KeyModifiers::CONTROL),
                                alt: modifiers.contains(KeyModifiers::ALT),
                            }) {
                                Some(KeyBinding::Command(c)) => {
                                    debug!("found matching command");
                                    this.next_maps = Vec::new();
                                    break 'outer Some(Ok(KeybindEvent::Command(*c)));
                                }
                                Some(KeyBinding::Group { map, name }) => {
                                    debug!(name, "found matching group");
                                    this.next_maps.push(map.clone());
                                }
                                Some(KeyBinding::Invalid(name)) => {
                                    warn!("'{name}' is an invalid command");
                                    if !current_map.is_empty() {
                                        this.next_maps = Vec::new();
                                        break 'outer Some(Ok(KeybindEvent::Render));
                                    }
                                    break;
                                }
                                None => {}
                            }
                        }
                        if !(current_map.is_empty() && this.next_maps.is_empty()) {
                            debug!("should render");
                            break Some(Ok(KeybindEvent::Render));
                        }
                    }
                    Some(Ok(Event::Paste(text))) => {
                        if this.text_input {
                            debug!("paste while text input active");
                            break Some(Ok(KeybindEvent::Text(Text::Str(text))));
                        } else {
                            debug!("currently no active text input");
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => break Some(Ok(KeybindEvent::Render)),
                    _ => {}
                }
            };
            debug!(?event, "emitting event");
            drop(e);
            Poll::Ready(event)
        }
    }
}
