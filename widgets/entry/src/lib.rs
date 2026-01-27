use color_eyre::Result;
use jellyfin::{
    JellyfinClient,
    image::select_images,
    items::{ItemType, MediaItem},
    user_views::UserView,
};
use jellyhaj_image::{JellyfinImage, JellyfinImageState};
pub use jellyhaj_image::{Picker, SqliteConnection, Stats, cache::ImageProtocolCache};
use jellyhaj_widgets_core::{Config, FontSize, ItemWidget, JellyhajWidget, Wrapper};
use ratatui::{
    crossterm::event::{MouseButton, MouseEventKind},
    layout::{Rect, Size},
    style::Color,
    text::Span,
    widgets::{Block, BorderType, Paragraph, Widget},
};
use std::{borrow::Cow, fmt::Debug, sync::Arc};
use tracing::instrument;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum EntryData {
    Item(MediaItem),
    View(UserView),
}

pub struct Entry {
    image: Option<JellyfinImage>,
    title: String,
    subtitle: Option<String>,
    inner: EntryData,
    watch_status: Option<Cow<'static, str>>,
    size: Size,
    active: bool,
}

impl Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("title", &self.title)
            .field("subtitle", &self.subtitle)
            .field("watch_status", &self.watch_status)
            .finish_non_exhaustive()
    }
}

fn calc_dimensions(config: &Config, font_size: FontSize) -> Size {
    let image_width = config.entry_image_width;
    let image_height = {
        let width = image_width * font_size.0;
        let width: f64 = width.into();
        let height = (width / 16.0) * 9.0;
        let height = height / f64::from(font_size.1);
        height.ceil() as u16
    };
    Size {
        width: image_width + 2,
        height: image_height + 2,
    }
}

impl Entry {
    pub fn data(&self) -> &EntryData {
        &self.inner
    }
    pub fn new(
        state: EntryData,
        jellyfin: &JellyfinClient,
        db: &Arc<tokio::sync::Mutex<SqliteConnection>>,
        cache: &ImageProtocolCache,
        picker: &Arc<Picker>,
        stats: &Stats,
        config: &Config,
    ) -> Entry {
        let size = calc_dimensions(config, picker.font_size());
        match state {
            EntryData::Item(media_item) => {
                from_media_item(media_item, jellyfin, db, cache, picker, stats, size)
            }
            EntryData::View(user_view) => {
                from_user_view(user_view, jellyfin, db, cache, picker, stats, size)
            }
        }
    }
}

pub enum EntryAction {
    Inner(<JellyfinImage as JellyhajWidget>::Action),
    Activate,
    Play,
    Open,
    OpenSeries,
    OpenSeason,
    OpenEpisode,
}

pub enum EntryResult {
    Activate(EntryData),
    Play(EntryData),
    Open(EntryData),
    OpenSeries(EntryData),
    OpenSeason(EntryData),
    OpenEpisode(EntryData),
}

#[derive(Clone, Copy)]
struct EntryWrapper;

impl Wrapper<<JellyfinImage as JellyhajWidget>::Action> for EntryWrapper {
    type F = EntryAction;

    fn wrap(&self, val: <JellyfinImage as JellyhajWidget>::Action) -> Self::F {
        EntryAction::Inner(val)
    }
}

impl ItemWidget for Entry {
    type State = EntryData;
    type Action = EntryAction;
    type ActionResult = EntryResult;

    fn dimensions_static(par: jellyhaj_widgets_core::DimensionsParameter<'_>) -> Size {
        calc_dimensions(par.config, par.font_size)
    }
    fn dimensions(&self) -> Size {
        self.size
    }
    fn into_state(self) -> Self::State {
        self.inner
    }
    fn apply_action(&mut self, action: Self::Action) -> Result<Option<Self::ActionResult>> {
        Ok(Some(match action {
            EntryAction::Inner(action) => {
                if let Some(image) = self.image.as_mut() {
                    let None = image.apply_action(action)?;
                }
                return Ok(None);
            }
            EntryAction::Activate => EntryResult::Activate(self.inner.clone()),
            EntryAction::Play => EntryResult::Play(self.inner.clone()),
            EntryAction::Open => EntryResult::Open(self.inner.clone()),
            EntryAction::OpenSeries => EntryResult::OpenSeries(self.inner.clone()),
            EntryAction::OpenSeason => EntryResult::OpenSeason(self.inner.clone()),
            EntryAction::OpenEpisode => EntryResult::OpenEpisode(self.inner.clone()),
        }))
    }
    fn click(
        &mut self,
        _: ratatui::prelude::Position,
        _: Size,
        kind: ratatui::crossterm::event::MouseEventKind,
        _: ratatui::crossterm::event::KeyModifiers,
    ) -> Result<Option<Self::ActionResult>> {
        if kind == MouseEventKind::Down(MouseButton::Left) {
            Ok(Some(EntryResult::Activate(self.inner.clone())))
        } else {
            Ok(None)
        }
    }

