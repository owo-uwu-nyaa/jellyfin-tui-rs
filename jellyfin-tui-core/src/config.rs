use std::path::PathBuf;

use libmpv::MpvProfile;

use crate::keybinds::Keybinds;

#[derive(Debug)]
pub struct Config {
    pub hwdec: String,
    pub keybinds: Keybinds,
    pub login_file: PathBuf,
    pub mpv_log_level: String,
    pub mpv_profile: MpvProfile,
}
