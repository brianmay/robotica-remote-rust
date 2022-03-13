use std::sync::mpsc;

use anyhow::Result;

use crate::display;
use crate::messages;

#[cfg(feature = "lca2021_badge")]
mod lca2021_badge;

#[cfg(feature = "lca2021_badge")]
use lca2021_badge as board;

#[cfg(feature = "robotica")]
mod robotica;

#[cfg(feature = "robotica")]
use robotica as board;

pub trait Board {
    fn get_display(&self) -> mpsc::Sender<display::DisplayCommand>;
    fn physical_button_to_controller(&self, id: usize, page: usize) -> usize;
}

pub fn configure_devices(tx: mpsc::Sender<messages::Message>) -> Result<impl Board> {
    board::configure_devices(tx)
}
