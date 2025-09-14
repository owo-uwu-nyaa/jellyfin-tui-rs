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
    widgets::{Block, Padding, Paragraph, Scrollbar, ScrollbarState, Widget},
};

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
    Ok(Navigation::Replace(NextScreen::ItemListDetails(
        item,
        EntryList::new(
            childs
                .iter()
                .map(|item| {
                    Entry::from_media_item(item.clone(), &cx.jellyfin, &cx.cache, &cx.image_cache)
                })
                .collect::<Result<Vec<_>>>()?,
            name,
        ),
    )))
}

pub async fn display_item_list_details(
    cx: Pin<&mut TuiContext>,
    item: MediaItem,
    mut entries: EntryList,
) -> Result<Navigation> {
    let images_available = ImagesAvailable::new();
    let cx = cx.project();
    let mut events =
        KeybindEventStream::new(cx.events, cx.config.keybinds.item_list_details.clone());
    let block = Block::bordered().padding(ratatui::widgets::Padding::uniform(1));
    let mut width = None;
    let mut scrollbar_state = ScrollbarState::new(0);
    let mut scrollbar_pos = 0;
    let mut scrollbar_len = 0;
    let mut descr = None;
    loop {
        cx.term
            .draw(|frame| {
                let height = entry_list_height(cx.image_picker.font_size());
                let main = block.inner(events.inner(frame.area()));
                let [entry_area, descripton_area] =
                    Layout::vertical([Constraint::Length(height), Constraint::Min(1)])
                        .spacing(1)
                        .areas(main);
                entries.render(
                    entry_area,
                    frame.buffer_mut(),
                    &images_available,
                    cx.image_picker,
                    true,
                );
                let w = descripton_area.width.saturating_sub(4);
                if width != Some(w) {
                    width = Some(w);
                    if let Some(d) = &item.overview {
                        let lines = textwrap::wrap(d, w as usize);
                        scrollbar_state = scrollbar_state.content_length(lines.len());
                        scrollbar_len = lines.len() as u16;
                        scrollbar_pos = min(scrollbar_pos, scrollbar_len.saturating_sub(1));
                        descr = Some(
                            Paragraph::new(Text::from_iter(lines.into_iter())).block(
                                Block::bordered()
                                    .title("Overview")
                                    .padding(Padding::uniform(1)),
                            ),
                        );
                    }
                }
                if let Some(descr) = &mut descr {
                    frame.render_widget(descr.clone().scroll((scrollbar_pos, 0)), descripton_area);
                    frame.render_stateful_widget(
                        Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight),
                        descripton_area.inner(Margin {
                            horizontal: 0,
                            vertical: 2,
                        }),
                        &mut scrollbar_state,
                    );
                }
                events.render(frame.area(), frame.buffer_mut());
            })
            .context("drawing item list details")?;
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
                scrollbar_pos = min(scrollbar_pos + 1, scrollbar_len.saturating_sub(1));
            }
            ItemListDetailsCommand::Down => {
                scrollbar_pos = scrollbar_pos.saturating_sub(1);
            }
            ItemListDetailsCommand::Left => {
                entries.left();
            }
            ItemListDetailsCommand::Right => {
                entries.right();
            }
            ItemListDetailsCommand::Play => {
                if let Some(entry) = entries.get()
                    && let Some(next) = entry.play()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::ItemListDetails(item, entries),
                        next,
                    });
                }
            }
            ItemListDetailsCommand::Open => {
                if let Some(entry) = entries.get() {
                    let next = entry.open();
                    break Ok(Navigation::Push {
                        current: NextScreen::ItemListDetails(item, entries),
                        next,
                    });
                }
            }
            ItemListDetailsCommand::OpenEpisode => {
                if let Some(entry) = entries.get()
                    && let Some(next) = entry.episode()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::ItemListDetails(item, entries),
                        next,
                    });
                }
            }
            ItemListDetailsCommand::OpenSeason => {
                if let Some(entry) = entries.get()
                    && let Some(next) = entry.season()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::ItemListDetails(item, entries),
                        next,
                    });
                }
            }
            ItemListDetailsCommand::OpenSeries => {
                if let Some(entry) = entries.get()
                    && let Some(next) = entry.series()
                {
                    break Ok(Navigation::Push {
                        current: NextScreen::ItemListDetails(item, entries),
                        next,
                    });
                }
            }
        }
    }
}
