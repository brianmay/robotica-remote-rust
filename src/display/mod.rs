use crate::button_controllers;

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

#[derive(Debug)]
pub enum DisplayCommand {
    Started,
    DisplayState(
        button_controllers::DisplayState,
        button_controllers::Icon,
        usize,
        String,
    ),
    DisplayNone(usize),
    BlankAll,
    UnBlankAll,
    ShowPage(usize),
    ButtonPressed(usize),
    ButtonReleased(usize),
}
