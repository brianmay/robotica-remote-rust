use std::sync::mpsc;

use anyhow::Result;

use esp_idf_hal::prelude::Peripherals;

use log::*;

use crate::button;
use crate::display;
use crate::messages;
use crate::touch;
use crate::wifi;

pub const NUM_DISPLAYS: usize = display::lca2021_badge::NUM_DISPLAYS;

pub fn configure_devices(
    tx: mpsc::Sender<messages::Message>,
) -> Result<(Box<dyn wifi::Wifi>, mpsc::Sender<display::DisplayCommand>)> {
    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let display =
        display::lca2021_badge::connect(peripherals.i2c0, pins.gpio4, pins.gpio5, tx.clone())?;

    let wifi = wifi::esp::connect()?;

    let pin = pins.gpio16.into_input().unwrap();
    button::configure_button(pin, tx.clone(), button::ButtonId::Physical(0))?;

    let pin = pins.gpio17.into_input().unwrap();
    button::configure_button(pin, tx.clone(), button::ButtonId::Physical(1))?;

    let mut touch_builder = touch::TouchControllerBuilder::new().unwrap();
    let touch_pin1 = touch_builder.add_pin(pins.gpio15, 400).unwrap();
    let touch_pin2 = touch_builder.add_pin(pins.gpio12, 400).unwrap();
    let touch_pin3 = touch_builder.add_pin(pins.gpio27, 400).unwrap();
    let touch_pin4 = touch_builder.add_pin(pins.gpio14, 400).unwrap();

    button::configure_button(touch_pin1, tx.clone(), button::ButtonId::PageUp)?;
    button::configure_button(touch_pin2, tx.clone(), button::ButtonId::PageDown)?;
    button::configure_button(touch_pin3, tx.clone(), button::ButtonId::Controller(0))?;
    button::configure_button(touch_pin4, tx, button::ButtonId::Controller(1))?;

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
