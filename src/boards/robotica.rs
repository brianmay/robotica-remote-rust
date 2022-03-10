use std::sync::mpsc;

use anyhow::Result;

use esp_idf_hal::prelude::Peripherals;

use log::*;

use crate::button;
use crate::display;
use crate::messages;
use crate::wifi;

pub const NUM_DISPLAYS: usize = display::robotica::NUM_DISPLAYS;

pub fn configure_devices(
    tx: mpsc::Sender<messages::Message>,
) -> Result<(Box<dyn wifi::Wifi>, mpsc::Sender<display::DisplayCommand>)> {
    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let pin = pins.gpio33.into_input().unwrap();
    button::configure_button(pin, tx.clone(), button::ButtonId::Physical(0))?;

    let pin = pins.gpio27.into_input().unwrap();
    button::configure_button(pin, tx.clone(), button::ButtonId::Physical(1))?;

    let pin = pins.gpio15.into_input().unwrap();
    button::configure_button(pin, tx.clone(), button::ButtonId::Physical(2))?;

    let pin = pins.gpio12.into_input().unwrap();
    button::configure_button(pin, tx.clone(), button::ButtonId::Physical(3))?;

    let display = display::robotica::connect(13, tx)?;

    let wifi = wifi::esp::connect()?;

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
