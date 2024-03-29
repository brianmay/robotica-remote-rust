use std::sync::mpsc;

use anyhow::Result;

use crate::display;
use crate::messages;

#[cfg(feature = "lca2021_badge")]
pub mod lca2021_badge;

#[cfg(feature = "lca2021_badge")]
use lca2021_badge as board;

#[cfg(feature = "robotica")]
pub mod robotica;

#[cfg(feature = "robotica")]
use robotica as board;

#[cfg(feature = "makerfab")]
pub mod makerfab;

#[cfg(feature = "makerfab")]
use makerfab as board;

pub const NUM_CONTROLLERS_PER_PAGE: usize = board::NUM_CONTROLLERS_PER_PAGE;

pub trait Board {
    fn get_display(&self) -> mpsc::Sender<display::DisplayCommand>;
}

pub fn configure_devices(tx: mpsc::Sender<messages::Message>) -> Result<impl Board> {
    board::configure_devices(tx)
}
