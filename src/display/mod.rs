use robotica_common::controllers::DisplayState;

use self::icon::Icon;

#[cfg(feature = "lca2021_badge")]
pub mod lca2021_badge;

#[cfg(feature = "makerfab")]
pub mod makerfab;

#[cfg(feature = "robotica")]
pub mod robotica;

#[cfg(feature = "lca2021_badge")]
pub mod graphics;

#[cfg(feature = "makerfab")]
pub mod graphics;

pub mod icon;

#[derive(Debug)]
pub enum DisplayCommand {
    Started,
    DisplayState(DisplayState, Icon, usize, String),
    DisplayNone(usize),
    BlankAll,
    UnBlankAll,
    ShowPage(usize),
    ButtonPressed(usize),
    ButtonReleased(usize),
}
