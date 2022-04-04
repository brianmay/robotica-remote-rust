mod touchscreen;

use std::sync::mpsc;

use anyhow::Result;

use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use esp_idf_hal::prelude::*;
use esp_idf_svc::sntp::EspSntp;
use esp_idf_svc::wifi::EspWifi;

use crate::button::ButtonId;
use crate::display;
use crate::display::makerfab::NUM_PER_PAGE;
use crate::messages;
use crate::wifi;

use super::Board;

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

pub struct ButtonInfo {
    pub position: Rectangle,
    pub id: ButtonId,
}

pub fn configure_devices(tx: mpsc::Sender<messages::Message>) -> Result<Makerfab> {
    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let backlight = pins.gpio5.into_output().unwrap();
    // backlight.set_low().unwrap();

    let buttons: [ButtonInfo; NUM_PER_PAGE] = [
        ButtonInfo {
            position: Rectangle::new(Point::new(10, 10), Size::new(128, 64)),
            id: ButtonId::Physical(0),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(128 + 20, 10), Size::new(128, 64)),
            id: ButtonId::Physical(1),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(256 + 30, 10), Size::new(128, 64)),
            id: ButtonId::Physical(2),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(10, 64 + 20), Size::new(128, 64)),
            id: ButtonId::Physical(3),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(128 + 20, 64 + 20), Size::new(128, 64)),
            id: ButtonId::Physical(4),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(256 + 30, 64 + 20), Size::new(128, 64)),
            id: ButtonId::Physical(5),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(10, 64 * 2 + 30), Size::new(128, 64)),
            id: ButtonId::Physical(6),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(128 + 20, 64 * 2 + 30), Size::new(128, 64)),
            id: ButtonId::Physical(7),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(256 + 30, 64 * 2 + 30), Size::new(128, 64)),
            id: ButtonId::Physical(8),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(10, 64 * 3 + 40), Size::new(128, 64)),
            id: ButtonId::Physical(9),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(128 + 20, 64 * 3 + 40), Size::new(128, 64)),
            id: ButtonId::Physical(10),
        },
        ButtonInfo {
            position: Rectangle::new(Point::new(256 + 30, 64 * 3 + 40), Size::new(128, 64)),
            id: ButtonId::Physical(11),
        },
    ];

    let display = display::makerfab::connect(
        pins.gpio33,
        pins.gpio4,
        peripherals.spi2,
        pins.gpio14,
        pins.gpio13,
        pins.gpio12,
        pins.gpio15,
        backlight,
        &buttons,
    )
    .unwrap();

    let (wifi, sntp) = wifi::esp::connect()?;

    let sda = pins.gpio26.into_output().unwrap();
    let scl = pins.gpio27.into_output().unwrap();
    let i2c1 = peripherals.i2c1;
    touchscreen::connect(i2c1, sda, scl, buttons, tx);

    Ok(Makerfab {
        wifi,
        sntp,
        display,
    })
}
