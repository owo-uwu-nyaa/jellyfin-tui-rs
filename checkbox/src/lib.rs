use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};
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
            let mut check = area;
            check.width = 3;
            let mut inner = area;
            inner.width -= 3;
            inner.x += 3;
            self.render(check, buf);
            other.render(inner, buf);
        }
    }

    pub const WIDTH: u16 = 3;
    pub const HEIGHT: u16 = 1;
}

impl Widget for &Checkbox {
    #[instrument(skip_all, name = "render_checkbox")]
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < Checkbox::WIDTH || area.height < Checkbox::HEIGHT {
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
