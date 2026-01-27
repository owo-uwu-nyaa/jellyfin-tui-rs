pub mod grid;
pub mod list;
pub mod screen;

use config::Config;
use ratatui::layout::Size;
use ratatui_fallible_widget::FallibleWidget;
use ratatui_image::FontSize;

pub trait Item: FallibleWidget {
    fn set_active(&mut self, active: bool);
    fn dimension(config: &Config, font_size: FontSize) -> Size;
}
