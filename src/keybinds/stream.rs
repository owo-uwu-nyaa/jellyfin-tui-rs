use std::task::Poll;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::{Stream, StreamExt, stream::FusedStream};
use tracing::warn;

use super::{Command, KeyBinding, KeybindEvent, KeybindEventStream, Text};
use color_eyre::Result;

impl<T: Command> FusedStream for KeybindEventStream<'_, T> {
    fn is_terminated(&self) -> bool {
        self.inner.finished
    }
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
                        modifiers: _,
                        kind: KeyEventKind::Press,
                        state: _,
                    }))) => {
                        if self.text_input && self.current.is_none() {
                            if let KeyCode::Char(c) = code {
                                break Some(Ok(KeybindEvent::Text(Text::Char(c))));
                            }
                        }
                        if self.current.is_none() && matches!(code, KeyCode::Char('?')) {
                            self.current = self.top.clone().into();
                            break Some(Ok(KeybindEvent::Render));
                        }
                        let current = self.current.as_ref().unwrap_or(&self.top).clone();
                        match current.get(&code.into()) {
                            Some(KeyBinding::Command(c)) => {
                                self.current = None;
                                break Some(Ok(KeybindEvent::Command(*c)));
                            }
                            Some(KeyBinding::Group { map, name: _ }) => {
                                self.current = Some(map.clone());
                                break Some(Ok(KeybindEvent::Render));
                            }
                            Some(KeyBinding::Invalid(name)) => {
                                warn!("'{name}' is an invalid command");
                                if self.current.take().is_some() {
                                    break Some(Ok(KeybindEvent::Render));
                                }
                            }
                            None => {
                                if self.current.take().is_some() {
                                    break Some(Ok(KeybindEvent::Render));
                                }
                            }
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
