use std::{
    io::Cursor,
    task::{self, Poll},
};

use bytes::Bytes;
use color_eyre::eyre::{eyre, Context};
use futures_util::FutureExt;
use image::DynamicImage;
use jellyfin::{image::GetImageQuery, items::ImageType, sha::Sha256, AuthStatus, JellyfinClient};
use ratatui::layout::Rect;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, Resize};
use tokio::sync::oneshot;
use url::Url;

use crate::Result;

pub enum LoadImage {
    DynamicImage(oneshot::Receiver<Result<DynamicImage>>),
    Resized(oneshot::Receiver<StatefulProtocol>),
    Ready(StatefulProtocol),
    Err,
}

async fn get_image(
    client: reqwest::Client,
    url: Url,
    tag: String,
    item_id: String,
    image_type: ImageType,
) -> Result<Bytes> {
    client
        .get(format!("{}Items/{item_id}/Images/{image_type}", url))
        .query(&GetImageQuery {
            tag: Some(tag.as_str()),
            format: Some("webp"),
        })
        .send()
        .await
        .context("receiving image response")?
        .error_for_status()
        .context("getting image")?
        .bytes()
        .await
        .context("receiving image body")
}

fn parse_image(val: Bytes) -> Result<DynamicImage> {
    image::ImageReader::new(Cursor::new(val))
        .with_guessed_format()
        .context("unable to guess format")?
        .decode()
        .context("error decoding image")
}

impl LoadImage {
    pub fn new(
        client: &JellyfinClient<impl AuthStatus, impl Sha256>,
        tag: String,
        item_id: String,
        image_type: ImageType,
    ) -> Self {
        let (send, receive) = oneshot::channel();
        let url = client.get_base_url().clone();
        let client = client.get_http_client().clone();
        tokio::spawn(async move {
            match get_image(client, url, tag, item_id, image_type).await {
                Ok(val) => {
                    rayon::spawn(move || {
                        let _ = send.send(parse_image(val));
                    });
                }
                Err(e) => {
                    let _ = send.send(Err(e));
                }
            }
        });
        Self::DynamicImage(receive)
    }
    pub fn poll(
        &mut self,
        cx: &mut task::Context,
        picker: &Picker,
        resize: Resize,
        area: Rect,
    ) -> Result<Option<&mut StatefulProtocol>> {
        match self {
            LoadImage::DynamicImage(receiver) => match receiver.poll_unpin(cx) {
                Poll::Ready(Ok(Ok(image))) => {
                    let image = picker.new_resize_protocol(image);
                    self.set_ready_or_resize(resize, area, image, cx)
                }
                Poll::Ready(Ok(Err(e))) => {
                    *self = LoadImage::Err;
                    Err(e).context("error receiving resized image")
                }
                Poll::Ready(Err(e)) => {
                    *self = LoadImage::Err;
                    Err(e).context("error receiving resized image")
                }
                Poll::Pending => Ok(None),
            },
            LoadImage::Resized(receiver) => match receiver.poll_unpin(cx) {
                Poll::Ready(Ok(image)) => self.set_ready_or_resize(resize, area, image, cx),
                Poll::Ready(Err(e)) => {
                    *self = LoadImage::Err;
                    Err(e).context("error receiving resized image")
                }
                Poll::Pending => Ok(None),
            },

            //this should actually recheck id the image needs to be resized. this is difficult because of lifetimes.
            LoadImage::Ready(stateful_protocol) => Ok(Some(stateful_protocol)),
            LoadImage::Err => Err(eyre!("some previous failure occured")),
        }
    }

    fn set_ready(
        &mut self,
        image: StatefulProtocol,
    ) -> std::result::Result<Option<&mut StatefulProtocol>, color_eyre::eyre::Error> {
        *self = LoadImage::Ready(image);
        match self {
            LoadImage::Ready(image) => Ok(Some(image)),
            _ => unreachable!(),
        }
    }
    fn resize(
        &mut self,
        image: StatefulProtocol,
        resize: Resize,
        area: Rect,
        cx: &mut task::Context,
    ) -> Result<Option<&mut StatefulProtocol>> {
        let (send, mut receive) = oneshot::channel();
        assert!(matches!(receive.poll_unpin(cx), Poll::Pending));
        rayon::spawn(move || {
            let mut image = image;
            image.resize_encode(&resize, image.background_color(), area);
            let _ = send.send(image);
        });
        *self = LoadImage::Resized(receive);
        Ok(None)
    }

    fn set_ready_or_resize(
        &mut self,
        resize: Resize,
        area: Rect,
        mut image: StatefulProtocol,
        cx: &mut task::Context,
    ) -> Result<Option<&mut StatefulProtocol>> {
        if let Some(area) = image.needs_resize(&resize, area) {
            self.resize(image, resize, area, cx)
        } else {
            self.set_ready(image)
        }
    }
}
