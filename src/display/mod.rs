use crate::button_controllers;

#[cfg(feature = "lca2021_badge")]
pub mod lca2021_badge;

#[cfg(feature = "robotica")]
pub mod robotica;

#[cfg(feature = "lca2021_badge")]
pub mod graphics;

pub enum DisplayCommand {
    DisplayState(
        button_controllers::DisplayState,
        button_controllers::Icon,
        usize,
        String,
    ),
    BlankAll,
    UnBlankAll,
    PageUp,
    PageDown,
    ButtonPressed(usize),
    ButtonReleased(usize),
}
