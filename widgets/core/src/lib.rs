pub mod async_task;

use std::any::type_name;

pub use color_eyre::Result;
use color_eyre::eyre::ensure;
pub use config::Config;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{KeyModifiers, MouseEventKind},
    layout::{Position, Rect, Size},
    widgets::{Paragraph, Widget, Wrap},
};
pub use ratatui_image::FontSize;
use tracing::warn;

use crate::async_task::TaskSubmitter;

pub struct DimensionsParameter<'c> {
    pub config: &'c Config,
    pub font_size: FontSize,
}

pub trait Wrapper<C>: Clone + Copy + Send + Sync + 'static {
    type F: Send + 'static;
    fn wrap(&self, val: C) -> Self::F;
}

pub trait JellyhajWidget {
    type State;
    type Action: Send + 'static;
    type ActionResult;
    fn min_width(&self) -> Option<u16> {
        None
    }
    fn min_height(&self) -> Option<u16> {
        None
    }

    fn min_width_static(_par: DimensionsParameter<'_>) -> Option<u16> {
        None
    }
    fn min_height_static(_par: DimensionsParameter<'_>) -> Option<u16> {
        None
    }

    fn into_state(self) -> Self::State;

    fn apply_action(&mut self, action: Self::Action) -> Result<Option<Self::ActionResult>>;
    fn click(
        &mut self,
        position: Position,
        size: Size,
        kind: MouseEventKind,
        modifier: KeyModifiers,
    ) -> Result<Option<Self::ActionResult>>;

    fn render_fallible(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        task: TaskSubmitter<Self::Action, impl Wrapper<Self::Action>>,
    ) -> Result<()> {
        fn size_ok(
            widget: &'static str,
            min_width: Option<u16>,
            min_height: Option<u16>,
            area: Rect,
            buf: &mut Buffer,
        ) -> bool {
            if let Some(min_width) = min_width
                && min_width < area.width
            {
                let message = if let Some(min_height) = min_height
                    && min_height < area.height
                {
                    format!("Terminal not large enough. Requires at least {min_width}x{min_height}")
                } else {
                    format!("Terminal not large enough. Requires a width of at least {min_width}")
                };
                warn!(widget, "{message}");
                Paragraph::new(message)
                    .wrap(Wrap { trim: true })
                    .render(area, buf);
                false
            } else if let Some(min_height) = min_height
                && min_height < area.height
            {
                let message = format!(
                    "Terminal not large enough. Requires a height of at least {min_height}"
                );
                warn!(widget, "{message}");
                Paragraph::new(message)
                    .wrap(Wrap { trim: true })
                    .render(area, buf);
                false
            } else {
                true
            }
        }
        let min_width = self.min_width();
        let min_height = self.min_height();
        if (min_width.is_none() && min_height.is_none())
            || size_ok(type_name::<Self>(), min_width, min_height, area, buf)
        {
            self.render_fallible_inner(area, buf, task)
        } else {
            Ok(())
        }
    }

    fn render_fallible_inner(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        task: TaskSubmitter<Self::Action, impl Wrapper<Self::Action>>,
    ) -> Result<()>;
}

pub trait ItemWidget {
    type State;
    type Action: Send + 'static;
    type ActionResult;

    fn dimensions(&self) -> Size;
    fn dimensions_static(par: DimensionsParameter<'_>) -> Size;

    fn into_state(self) -> Self::State;

    fn set_active(&mut self, active: bool);

    fn apply_action(&mut self, action: Self::Action) -> Result<Option<Self::ActionResult>>;
    fn click(
        &mut self,
        position: Position,
        size: Size,
        kind: MouseEventKind,
        modifier: KeyModifiers,
    ) -> Result<Option<Self::ActionResult>>;

    fn render_item(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        task: TaskSubmitter<Self::Action, impl Wrapper<Self::Action>>,
    ) -> Result<()>;
}

impl<I: ItemWidget> JellyhajWidget for I {
    #[inline(always)]
    fn min_width(&self) -> Option<u16> {
        Some(self.dimensions().width)
    }

    #[inline(always)]
    fn min_height(&self) -> Option<u16> {
        Some(self.dimensions().height)
    }

    #[inline(always)]
    fn min_width_static(par: DimensionsParameter<'_>) -> Option<u16> {
        Some(Self::dimensions_static(par).width)
    }

    #[inline(always)]
    fn min_height_static(par: DimensionsParameter<'_>) -> Option<u16> {
        Some(Self::dimensions_static(par).height)
    }

    type State = <I as ItemWidget>::State;
    type Action = <I as ItemWidget>::Action;
    type ActionResult = <I as ItemWidget>::ActionResult;

    #[inline(always)]
    fn into_state(self) -> Self::State {
        ItemWidget::into_state(self)
    }

    fn render_fallible_inner(
        &mut self,
        mut area: Rect,
        buf: &mut Buffer,
        task: TaskSubmitter<Self::Action, impl Wrapper<Self::Action>>,
    ) -> Result<()> {
        let dim = self.dimensions();
        assert!(dim.width <= area.width, "width is too small");
        assert!(dim.height <= area.height, "height is too small");
        area = area.resize(dim);
        ensure!(dim.width == area.width, "width is too large for position");
        ensure!(dim.height == area.height, "width is too large for position");
        Self::render_item(self, area, buf, task)
    }

    fn apply_action(&mut self, action: Self::Action) -> Result<Option<Self::ActionResult>> {
        ItemWidget::apply_action(self, action)
    }
    fn click(
        &mut self,
        position: Position,
        size: Size,
        kind: MouseEventKind,
        modifier: KeyModifiers,
    ) -> Result<Option<Self::ActionResult>> {
        ItemWidget::click(self, position, size, kind, modifier)
    }
}
