use keybinds::{BindingMap, Command, keybind_config};

#[derive(Debug)]
#[keybind_config]
pub struct Keybinds {
    pub fetch: BindingMap<LoadingCommand>,
    pub play_mpv: BindingMap<MpvCommand>,
    pub user_view: BindingMap<UserViewCommand>,
    pub home_screen: BindingMap<HomeScreenCommand>,
    pub login_info: BindingMap<LoginInfoCommand>,
    pub error: BindingMap<ErrorCommand>,
    pub item_details: BindingMap<ItemDetailsCommand>,
    pub item_list_details: BindingMap<ItemListDetailsCommand>,
    pub refresh_item: BindingMap<RefreshItemCommand>,
}

#[derive(Debug, Clone, Copy, Command)]
pub enum LoadingCommand {
    Quit,
}

#[derive(Debug, Clone, Copy, Command)]
pub enum MpvCommand {
    Quit,
    Pause,
}

#[derive(Debug, Clone, Copy, Command)]
pub enum RefreshItemCommand {
    Quit,
    Up,
    Down,
    Select,
}

#[derive(Debug, Clone, Copy, Command)]
pub enum UserViewCommand {
    Quit,
    Reload,
    Prev,
    Next,
    Up,
    Down,
    Open,
    Play,
    OpenEpisode,
    OpenSeason,
    OpenSeries,
    RefreshItem,
}

#[derive(Debug, Clone, Copy, Command)]
pub enum HomeScreenCommand {
    Quit,
    Reload,
    Left,
    Right,
    Up,
    Down,
    Open,
    Play,
    PlayOpen,
    OpenEpisode,
    OpenSeason,
    OpenSeries,
    RefreshItem,
}

#[derive(Debug, Clone, Copy, Command)]
pub enum LoginInfoCommand {
    Delete,
    Submit,
    Next,
    Prev,
    Quit,
}

#[derive(Debug, Clone, Copy, Command)]
pub enum ErrorCommand {
    Quit,
    Kill,
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, Command)]
pub enum ItemDetailsCommand {
    Quit,
    Up,
    Down,
    Play,
    Reload,
    RefreshItem,
}

#[derive(Debug, Clone, Copy, Command)]
pub enum ItemListDetailsCommand {
    Quit,
    Reload,
    Up,
    Down,
    Left,
    Right,
    Play,
    Open,
    OpenEpisode,
    OpenSeason,
    OpenSeries,
    RefreshCurrentItem,
    RefreshParentItem,
}
