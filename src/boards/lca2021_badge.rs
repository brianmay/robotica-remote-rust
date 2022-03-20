use std::sync::mpsc;

use anyhow::Result;

use esp_idf_hal::prelude::Peripherals;

use esp_idf_svc::sntp::EspSntp;
use esp_idf_svc::wifi::EspWifi;

use crate::button;
use crate::display;
use crate::input::esp32::TouchControllerBuilder;
use crate::messages;
use crate::wifi;

use super::Board;

pub const NUM_CONTROLLERS_PER_PAGE: usize = display::lca2021_badge::NUM_PER_PAGE;

#[allow(dead_code)]
pub struct Lca2022Badge {
    wifi: EspWifi,
    sntp: EspSntp,
    display: mpsc::Sender<display::DisplayCommand>,
}

impl Board for Lca2022Badge {
    fn get_display(&self) -> mpsc::Sender<display::DisplayCommand> {
        self.display.clone()
    }
}

pub fn configure_devices(tx: mpsc::Sender<messages::Message>) -> Result<Lca2022Badge> {
    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let display = display::lca2021_badge::connect(peripherals.i2c0, pins.gpio4, pins.gpio5)?;

    let (wifi, sntp) = wifi::esp::connect()?;

    let pin = pins.gpio16.into_input().unwrap();
    button::gpio::configure_button(pin, tx.clone(), button::ButtonId::Physical(0))?;

    let pin = pins.gpio17.into_input().unwrap();
    button::gpio::configure_button(pin, tx.clone(), button::ButtonId::Physical(1))?;

    let mut touch_builder = TouchControllerBuilder::new().unwrap();
    let touch_pin1 = touch_builder.add_pin(pins.gpio15, 400).unwrap();
    let touch_pin2 = touch_builder.add_pin(pins.gpio12, 400).unwrap();
    // let touch_pin3 = touch_builder.add_pin(pins.gpio27, 400).unwrap();
    // let touch_pin4 = touch_builder.add_pin(pins.gpio14, 400).unwrap();

    button::touch::configure_touch_button(touch_pin1, tx.clone(), button::ButtonId::PageUp)?;
    button::touch::configure_touch_button(touch_pin2, tx, button::ButtonId::PageDown)?;
    // button::touch::configure_touch_button(touch_pin3, tx.clone(), button::ButtonId::Controller(0))?;
    // button::touch::configure_touch_button(touch_pin4, tx, button::ButtonId::Controller(1))?;

    Ok(Lca2022Badge {
        wifi,
        sntp,
        display,
    })
}
