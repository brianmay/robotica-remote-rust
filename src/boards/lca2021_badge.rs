use std::sync::mpsc;

use anyhow::Error;

use esp_idf_hal::prelude::Peripherals;

use log::*;

use crate::button;
use crate::display;
use crate::messages;
use crate::wifi;

type Result<T, E = Error> = core::result::Result<T, E>;

pub fn configure_devices(
    tx: mpsc::Sender<messages::Message>,
) -> Result<(Box<dyn wifi::Wifi>, mpsc::Sender<display::DisplayCommand>)> {
    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let display = display::lca2021_badge::connect(peripherals.i2c0, pins.gpio4, pins.gpio5)?;

    let wifi = wifi::esp::connect()?;

    let pin = pins.gpio16.into_input().unwrap();
    button::esp::configure_button(pin, tx.clone(), 0)?;

    let pin = pins.gpio17.into_input().unwrap();
    button::esp::configure_button(pin, tx, 1)?;

    Ok((Box::new(wifi), display))
}

pub fn initialize() {
    esp_idf_sys::link_patches();

    use pretty_env_logger::env_logger::WriteStyle;

    pretty_env_logger::formatted_timed_builder()
        .filter(None, LevelFilter::Trace)
        .write_style(WriteStyle::Always)
        .init();
}
