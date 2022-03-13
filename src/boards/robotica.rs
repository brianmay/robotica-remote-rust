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

#[allow(dead_code)]
pub struct RoboticaBoard {
    wifi: EspWifi,
    sntp: EspSntp,
    display: mpsc::Sender<display::DisplayCommand>,
}

impl Board for RoboticaBoard {
    fn get_display(&self) -> mpsc::Sender<display::DisplayCommand> {
        self.display.clone()
    }

    fn physical_button_to_controller(&self, id: usize, _page: usize) -> usize {
        id
    }
}

pub fn configure_devices(tx: mpsc::Sender<messages::Message>) -> Result<RoboticaBoard> {
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

    let (wifi, sntp) = wifi::esp::connect()?;

    Ok(RoboticaBoard {
        wifi,
        sntp,
        display,
    })
}
