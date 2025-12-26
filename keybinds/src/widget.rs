use std::cmp::{max, min};

use itertools::Itertools;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    symbols::{self, border::PLAIN},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};
use ratatui_fallible_widget::FallibleWidget;
use tracing::trace;

use super::{Command, KeybindEventStream};

fn inner_area(
    stream: &KeybindEventStream<'_, impl Command, impl FallibleWidget>,
    area: Rect,
) -> Rect {
    let len: usize = stream.next_maps.iter().map(|v| v.len()).sum();
    if len > 0 {
        let width = (area.width - 4) / 20;
        let full_usable_height = len.div_ceil(width as usize);
        let full_height = full_usable_height + 3;
        let height = min(full_height, max(5, area.height as usize / 4));
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height - height as u16,
        }
    } else {
        area
    }
}

impl<T: Command, W: FallibleWidget> FallibleWidget for KeybindEventStream<'_, T, W> {
    fn render_fallible(&mut self, area: Rect, buf: &mut Buffer) -> color_eyre::eyre::Result<()> {
        self.inner_widget
            .render_fallible(inner_area(self, area), buf)?;
        let len: usize = self.next_maps.iter().map(|v| v.len()).sum();
        if len > 0 {
            let width = (area.width - 4) / 20;
            let full_usable_height = len.div_ceil(width as usize);
            let full_height = full_usable_height + 3;
            let height = min(full_height, max(5, area.height as usize / 4));
            let usable_height = height - 3;
            let num_views = full_usable_height.div_ceil(usable_height);
            self.current_view = min(self.current_view, num_views);
            trace!(
                len,
                width,
                full_usable_height,
                full_height,
                height,
                usable_height,
                num_views,
                "calculated position"
            );
            let area = Rect {
                x: area.x,
                y: area.y + area.height - height as u16,
                width: area.width,
                height: height as u16,
            };
            let block_left = &mut buf[(area.x, area.y + area.height - height as u16 - 1)];
            if block_left.symbol() == symbols::line::NORMAL.bottom_left {
                block_left.set_symbol(symbols::line::NORMAL.vertical_right);
            }
            let block_right = &mut buf[(
                area.x + area.width - 1,
                area.y + area.height - height as u16 - 1,
            )];
            if block_right.symbol() == symbols::line::NORMAL.bottom_right {
                block_right.set_symbol(symbols::line::NORMAL.vertical_left);
            }
            let mut block = Block::new()
                .border_set(PLAIN)
                .borders(Borders::RIGHT | Borders::BOTTOM | Borders::LEFT)
                .padding(ratatui::widgets::Padding {
                    left: 1,
                    right: 1,
                    top: 1,
                    bottom: 1,
                });
            if block_right.symbol() == " " {
                block = block.borders(Borders::all());
            }
            if num_views > 1 {
                block = block
                    .title_bottom(format!("{} of {}", self.current_view, num_views))
                    .title_bottom("switch with Ctrl+left/right");
            }
            let main = block.inner(area);
            block.render(area, buf);
            let items_per_screen = width as usize * usable_height;
            let items = self
                .next_maps
                .iter()
                .map(|v| v.iter())
                .kmerge_by(|(a, _), (b, _)| a < b)
                .skip(items_per_screen * self.current_view)
                .take(items_per_screen);
            let position =
                (0u16..usable_height as u16).flat_map(|y| (0..width).map(move |x| (x, y)));
            for ((key, binding), (x, y)) in items.zip(position) {
                let binding = match binding {
                    super::KeyBinding::Command(c) => Span::styled(c.to_name(), Color::Green),
                    super::KeyBinding::Group { map: _, name } => {
                        Span::styled(name.as_str(), Color::Blue)
                    }
                    super::KeyBinding::Invalid(name) => Span::styled(name.as_str(), Color::Red),
                };
                Paragraph::new(Line::from(vec![
                    Span::raw(key.to_string()),
                    Span::raw(" "),
                    binding,
                ]))
                .render(
                    Rect {
                        x: main.x + x * 20,
                        y: main.y + y,
                        width: 16,
                        height: 1,
                    },
                    buf,
                );
            }
        } else {
            let len = self.help_prefixes.len();
            if len != 0 {
                let mut area = area;
                area.y += area.height - 1;
                area.x += 2;
                area.width = area.width.saturating_sub(2);
                area.height = 1;
                let mut message = "For help press ".to_string();
                for (i, bind) in self.help_prefixes.iter().enumerate() {
                    if i == 0 {
                    } else if i == len - 1 {
                        message.push_str(" or ");
                    } else {
                        message.push_str(", ");
                    }
                    message.push_str(bind);
                }
                message.push('.');
                message.render(area, buf);
            }
        }
        Ok(())
    }
}
