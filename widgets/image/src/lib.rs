pub mod cache;
mod fetch;

use std::{cmp::min, convert::Infallible, mem, sync::Arc};

use crate::{
    cache::{ImageProtocolCache, ImageProtocolKey, ImageProtocolKeyRef},
    fetch::{ParsedImage, get_image},
};
use color_eyre::eyre::Context;
pub use jellyfin::{JellyfinClient, items::ImageType};
use jellyhaj_widgets_core::{JellyhajWidget, Wrapper, async_task::TaskSubmitter};
use ratatui::{
    layout::{Rect, Size},
    widgets::Widget,
};
pub use ratatui_image::picker::Picker;
use ratatui_image::{Image, Resize, protocol::Protocol};
pub use sqlx::SqliteConnection;
pub use stats_data::Stats;
pub use tokio;
use tracing::info_span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageSize {
    pub p_width: u32,
    pub p_height: u32,
}

pub struct JellyfinImage {
    item_id: String,
    tag: String,
    image_type: ImageType,
    jellyfin: JellyfinClient,
    db: Arc<tokio::sync::Mutex<SqliteConnection>>,
    image: Option<(Protocol, ImageSize)>,
    size: Size,
    cache: ImageProtocolCache,
    stats: Stats,
    picker: Arc<Picker>,
    loading: bool,
}

impl Drop for JellyfinImage {
    fn drop(&mut self) {
        if let Some((protocol, size)) = self.image.take() {
            let key = ImageProtocolKey {
                image_type: self.image_type,
                item_id: mem::take(&mut self.item_id),
                tag: mem::take(&mut self.tag),
                size,
            };
            self.cache.store(protocol, key);
        }
    }
}

impl JellyfinImage {
    pub fn new(
        state: JellyfinImageState,
        jellyfin: JellyfinClient,
        db: Arc<tokio::sync::Mutex<SqliteConnection>>,
        cache: ImageProtocolCache,
        stats: Stats,
        picker: Arc<Picker>,
    ) -> Self {
        Self {
            item_id: state.item_id,
            tag: state.tag,
            image_type: state.image_type,
            jellyfin,
            db,
            image: None,
            size: Size::ZERO,
            cache,
            stats,
            picker,
            loading: false,
        }
    }

    fn get_image(
        &mut self,
        task_submitter: TaskSubmitter<ParsedImage, impl Wrapper<ParsedImage>>,
    ) -> Option<&Protocol> {
        if self.image.is_some() {
            self.image.as_ref().map(|(p, _)| p)
        } else {
            let p_height = (self.size.height as u32) * (self.picker.font_size().1 as u32);
            let p_width = (self.size.width as u32) * (self.picker.font_size().0 as u32);
            if !self.loading {
                let image_size = ImageSize { p_width, p_height };
                let cached = self.cache.remove(&ImageProtocolKeyRef::new(
                    self.image_type,
                    &self.item_id,
                    &self.tag,
                    image_size,
                ));
                if let Some(image) = cached {
                    Some(&self.image.insert((image, image_size)).0)
                } else {
                    let key = ImageProtocolKey {
                        image_type: self.image_type,
                        item_id: self.item_id.clone(),
                        tag: self.tag.clone(),
                        size: image_size,
                    };
                    let db = self.db.clone();
                    let jellyfin = self.jellyfin.clone();
                    let size = self.size;
                    let stats = self.stats.clone();
                    task_submitter.spawn_task(
                        async move { get_image(key, db, jellyfin, size, stats).await },
                        info_span!("get_image"),
                    );
                    None
                }
            } else {
                None
            }
        }
    }
}

pub struct JellyfinImageState {
    pub item_id: String,
    pub tag: String,
    pub image_type: ImageType,
}

impl JellyhajWidget for JellyfinImage {
    type State = JellyfinImageState;
    type Action = ParsedImage;
    type ActionResult = Infallible;

    fn into_state(self) -> Self::State {
        JellyfinImageState {
            item_id: self.item_id.clone(),
            tag: self.tag.clone(),
            image_type: self.image_type,
        }
    }

    fn render_fallible_inner(
        &mut self,
        mut area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        task: TaskSubmitter<Self::Action, impl Wrapper<Self::Action>>,
    ) -> jellyhaj_widgets_core::Result<()> {
        let new_size = area.as_size();
        let old_size = self.size;
        if new_size != old_size {
            self.size = new_size;
            self.image = None;
        }
        if let Some(image) = self.get_image(task) {
            area.x += (area.width - new_size.width) / 2;
            area.y += (area.height - new_size.height) / 2;
            area.width = new_size.width;
            area.height = new_size.height;
            Image::new(image).render(area, buf)
        }
        Ok(())
    }

    fn apply_action(
        &mut self,
        action: Self::Action,
    ) -> jellyhaj_widgets_core::Result<Option<Self::ActionResult>> {
        self.loading = false;
        if action.size == self.size {
            let width = min(
                self.size.width as u32,
                action
                    .image
                    .width()
                    .div_ceil(self.picker.font_size().0 as u32),
            ) as u16;
            let height = min(
                self.size.height as u32,
                action
                    .image
                    .height()
                    .div_ceil(self.picker.font_size().1 as u32),
            ) as u16;
            let image = self
                .picker
                .new_protocol(
                    action.image,
                    Rect {
                        x: 0,
                        y: 0,
                        width,
                        height,
                    },
                    Resize::Fit(None),
                )
                .context("generating protocol")?;
            self.image = Some((image, action.image_size))
        }
        Ok(None)
    }

    fn click(
        &mut self,
        _: ratatui::prelude::Position,
        _: Size,
        _: ratatui::crossterm::event::MouseEventKind,
        _: ratatui::crossterm::event::KeyModifiers,
    ) -> jellyhaj_widgets_core::Result<Option<Self::ActionResult>> {
        Ok(None)
    }

    fn min_width(&self) -> Option<u16> {
        Some(1)
    }
    fn min_height(&self) -> Option<u16> {
        Some(1)
    }
    fn min_width_static(_par: jellyhaj_widgets_core::DimensionsParameter<'_>) -> Option<u16> {
        Some(1)
    }
    fn min_height_static(_par: jellyhaj_widgets_core::DimensionsParameter<'_>) -> Option<u16> {
        Some(1)
    }
}
