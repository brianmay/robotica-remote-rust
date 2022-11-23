use std::sync::mpsc;

use anyhow::Result;

use esp_idf_hal::prelude::Peripherals;

use esp_idf_svc::sntp::EspSntp;
use esp_idf_svc::wifi::EspWifi;

use crate::button;
use crate::display;
use crate::messages;
use crate::wifi;

use super::Board;

pub const NUM_CONTROLLERS_PER_PAGE: usize = 4;

#[allow(dead_code)]
pub struct RoboticaBoard {
    wifi: EspWifi<'static>,
    sntp: EspSntp,
    display: mpsc::Sender<display::DisplayCommand>,
}

impl Board for RoboticaBoard {
    fn get_display(&self) -> mpsc::Sender<display::DisplayCommand> {
        self.display.clone()
    }
}

pub fn configure_devices(tx: mpsc::Sender<messages::Message>) -> Result<RoboticaBoard> {
    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    button::gpio::configure_button(pins.gpio33, tx.clone(), button::ButtonId::Physical(0))?;

    button::gpio::configure_button(pins.gpio27, tx.clone(), button::ButtonId::Physical(1))?;

    button::gpio::configure_button(pins.gpio15, tx.clone(), button::ButtonId::Physical(2))?;

    button::gpio::configure_button(pins.gpio12, tx, button::ButtonId::Physical(3))?;

    let display = display::robotica::connect(13)?;

    let (wifi, sntp) = wifi::esp::connect(peripherals.modem)?;

    Ok(RoboticaBoard {
        wifi,
        sntp,
        display,
    })
}
