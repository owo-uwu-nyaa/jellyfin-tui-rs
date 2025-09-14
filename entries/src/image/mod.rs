use std::{
    cmp::min,
    ops::DerefMut,
    sync::{Arc, Weak, atomic::Ordering},
};

use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui_image::{
    FilterType, Resize, ResizeEncodeRender, picker::Picker, protocol::StatefulProtocol,
};
use spawn::spawn;
use tracing::{error_span, instrument, trace};

use crate::{
    entry::{IMAGE_WIDTH, image_height},
    image::{
        available::{ImagesAvailable, ImagesAvailableInner},
        cache::ImageProtocolKey,
        fetch::fetch_image,
        state::{ImageStateInner, ImageStateInnerState, JellyfinImageState},
    },
};

pub mod available;
pub mod cache;
mod fetch;
mod parse;
pub mod state;

#[instrument(skip_all)]
fn resize_image(
    resize: Resize,
    area: ratatui::layout::Rect,
    out: &Weak<ImageStateInner>,
    wake: &ImagesAvailableInner,
) {
    trace!("resizing image");
    if let Some(out) = out.upgrade() {
        let mut value = out.value.lock();
        if let ImageStateInnerState::Image(protocol, _, _) = value.deref_mut() {
            protocol.resize_encode(&resize, area);
            trace!("resized");
        } else {
            *value = ImageStateInnerState::Invalid;
            panic!("tried to resize invalid state");
        }
        out.ready.store(true, Ordering::SeqCst);
        wake.wake();
    } else {
        trace!("cancelled");
    }
}

pub struct JellyfinImage {
    resize: Resize,
}

impl Default for JellyfinImage {
    fn default() -> Self {
        Self {
            resize: Resize::Scale(FilterType::Triangle.into()),
        }
    }
}

impl JellyfinImage {
    #[allow(unused)]
    pub fn resize(self, resize: Resize) -> JellyfinImage {
        JellyfinImage { resize }
    }

    #[instrument(skip_all)]
    #[allow(clippy::too_many_arguments)]
    fn render_image_inner(
        self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        mut image: StatefulProtocol,
        key: ImageProtocolKey,
        state_mut: &mut ImageStateInnerState,
        state: &JellyfinImageState,
        availabe: &ImagesAvailable,
        width: u16,
    ) {
        let [area] = Layout::horizontal([Constraint::Length(width)])
            .flex(Flex::Center)
            .areas(area);
        if let Some(area) = image.needs_resize(&self.resize, area) {
            trace!("image needs resize");
            state.inner.ready.store(false, Ordering::SeqCst);
            *state_mut = ImageStateInnerState::Image(image, key, width);
            let resize = self.resize;
            let out = Arc::downgrade(&state.inner);
            let wake = availabe.inner.clone();
            rayon::spawn(move || resize_image(resize, area, &out, &wake));
        } else {
            image.render(area, buf);
            *state_mut = ImageStateInnerState::Image(image, key, width);
        }
    }

    #[instrument(skip_all, name = "render_image")]
    pub fn render(
        self,
        area: Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut JellyfinImageState,
        availabe: &ImagesAvailable,
        picker: &Picker,
    ) {
        if state.inner.ready.load(Ordering::SeqCst) {
            let mut value_ref = state.inner.value.lock();
            let value = std::mem::take(value_ref.deref_mut());
            match value {
                ImageStateInnerState::Invalid => panic!("image in invalid state"),
                ImageStateInnerState::ImageReady(dynamic_image, key) => {
                    trace!("image ready");
                    let image_height = image_height(picker.font_size());
                    let height = image_height * picker.font_size().1;
                    let height: f64 = height.into();
                    let width =
                        (height / (dynamic_image.height() as f64)) * (dynamic_image.width() as f64);
                    let width = width / (picker.font_size().0 as f64);
                    let width = min(width.ceil() as u16, IMAGE_WIDTH);
                    let image = picker.new_resize_protocol(dynamic_image);
                    self.render_image_inner(
                        area,
                        buf,
                        image,
                        key,
                        value_ref.deref_mut(),
                        state,
                        availabe,
                        width,
                    );
                }
                ImageStateInnerState::Image(image, key, width) => {
                    self.render_image_inner(
                        area,
                        buf,
                        image,
                        key,
                        value_ref.deref_mut(),
                        state,
                        availabe,
                        width,
                    );
                }
                ImageStateInnerState::Lazy {
                    get_image,
                    db,
                    tag,
                    item_id,
                    image_type,
                    cancel,
                } => {
                    state.inner.ready.store(false, Ordering::SeqCst);
                    spawn(
                        fetch_image(
                            get_image,
                            db,
                            tag,
                            item_id,
                            image_type,
                            cancel,
                            Arc::downgrade(&state.inner),
                            availabe.inner.clone(),
                            state.inner.cache.clone(),
                        ),
                        error_span!("fetch_image"),
                    );
                }
            }
        }
    }
}
