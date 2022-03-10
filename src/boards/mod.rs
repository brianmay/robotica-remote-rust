use std::sync::mpsc;

use anyhow::Result;

use crate::display;
use crate::messages;
use crate::wifi;

#[cfg(feature = "lca2021_badge")]
mod lca2021_badge;

#[cfg(feature = "lca2021_badge")]
use lca2021_badge as board;

#[cfg(feature = "robotica")]
mod robotica;

#[cfg(feature = "robotica")]
use robotica as board;

pub const NUM_DISPLAYS: usize = board::NUM_DISPLAYS;
// pub const NUM_PAGES: usize = board::NUM_PAGES;

pub fn configure_devices(
    tx: mpsc::Sender<messages::Message>,
) -> Result<(Box<dyn wifi::Wifi>, mpsc::Sender<display::DisplayCommand>)> {
    board::configure_devices(tx)
}

pub fn initialize() {
    board::initialize()
}
