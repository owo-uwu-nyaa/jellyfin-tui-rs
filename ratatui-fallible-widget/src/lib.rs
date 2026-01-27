use color_eyre::{Result, eyre::Context};
use ratatui_core::{
    backend::Backend,
    buffer::Buffer,
    layout::Rect,
    terminal::{CompletedFrame, Terminal},
    widgets::Widget,
};

pub trait FallibleWidget {
    fn render_fallible(&mut self, area: Rect, buf: &mut Buffer) -> Result<()>;
}

impl<W> FallibleWidget for W
where
    for<'w> &'w W: Widget,
{
    #[inline(always)]
    fn render_fallible(&mut self, area: Rect, buf: &mut Buffer) -> Result<()> {
        self.render(area, buf);
        Ok(())
    }
}

pub trait TermExt {
    fn draw_fallible(&mut self, widget: &mut impl FallibleWidget) -> Result<CompletedFrame<'_>>;
}

impl<B: Backend> TermExt for Terminal<B>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    fn draw_fallible(&mut self, widget: &mut impl FallibleWidget) -> Result<CompletedFrame<'_>> {
        let mut widget_err: Result<_> = Ok(());
        let frame = self
            .draw(|frame| {
                widget_err = widget.render_fallible(frame.area(), frame.buffer_mut());
                if widget_err.is_err() {
                    frame.buffer_mut().reset();
                }
            })
            .context("drawing frame to terminal")?;
        widget_err?;
        Ok(frame)
    }
}
