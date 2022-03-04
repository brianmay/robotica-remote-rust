pub mod lca2021_badge;

use crate::button_controllers;

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
