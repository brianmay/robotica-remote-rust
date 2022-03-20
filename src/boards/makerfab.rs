use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Result;

use esp_idf_hal::i2c;
use esp_idf_hal::prelude::FromValueType;
use esp_idf_hal::prelude::Peripherals;

use esp_idf_svc::sntp::EspSntp;
use esp_idf_svc::wifi::EspWifi;
use ft6x36::Ft6x36;

use crate::display;
use crate::messages;
use crate::wifi;

use super::Board;

use log::*;

pub const NUM_CONTROLLERS_PER_PAGE: usize = display::makerfab::NUM_PER_PAGE;

#[allow(dead_code)]
pub struct Makerfab {
    wifi: EspWifi,
    sntp: EspSntp,
    display: mpsc::Sender<display::DisplayCommand>,
    // touch_screen: Ft6x36<EspI2c1>,
}

impl Board for Makerfab {
    fn get_display(&self) -> mpsc::Sender<display::DisplayCommand> {
        self.display.clone()
    }
}

pub fn configure_devices(_tx: mpsc::Sender<messages::Message>) -> Result<Makerfab> {
    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let backlight = pins.gpio5.into_output().unwrap();
    // backlight.set_low().unwrap();

    let display = display::makerfab::connect(
        pins.gpio33,
        pins.gpio4,
        peripherals.spi2,
        pins.gpio14,
        pins.gpio13,
        pins.gpio12,
        pins.gpio15,
        backlight,
    )
    .unwrap();

    let (wifi, sntp) = wifi::esp::connect()?;

    let sda = pins.gpio26.into_output().unwrap();
    let scl = pins.gpio27.into_output().unwrap();
    let config = <i2c::config::MasterConfig as Default>::default().baudrate(400_u32.kHz().into());
    let i2c1 =
        i2c::Master::<i2c::I2C1, _, _>::new(peripherals.i2c1, i2c::MasterPins { sda, scl }, config)
            .unwrap();

    let mut touch_screen = Ft6x36::new(i2c1);

    touch_screen.init().unwrap();
    match touch_screen.get_info() {
        Some(info) => info!("Touch screen info: {info:?}"),
        None => warn!("No info"),
    }

    thread::spawn(move || loop {
        let x = touch_screen.get_touch_event().unwrap();
        println!("{x:?}");
        thread::sleep(Duration::from_millis(500));
    });

    Ok(Makerfab {
        wifi,
        sntp,
        display,
    })
}
