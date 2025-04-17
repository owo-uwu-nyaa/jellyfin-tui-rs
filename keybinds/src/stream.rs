use std::task::Poll;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::{Stream, StreamExt, stream::FusedStream};
use tracing::warn;

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
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.inner.finished {
            Poll::Ready(None)
        } else {
            Poll::Ready(loop {
                match std::task::ready!(self.inner.events.poll_next_unpin(cx)) {
                    None
                    | Some(Ok(Event::Key(KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: _,
                    }))) => {
                        self.inner.finished = true;
                        break None;
                    }
                    Some(Err(e)) => break Some(Err(e.into())),
                    Some(Ok(Event::Key(KeyEvent {
                        code: KeyCode::Right,
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: _,
                    }))) => {
                        self.current_view = self.current_view.saturating_add(1);
                        break Some(Ok(KeybindEvent::Render));
                    }
                    Some(Ok(Event::Key(KeyEvent {
                        code: KeyCode::Left,
                        modifiers: KeyModifiers::CONTROL,
                        kind: KeyEventKind::Press,
                        state: _,
                    }))) => {
                        self.current_view = self.current_view.saturating_sub(1);
                        break Some(Ok(KeybindEvent::Render));
                    }
                    Some(Ok(Event::Key(KeyEvent {
                        code,
                        modifiers,
                        kind: KeyEventKind::Press,
                        state: _,
                    }))) => {
                        if self.text_input
                            && self.current.is_empty()
                            && modifiers
                                .intersection(KeyModifiers::CONTROL | KeyModifiers::ALT)
                                .is_empty()
                        {
                            if let KeyCode::Char(c) = code {
                                break Some(Ok(KeybindEvent::Text(Text::Char(c))));
                            }
                        }
                        let mut next = Vec::new();
                        let mut ret = None;
                        for c in if_non_empty(self.current.as_ref())
                            .map(|v| either::Right(v.iter()))
                            .unwrap_or_else(|| {
                                either::Left(std::iter::once(&self.top).chain(&self.minor))
                            })
                        {
                            match c.get(&Key {
                                inner: code,
                                control: modifiers.contains(KeyModifiers::CONTROL),
                                alt: modifiers.contains(KeyModifiers::ALT),
                            }) {
                                Some(KeyBinding::Command(c)) => {
                                    ret = Some(KeybindEvent::Command(*c));
                                    break;
                                }
                                Some(KeyBinding::Group { map, name: _ }) => {
                                    next.push(map.clone());
                                }
                                Some(KeyBinding::Invalid(name)) => {
                                    warn!("'{name}' is an invalid command");
                                    if !self.current.is_empty() {
                                        ret = Some(KeybindEvent::Render);
                                    }
                                    next = Vec::new();
                                    break;
                                }
                                None => {}
                            }
                        }
                        if let Some(r) = ret {
                            self.current = Vec::new();
                            break Some(Ok(r));
                        }
                        if next.is_empty() {
                            if !std::mem::take(&mut self.current).is_empty() {
                                break Some(Ok(KeybindEvent::Render));
                            }
                        } else {
                            self.current = next;
                        }
                    }
                    Some(Ok(Event::Paste(text))) => {
                        if self.text_input {
                            break Some(Ok(KeybindEvent::Text(Text::Str(text))));
                        }
                    }
                    Some(Ok(Event::Resize(_, _))) => break Some(Ok(KeybindEvent::Render)),
                    _ => {}
                }
            })
        }
    }
}
