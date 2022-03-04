use std::sync::mpsc;

use anyhow::Result;

use crate::display;
use crate::messages;
use crate::wifi;

mod lca2021_badge;

pub const NUM_DISPLAYS: usize = lca2021_badge::NUM_DISPLAYS;
// pub const NUM_PAGES: usize = lca2021_badge::NUM_PAGES;

pub fn configure_devices(
    tx: mpsc::Sender<messages::Message>,
) -> Result<(Box<dyn wifi::Wifi>, mpsc::Sender<display::DisplayCommand>)> {
    lca2021_badge::configure_devices(tx)
}

pub fn initialize() {
    lca2021_badge::initialize()
}
