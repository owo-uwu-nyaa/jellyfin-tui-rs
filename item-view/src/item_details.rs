use std::{cmp::min, pin::Pin};

use color_eyre::{Result, eyre::Context};
use entries::{
    entry::{ENTRY_WIDTH, Entry, entry_height},
    image::available::ImagesAvailable,
};
use fetch::{fetch_child_of_type, fetch_screen};
use futures_util::StreamExt;
use jellyfin::items::MediaItem;
use jellyfin_tui_core::{
    context::TuiContext,
    keybinds::ItemDetailsCommand,
    state::{Navigation, NextScreen, ToNavigation},
};
use keybinds::{KeybindEvent, KeybindEventStream};
use ratatui::{
    layout::{Constraint, Layout, Margin},
    text::Text,
    widgets::{
        Block, Padding, Paragraph, Scrollbar, ScrollbarState, StatefulWidget, Widget,
    },
};
use ratatui_fallible_widget::{FallibleWidget, TermExt};

pub async fn display_fetch_item(cx: Pin<&mut TuiContext>, parent: &str) -> Result<Navigation> {
    let cx = cx.project();
    let jellyfin = cx.jellyfin;
    fetch_screen(
        "fetching episode",
        async {
            Ok(fetch_child_of_type(jellyfin, "Episode, Movie", parent)
                .await
                .context("fetching episode")
                .map(|item| Navigation::Replace(NextScreen::ItemDetails(item)))
                .to_nav())
        },
        cx.events,
        cx.config.keybinds.fetch.clone(),
        cx.term,
    )
    .await
}

struct ItemDisplay<'s> {
    entry: &'s mut Entry,
    height: u16,
    width: Option<u16>,
    scrollbar_state: ScrollbarState,
    scrollbar_pos: u16,
    scrollbar_len: u16,
    item: &'s MediaItem,
}

impl FallibleWidget for ItemDisplay<'_> {
    fn render_fallible(
        &mut self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) -> Result<()> {
        let block = Block::bordered()
            .title(self.item.name.as_str())
            .padding(ratatui::widgets::Padding::uniform(1));
        let main = block.inner(area);
        let [entry_area, descripton_area] =
            Layout::vertical([Constraint::Length(self.height), Constraint::Min(1)])
                .spacing(1)
                .areas(main);
        let [entry_area] = Layout::horizontal([Constraint::Length(ENTRY_WIDTH)]).areas(entry_area);
        self.entry.render_fallible(entry_area, buf)?;
        let w = descripton_area.width.saturating_sub(4);
        if self.width != Some(w) {
            self.width = Some(w);
            if let Some(d) = &self.item.overview {
                let lines = textwrap::wrap(d, w as usize);
                self.scrollbar_state = self.scrollbar_state.content_length(lines.len());
                self.scrollbar_len = lines.len() as u16;
                self.scrollbar_pos = min(self.scrollbar_pos, self.scrollbar_len - 1);
                Paragraph::new(Text::from_iter(lines))
                    .block(
                        Block::bordered()
                            .title("Overview")
                            .padding(Padding::uniform(1)),
                    )
                    .scroll((self.scrollbar_pos, 0))
                    .render(descripton_area, buf);
                Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight).render(
                    descripton_area.inner(Margin {
                        horizontal: 0,
                        vertical: 2,
                    }),
                    buf,
                    &mut self.scrollbar_state,
                );
            }
        }
        Ok(())
    }
}

//also works with movies
pub async fn display_item(cx: Pin<&mut TuiContext>, item: MediaItem) -> Result<Navigation> {
    let images_available = ImagesAvailable::new();
    let mut entry = Entry::from_media_item(
        item.clone(),
        &cx.jellyfin,
        &cx.cache,
        &cx.image_cache,
        &images_available,
        &cx.image_picker,
    )?;
    let mut widget = ItemDisplay {
        entry: &mut entry,
        height: entry_height(cx.image_picker.font_size()),
        width: None,
        scrollbar_state: ScrollbarState::new(0),
        scrollbar_pos: 0,
        scrollbar_len: 0,
        item: &item,
    };
    let cx = cx.project();
    let mut events = KeybindEventStream::new(
        cx.events,
        &mut widget,
        cx.config.keybinds.item_details.clone(),
    );
    loop {
        cx.term.draw_fallible(&mut events)?;
        let cmd = tokio::select! {
            _ = images_available.wait_available() => {continue          }
            term = events.next() => {
                match term {
                    Some(Ok(KeybindEvent::Command(cmd))) => cmd,
                    Some(Ok(KeybindEvent::Render)) => continue ,
                    Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
                    Some(Err(e)) => break  Err(e).context("getting key events from terminal"),
                    None => break  Ok(Navigation::PopContext)
                }
            }
        };
        match cmd {
            ItemDetailsCommand::Quit => break Ok(Navigation::PopContext),
            ItemDetailsCommand::Up => {
                events.get_inner().scrollbar_pos = min(
                    events.get_inner().scrollbar_pos + 1,
                    events.get_inner().scrollbar_len - 1,
                );
            }
            ItemDetailsCommand::Down => {
                events.get_inner().scrollbar_pos =
                    events.get_inner().scrollbar_pos.saturating_sub(1);
            }
            ItemDetailsCommand::Reload => {
                break Ok(Navigation::Replace(NextScreen::FetchItemDetails(item.id)));
            }
            ItemDetailsCommand::Play => {
                let next = NextScreen::LoadPlayItem(jellyfin_tui_core::entries::play(&item));
                break Ok(Navigation::Push {
                    current: NextScreen::ItemDetails(item),
                    next,
                });
            }
        }
    }
}
