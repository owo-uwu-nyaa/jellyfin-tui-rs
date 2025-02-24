use jellyfin::items::MediaItem;

use crate::{mpv::MpvPlayer, NextScreen, TuiContext};
use color_eyre::Result;

//open /Videos/{id}/main.m3u8 in mpv with correct auth headers
//
pub async fn play_item(cx: &mut TuiContext,_item: MediaItem)->Result<NextScreen>{
    let _mpv = MpvPlayer::new(cx);

    Ok(NextScreen::Quit)
}
