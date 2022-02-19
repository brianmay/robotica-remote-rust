use std::error::Error;
use std::sync::mpsc;
use std::thread;

use display_interface::DisplayError;
use log::*;

use embedded_svc::utils::anyerror::*;

use ssd1306;
use ssd1306::mode::DisplayConfig;

use esp_idf_hal::adc;
use esp_idf_hal::delay;
use esp_idf_hal::gpio::{self, GpioPin, Input, Pins, Pull};
use esp_idf_hal::i2c;
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi;

use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
use embedded_graphics::pixelcolor::*;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::*;
use embedded_graphics::text::*;

use crate::button_controllers;

pub enum DisplayMessage {
    DisplayState(button_controllers::DisplayState, u32),
}

pub fn connect(
    i2c: i2c::I2C0,
    scl: gpio::Gpio4<gpio::Unknown>,
    sda: gpio::Gpio5<gpio::Unknown>,
) -> Result<mpsc::Sender<DisplayMessage>, Box<dyn Error>> {
    let (tx, rx) = mpsc::channel();

    let config = <i2c::config::MasterConfig as Default>::default().baudrate(400.kHz().into());
    let xxx =
        i2c::Master::<i2c::I2C0, _, _>::new(i2c, i2c::MasterPins { sda, scl }, config).unwrap();
    let bus = shared_bus::BusManagerSimple::new(xxx);

    let builder = thread::Builder::new().stack_size(8 * 1024);

    builder.spawn(move || {
        use ssd1306::Ssd1306;

        let di0 = ssd1306::I2CDisplayInterface::new_custom_address(bus.acquire_i2c(), 0x3C);
        let di1 = ssd1306::I2CDisplayInterface::new_custom_address(bus.acquire_i2c(), 0x3D);

        let mut display0 = ssd1306::Ssd1306::new(
            di0,
            ssd1306::size::DisplaySize128x64,
            ssd1306::rotation::DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();

        let mut display1 = ssd1306::Ssd1306::new(
            di1,
            ssd1306::size::DisplaySize128x64,
            ssd1306::rotation::DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();

        let z = AnyError::<display_interface::DisplayError>::wrap(|| {
            display0.init()?;
            display1.init()?;

            led_draw_number(&mut display0, 0)?;
            led_draw_number(&mut display1, 1)?;

            display0.flush()?;
            display1.flush()
        });

        match z {
            Ok(_) => {}
            Err(err) => error!("Got error {}", err),
        }

        for received in rx {
            match received {
                DisplayMessage::DisplayState(state, id) => {
                    info!("got message to display on {}", id);
                    let display = match id {
                        0 => &mut display0,
                        1 => &mut display1,
                        _ => panic!("Invalid display value received"),
                    };

                    let message = match state {
                        button_controllers::DisplayState::HardOff => "hard off",
                        button_controllers::DisplayState::Error => "error",
                        button_controllers::DisplayState::Unknown => "unknown",
                        button_controllers::DisplayState::On => "on",
                        button_controllers::DisplayState::Off => "off",
                        button_controllers::DisplayState::Auto => "auto",
                        button_controllers::DisplayState::Rainbow => "rainbow",
                    };

                    led_draw_string(display, message).unwrap();
                    display.flush().unwrap();
                }
            }
        }
    })?;

    Ok(tx)
}

fn led_draw_number<D>(display: &mut D, number: u8) -> Result<(), D::Error>
where
    D: DrawTarget<Error = DisplayError> + Dimensions,
    D::Color: From<Rgb565>,
{
    display.clear(Rgb565::BLACK.into())?;

    Rectangle::new(display.bounding_box().top_left, display.bounding_box().size)
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(Rgb565::BLUE.into())
                .stroke_color(Rgb565::YELLOW.into())
                .stroke_width(1)
                .build(),
        )
        .draw(display)?;

    let t = format!("Hello Rusty\n{}", number);

    Text::new(
        &t,
        Point::new(10, (display.bounding_box().size.height - 10) as i32 / 2),
        MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE.into()),
    )
    .draw(display)?;

    info!("LED rendering number done {}", number);

    Ok(())
}

fn led_draw_string<D>(display: &mut D, t: &str) -> Result<(), D::Error>
where
    D: DrawTarget<Error = DisplayError> + Dimensions,
    D::Color: From<Rgb565>,
{
    display.clear(Rgb565::BLACK.into())?;

    Rectangle::new(display.bounding_box().top_left, display.bounding_box().size)
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(Rgb565::BLUE.into())
                .stroke_color(Rgb565::YELLOW.into())
                .stroke_width(1)
                .build(),
        )
        .draw(display)?;

    Text::new(
        t,
        Point::new(10, (display.bounding_box().size.height - 10) as i32 / 2),
        MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE.into()),
    )
    .draw(display)?;

    info!("LED rendering string done {}", t);

    Ok(())
}
