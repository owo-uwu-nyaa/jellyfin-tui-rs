use ratatui::{
    layout::{Constraint, Layout},
    prelude::{Buffer, Rect},
    style::Modifier,
    widgets::{Widget, WidgetRef},
};
use tracing::{instrument, warn};

#[derive(Debug, Default)]
pub struct Checkbox {
    pub active: bool,
    pub set: bool,
}

impl Checkbox {
    pub fn new(active: bool, set: bool) -> Self {
        Self { active, set }
    }

    pub fn set_active(&mut self, active: bool) -> &mut Self {
        self.active = active;
        self
    }
    pub fn set_set(&mut self, set: bool) -> &mut Self {
        self.set = set;
        self
    }

    pub fn render_with(&self, area: Rect, buf: &mut Buffer, other: impl Widget) {
        if area.width < (Self::WIDTH + 2) || area.height < Self::HEIGHT {
            warn!("Not enough space to render Checkbox and other widget")
        } else {
            let [check, inner] =
                Layout::horizontal([Constraint::Length(Self::WIDTH), Constraint::Percentage(100)])
                    .horizontal_margin(1)
                    .areas(area);
            self.render_ref(check, buf);
            other.render(inner, buf);
        }
    }

    pub const WIDTH: u16 = 3;
    pub const HEIGHT: u16 = 1;
}

impl WidgetRef for Checkbox {
    #[instrument(skip_all, name = "render_checkbox")]
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        if area.width < Self::WIDTH || area.height < Self::HEIGHT {
            warn!("Not enough space to render Checkbox")
        } else {
            buf[area].set_char('[');
            buf[(area.x + 2, area.y)].set_char(']');
            let mark = &mut buf[(area.x + 1, area.y)];
            if self.set {
                mark.set_char('X');
            }
            if self.active {
                mark.set_style(Modifier::REVERSED);
            }
        }
    }
}
