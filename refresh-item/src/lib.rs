use std::pin::Pin;

use checkbox::Checkbox;
use color_eyre::{Result, eyre::Context};
use futures_util::StreamExt;
use jellyfin::items::{RefreshItemQuery, RefreshMode};
use jellyhaj_core::{
    context::TuiContext,
    keybinds::RefreshItemCommand,
    state::{Navigation, NextScreen},
};
use keybinds::{KeybindEvent, KeybindEventStream};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Modifier,
    widgets::{Block, BorderType, Clear, Padding, Widget, WidgetRef},
};
use ratatui_fallible_widget::TermExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Action {
    #[default]
    NewUpdated,
    MissingMetadata,
    ReplaceMetadata,
}

impl Action {
    fn to_str(self) -> &'static str {
        match self {
            Action::NewUpdated => "Scan for new and updated files",
            Action::MissingMetadata => "Search for missing metadata",
            Action::ReplaceMetadata => "Replace all metadata",
        }
    }
    const ALL: &[Action] = &[
        Action::NewUpdated,
        Action::MissingMetadata,
        Action::ReplaceMetadata,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Active {
    #[default]
    Action,
    ActionSelection(Action),
    ReplaceImages,
    ReplaceTrickplay,
    Refresh,
}

#[derive(Debug, Default)]
struct RefreshItem {
    action: Action,
    active: Active,
    replace_images: bool,
    replace_trickplay: bool,
}

impl RefreshItem {
    fn to_query(&self) -> RefreshItemQuery {
        match self.action {
            Action::NewUpdated => RefreshItemQuery {
                recursive: true,
                metadata_refresh_mode: RefreshMode::Default,
                image_refresh_mode: RefreshMode::Default,
                replace_all_metadata: false,
                replace_all_images: false,
                regenerate_trickplay: false,
            },
            Action::MissingMetadata => RefreshItemQuery {
                recursive: true,
                metadata_refresh_mode: RefreshMode::FullRefresh,
                image_refresh_mode: RefreshMode::FullRefresh,
                replace_all_metadata: false,
                replace_all_images: self.replace_images,
                regenerate_trickplay: self.replace_trickplay,
            },
            Action::ReplaceMetadata => RefreshItemQuery {
                recursive: true,
                metadata_refresh_mode: RefreshMode::FullRefresh,
                image_refresh_mode: RefreshMode::FullRefresh,
                replace_all_metadata: true,
                replace_all_images: self.replace_images,
                regenerate_trickplay: self.replace_trickplay,
            },
        }
    }
}

pub async fn show_refresh_item(cx: Pin<&mut TuiContext>, item: String) -> Result<Navigation> {
    let cx = cx.project();
    let mut widget = RefreshItem::default();
    let mut events = KeybindEventStream::new(
        cx.events,
        &mut widget,
        cx.config.keybinds.refresh_item.clone(),
        &cx.config.help_prefixes,
    );
    loop {
        cx.term.draw_fallible(&mut events)?;
        match events.next().await {
            None => return Ok(Navigation::Exit),
            Some(Err(e)) => return Err(e),
            Some(Ok(KeybindEvent::Render)) => {}
            Some(Ok(KeybindEvent::Text(_))) => unreachable!(),
            Some(Ok(KeybindEvent::Command(RefreshItemCommand::Quit))) => {
                let widget = events.get_inner();
                if let Active::ActionSelection(_) = widget.active {
                    widget.active = Active::Action
                } else {
                    return Ok(Navigation::PopContext);
                }
            }
            Some(Ok(KeybindEvent::Command(RefreshItemCommand::Down))) => {
                let widget = events.get_inner();
                let active = match widget.active {
                    Active::Action => {
                        if widget.action == Action::NewUpdated {
                            Active::Refresh
                        } else {
                            Active::ReplaceImages
                        }
                    }
                    Active::ActionSelection(Action::NewUpdated) => {
                        Active::ActionSelection(Action::MissingMetadata)
                    }
                    Active::ActionSelection(Action::MissingMetadata) => {
                        Active::ActionSelection(Action::ReplaceMetadata)
                    }
                    Active::ActionSelection(Action::ReplaceMetadata) => {
                        Active::ActionSelection(Action::NewUpdated)
                    }
                    Active::ReplaceImages => Active::ReplaceTrickplay,
                    Active::ReplaceTrickplay => Active::Refresh,
                    Active::Refresh => Active::Action,
                };
                widget.active = active;
            }
            Some(Ok(KeybindEvent::Command(RefreshItemCommand::Up))) => {
                let widget = events.get_inner();
                let active = match widget.active {
                    Active::Refresh => {
                        if widget.action == Action::NewUpdated {
                            Active::Action
                        } else {
                            Active::ReplaceTrickplay
                        }
                    }
                    Active::ActionSelection(Action::NewUpdated) => {
                        Active::ActionSelection(Action::ReplaceMetadata)
                    }
                    Active::ActionSelection(Action::MissingMetadata) => {
                        Active::ActionSelection(Action::NewUpdated)
                    }
                    Active::ActionSelection(Action::ReplaceMetadata) => {
                        Active::ActionSelection(Action::MissingMetadata)
                    }
                    Active::ReplaceTrickplay => Active::ReplaceImages,
                    Active::ReplaceImages => Active::Action,
                    Active::Action => Active::Refresh,
                };
                widget.active = active;
            }
            Some(Ok(KeybindEvent::Command(RefreshItemCommand::Select))) => {
                let widget = events.get_inner();
                let current = widget.active;
                match current {
                    Active::Action => widget.active = Active::ActionSelection(Action::default()),
                    Active::ActionSelection(action) => {
                        widget.active = Active::Action;
                        widget.action = action;
                    }
                    Active::ReplaceImages => widget.replace_images ^= true,
                    Active::ReplaceTrickplay => widget.replace_trickplay ^= true,
                    Active::Refresh => {
                        return Ok(Navigation::Replace(NextScreen::SendRefreshItem(
                            item,
                            widget.to_query(),
                        )));
                    }
                }
            }
        }
    }
}

impl Widget for &RefreshItem {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title("Refresh Metadata")
            .padding(Padding::uniform(1));
        let [
            action_area,
            replace_images_area,
            replace_trickplay_area,
            refresh_area,
        ] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
        ])
        .spacing(1)
        .areas(block.inner(area));
        let action_block =
            Block::bordered()
                .title("Refresh mode")
                .border_type(if self.active == Active::Action {
                    BorderType::Double
                } else {
                    BorderType::Plain
                });
        let mut action_inner = action_block.inner(action_area);
        let arrow_char = if let Active::ActionSelection(_) = self.active {
            '⮙'
        } else {
            '⮛'
        };
        buf[(action_inner.x + action_inner.width - 1, action_inner.y)].set_char(arrow_char);
        action_inner.width -= 2;
        self.action.to_str().render_ref(action_inner, buf);
        action_block.render(action_area, buf);
        if self.action != Action::NewUpdated {
            Checkbox::new(self.active == Active::ReplaceImages, self.replace_images).render_with(
                replace_images_area,
                buf,
                "Replace existing images",
            );
            Checkbox::new(
                self.active == Active::ReplaceTrickplay,
                self.replace_trickplay,
            )
            .render_with(
                replace_trickplay_area,
                buf,
                "Replace existing trickplay images",
            );
        }
        let refresh_block = Block::bordered().border_type(if self.active == Active::Refresh {
            BorderType::Double
        } else {
            BorderType::Plain
        });
        let refresh_text = "Refresh Now!";
        let refresh_area = refresh_area.centered(
            Constraint::Length((refresh_text.len() as u16) + 2),
            Constraint::Length(3),
        );

        refresh_text.render(refresh_block.inner(refresh_area), buf);
        refresh_block.render(refresh_area, buf);
        block.render(area, buf);
        if let Active::ActionSelection(action) = self.active {
            let mut area = action_area;
            area.y += 2;
            area.height = 2 + Action::ALL.len() as u16;
            area.width = 2 + Action::ALL
                .iter()
                .map(|a| a.to_str().len())
                .max()
                .unwrap_or(0) as u16;
            Clear.render(area, buf);
            let selection_block = Block::bordered().border_type(BorderType::Thick);
            let inner = selection_block.inner(area);
            for (i, c) in Action::ALL.iter().copied().enumerate() {
                let mut area = inner;
                area.y += i as u16;
                area.height = 1;
                c.to_str().render(area, buf);
                if action == c {
                    for i in 0..area.width {
                        buf[(area.x + i, area.y)].set_style(Modifier::REVERSED);
                    }
                }
            }
            selection_block.render(area, buf);
        }
    }
}

pub async fn refresh_screen(
    cx: Pin<&mut TuiContext>,
    item_id: String,
    query: RefreshItemQuery,
) -> Result<Navigation> {
    let cx = cx.project();
    let jellyfin = cx.jellyfin;
    fetch::fetch_screen(
        "Refreshing Item",
        async {
            jellyfin
                .refresh_item(&item_id, &query)
                .await
                .context("refreshing jellyfin item")?;

            Ok(Navigation::PopContext)
        },
        cx.events,
        cx.config.keybinds.fetch.clone(),
        cx.term,
        &cx.config.help_prefixes,
    )
    .await
}
