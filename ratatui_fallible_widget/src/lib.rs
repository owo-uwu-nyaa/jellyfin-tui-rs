use ratatui::{CompletedFrame, Terminal, buffer::Buffer, layout::Rect, prelude::Backend, widgets::WidgetRef};
use color_eyre::{Result, eyre::Context};

pub trait FallibleWidget {
    fn render_fallible(&mut self, area: Rect, buf: &mut Buffer) -> Result<()>;
}

impl<W:WidgetRef> FallibleWidget for W{
    #[inline(always)]
    fn render_fallible(&mut self, area: Rect, buf: &mut Buffer) -> Result<()> {
        self.render_ref(area, buf);
        Ok(())
    }
}

pub trait TermExt {
    fn draw_fallible(&mut self, widget: &mut impl FallibleWidget) -> Result<CompletedFrame<'_>>;
}

impl<B: Backend> TermExt for Terminal<B> {
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