    #[instrument(skip_all, name = "render_entry")]
    fn render_item(
        &mut self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        task: jellyhaj_widgets_core::async_task::TaskSubmitter<
            Self::Action,
            impl jellyhaj_widgets_core::Wrapper<Self::Action>,
        >,
    ) -> Result<()> {
        let mut outer = Block::bordered()
            .border_type(if self.active {
                BorderType::Double
            } else {
                BorderType::Plain
            })
            .title_top(self.title.as_str());
        if let Some(subtitle) = &self.subtitle {
            outer = outer.title_bottom(subtitle.as_str());
        }
        let inner = outer.inner(area);
        if let Some(image) = &mut self.image {
            image.render_fallible(inner, buf, task.wrap_with(EntryWrapper))?
        }
        outer.render(area, buf);
        if let Some(watch_status) = self.watch_status.as_ref() {
            Paragraph::new(Span::styled(watch_status.clone(), Color::LightBlue))
                .right_aligned()
                .render(
                    Rect {
                        x: area.x,
                        y: area.y,
                        width: area.width,
                        height: 1,
                    },
                    buf,
                );
        }
        Ok(())
    }
    fn set_active(&mut self, active: bool) {
        self.active = active
    }
}

fn from_media_item(
    item: MediaItem,
    jellyfin: &JellyfinClient,
    db: &Arc<tokio::sync::Mutex<SqliteConnection>>,
    cache: &ImageProtocolCache,
    picker: &Arc<Picker>,
    stats: &Stats,
    size: Size,
) -> Entry {
    let (title, subtitle) = match &item.item_type {
        ItemType::Movie | ItemType::Unknown | ItemType::CollectionFolder => {
            (item.name.clone(), None)
        }
        ItemType::Episode {
            season_id: _,
            season_name: _,
            series_id: _,
            series_name,
        } => (series_name.clone(), item.name.clone().into()),
        ItemType::Season {
            series_id: _,
            series_name,
        } => (series_name.clone(), item.name.clone().into()),
        ItemType::Series | ItemType::MusicAlbum => (item.name.clone(), None),
        ItemType::Playlist | ItemType::Folder => (item.name.clone(), None),
        ItemType::Music { album_id: _, album } => (album.clone(), item.name.clone().into()),
    };
    let image = select_images(&item)
        .map(|(image_type, tag)| {
            let image = JellyfinImageState {
                item_id: item.id.clone(),
                tag: tag.to_string(),
                image_type,
            };
            JellyfinImage::new(
                image,
                jellyfin.clone(),
                db.clone(),
                cache.clone(),
                stats.clone(),
                picker.clone(),
            )
        })
        .next();
    let watch_status = if let Some(user_data) = item.user_data.as_ref() {
        if let Some(num @ 1..) = user_data.unplayed_item_count {
            Some(format!("{num}").into())
        } else if user_data.played {
            Some("✓".into())
        } else {
            None
        }
    } else {
        None
    };
    Entry {
        image,
        title,
        subtitle,
        inner: EntryData::Item(item),
        watch_status,
        active: false,
        size,
    }
}

fn from_user_view(
    item: UserView,
    jellyfin: &JellyfinClient,
    db: &Arc<tokio::sync::Mutex<SqliteConnection>>,
    cache: &ImageProtocolCache,
    picker: &Arc<Picker>,
    stats: &Stats,
    size: Size,
) -> Entry {
    let title = item.name.clone();
    let image = item
        .image_tags
        .iter()
        .flat_map(|map| map.iter())
        .next()
        .map(|(image_type, tag)| {
            let image = JellyfinImageState {
                item_id: item.id.clone(),
                tag: tag.to_string(),
                image_type: *image_type,
            };
            JellyfinImage::new(
                image,
                jellyfin.clone(),
                db.clone(),
                cache.clone(),
                stats.clone(),
                picker.clone(),
            )
        });
    Entry {
        image,
        title,
        subtitle: None,
        inner: EntryData::View(item),
        watch_status: None,
        active: false,
        size,
    }
}
