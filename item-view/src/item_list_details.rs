use std::{cmp::min, pin::Pin};

use color_eyre::{Result, eyre::Context};
use entries::{
    entry::Entry,
    image::available::ImagesAvailable,
    list::{EntryList, entry_list_height},
};
use fetch::{fetch_all_children, fetch_child_of_type, fetch_item, fetch_screen};
use futures_util::{StreamExt, future::try_join};
use jellyfin::items::MediaItem;
use jellyfin_tui_core::{
    context::TuiContext,
    entries::EntryExt,
    keybinds::ItemListDetailsCommand,
    state::{Navigation, NextScreen, ToNavigation},
};
use keybinds::{KeybindEvent, KeybindEventStream};
use ratatui::{
    layout::{Constraint, Layout, Margin},
    text::Text,
    widgets::{Block, Padding, Paragraph, Scrollbar, ScrollbarState, StatefulWidget, Widget},
};
use ratatui_fallible_widget::{FallibleWidget, TermExt};

pub async fn display_fetch_item_list(
    cx: Pin<&mut TuiContext>,
    item: MediaItem,
) -> Result<Navigation> {
    let cx = cx.project();
    let jellyfin = cx.jellyfin;
    fetch_screen(
        &format!("Loading {}", &item.name),
        async move {
            Ok(fetch_all_children(jellyfin, &item.id)
                .await
                .map(move |data| Navigation::Replace(NextScreen::ItemListDetailsData(item, data)))
                .to_nav())
        },
        cx.events,
        cx.config.keybinds.fetch.clone(),
        cx.term,
    )
    .await
}

pub async fn display_fetch_item_list_ref(
    cx: Pin<&mut TuiContext>,
    item: &str,
) -> Result<Navigation> {
    let cx = cx.project();
    let jellyfin = cx.jellyfin;
    fetch_screen(
        "Loading item list",
        async {
            Ok(try_join(
                fetch_all_children(jellyfin, item),
                fetch_item(jellyfin, item),
            )
            .await
            .map(|(data, item)| Navigation::Replace(NextScreen::ItemListDetailsData(item, data)))
            .to_nav())
        },
        cx.events,
        cx.config.keybinds.fetch.clone(),
        cx.term,
    )
    .await
}

pub async fn display_fetch_season(cx: Pin<&mut TuiContext>, series: &str) -> Result<Navigation> {
    let cx = cx.project();
    let jellyfin = cx.jellyfin;
    fetch_screen(
        "Loading item list",
        async {
            Ok(fetch_child_of_type(jellyfin, "Season", series)
                .await
                .map(|item| Navigation::Replace(NextScreen::FetchItemListDetails(item)))
                .to_nav())
        },
        cx.events,
        cx.config.keybinds.fetch.clone(),
        cx.term,
    )
    .await
}

pub fn handle_item_list_details_data(
    cx: Pin<&mut TuiContext>,
    item: MediaItem,
    childs: Vec<MediaItem>,
) -> Result<Navigation> {
    let name = item.name.clone();
    let images_available = ImagesAvailable::new();
    Ok(Navigation::Replace(NextScreen::ItemListDetails(
        item,
        EntryList::new(
            childs
                .iter()
                .map(|item| {
                    Entry::from_media_item(
                        item.clone(),
                        &cx.jellyfin,
                        &cx.cache,
                        &cx.image_cache,
                        &images_available,
                        &cx.image_picker,
                        &cx.stats
                    )
                })
                .collect::<Result<Vec<_>>>()?,
            name,
        ),
        images_available,
    )))
}

struct ItemListDetails<'s> {
    height: u16,
    width: Option<u16>,
    scrollbar_state: ScrollbarState,
    scrollbar_pos: u16,
    scrollbar_len: u16,
    entries: &'s mut EntryList,
    item: &'s MediaItem,
    block: Block<'static>,
}

impl FallibleWidget for ItemListDetails<'_> {
    fn render_fallible(
        &mut self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
    ) -> Result<()> {
        let main = self.block.inner(area);
        let [entry_area, descripton_area] =
            Layout::vertical([Constraint::Length(self.height), Constraint::Min(1)])
                .spacing(1)
                .areas(main);
        self.entries.render_fallible(entry_area, buf)?;
        let w = descripton_area.width.saturating_sub(4);
        if self.width != Some(w) {
            self.width = Some(w);
            if let Some(d) = &self.item.overview {
                let lines = textwrap::wrap(d, w as usize);
                self.scrollbar_state = self.scrollbar_state.content_length(lines.len());
                self.scrollbar_len = lines.len() as u16;
                self.scrollbar_pos = min(self.scrollbar_pos, self.scrollbar_len.saturating_sub(1));
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

pub async fn display_item_list_details(
    cx: Pin<&mut TuiContext>,
    item: MediaItem,
    mut entries: EntryList,
    images_available: ImagesAvailable,
) -> Result<Navigation> {
    let cx = cx.project();
    entries.active = true;
    let mut details = ItemListDetails {
        height: entry_list_height(cx.image_picker.font_size()),
        width: None,
        scrollbar_state: ScrollbarState::new(0),
        scrollbar_pos: 0,
        scrollbar_len: 0,
        entries: &mut entries,
        item: &item,
        block: Block::bordered().padding(ratatui::widgets::Padding::uniform(1)),
    };
    let mut events = KeybindEventStream::new(
        cx.events,
        &mut details,
        cx.config.keybinds.item_list_details.clone(),
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
            ItemListDetailsCommand::Quit => break Ok(Navigation::PopContext),
            ItemListDetailsCommand::Up => {
                events.get_inner().scrollbar_pos = min(
                    events.get_inner().scrollbar_pos + 1,
                    events.get_inner().scrollbar_len.saturating_sub(1),
                );
            }
            ItemListDetailsCommand::Down => {
                events.get_inner().scrollbar_pos =
                    events.get_inner().scrollbar_pos.saturating_sub(1);
            }
            ItemListDetailsCommand::Left => {
                events.get_inner().entries.left();
            }
            ItemListDetailsCommand::Right => {
                events.get_inner().entries.right();
            }
            ItemListDetailsCommand::Reload => {
                break Ok(Navigation::Replace(NextScreen::FetchItemListDetails(item)));
            }
            ItemListDetailsCommand::Play => {
                if let Some(entry) = events.get_inner().entries.get()
                    && let Some(next) = entry.play()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::ItemListDetails(item, entries, images_available),
                        next,
                    });
                }
            }
            ItemListDetailsCommand::Open => {
                if let Some(entry) = events.get_inner().entries.get() {
                    let next = entry.open();
                    break Ok(Navigation::Push {
                        current: NextScreen::ItemListDetails(item, entries, images_available),
                        next,
                    });
                }
            }
            ItemListDetailsCommand::OpenEpisode => {
                if let Some(entry) = events.get_inner().entries.get()
                    && let Some(next) = entry.episode()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::ItemListDetails(item, entries, images_available),
                        next,
                    });
                }
            }
            ItemListDetailsCommand::OpenSeason => {
                if let Some(entry) = events.get_inner().entries.get()
                    && let Some(next) = entry.season()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::ItemListDetails(item, entries, images_available),
                        next,
                    });
                }
            }
            ItemListDetailsCommand::OpenSeries => {
                if let Some(entry) = events.get_inner().entries.get()
                    && let Some(next) = entry.series()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::ItemListDetails(item, entries, images_available),
                        next,
                    });
                }
            }
        }
    }
}
