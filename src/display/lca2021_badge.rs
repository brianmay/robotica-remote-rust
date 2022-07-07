use std::sync::mpsc;
use std::thread;

use anyhow::Result;

use embedded_graphics::prelude::Point;
use embedded_graphics::prelude::Size;
use embedded_graphics::primitives::Rectangle;
use esp_idf_hal::gpio::InputPin;
use esp_idf_hal::gpio::OutputPin;
use ssd1306;
use ssd1306::mode::BufferedGraphicsMode;
use ssd1306::mode::DisplayConfig;

use esp_idf_hal::i2c;
use esp_idf_hal::prelude::*;

use embedded_graphics::pixelcolor::*;

use super::graphics::display_thread;
use super::graphics::Button;
use super::graphics::FlushableDrawTarget;
use super::DisplayCommand;

use i2c::{Master, I2C0};
use shared_bus::BusManager;
use shared_bus::I2cProxy;
use shared_bus::NullMutex;
use ssd1306::prelude::I2CInterface;
use ssd1306::size::DisplaySize128x64;
use ssd1306::Ssd1306;

pub const NUM_PER_PAGE: usize = 2;

type SharedBus<SDA, SCL> = BusManager<NullMutex<Master<I2C0, SDA, SCL>>>;
type Bus<'a, SDA, SCL> = I2cProxy<'a, NullMutex<Master<I2C0, SDA, SCL>>>;
type Display<'a, SDA, SCL> = Ssd1306<
    I2CInterface<Bus<'a, SDA, SCL>>,
    DisplaySize128x64,
    BufferedGraphicsMode<DisplaySize128x64>,
>;

impl<SDA: InputPin + OutputPin, SCL: InputPin + OutputPin> FlushableDrawTarget
    for Display<'_, SDA, SCL>
{
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.flush()
    }

    fn set_display_on(&mut self, on: bool) -> Result<(), Self::Error> {
        self.set_display_on(on)
    }
}

fn get_bus<SDA: InputPin + OutputPin, SCL: InputPin + OutputPin>(
    i2c: i2c::I2C0,
    scl: SCL,
    sda: SDA,
) -> Result<SharedBus<SDA, SCL>> {
    let config = <i2c::config::MasterConfig as Default>::default().baudrate(400.kHz().into());
    let xxx =
        i2c::Master::<i2c::I2C0, _, _>::new(i2c, i2c::MasterPins { sda, scl }, config).unwrap();
    let bus: SharedBus<SDA, SCL> = shared_bus::BusManagerSimple::new(xxx);

    Ok(bus)
}

// This clippy warning is false, lifetimes are required here.
#[allow(clippy::needless_lifetimes)]
fn get_display<'a>(
    bus: Bus<'a, impl InputPin + OutputPin, impl InputPin + OutputPin>,
    address: u8,
) -> Result<
    impl FlushableDrawTarget<
            Error = impl std::fmt::Debug,
            Color = impl PixelColor + From<Gray8> + From<Rgb555> + From<Rgb888>,
        > + 'a,
> {
    let di = ssd1306::I2CDisplayInterface::new_custom_address(bus, address);

    let mut display = ssd1306::Ssd1306::new(
        di,
        ssd1306::size::DisplaySize128x64,
        ssd1306::rotation::DisplayRotation::Rotate0,
    )
    .into_buffered_graphics_mode();

    display.init().unwrap();

    Ok(display)
}

pub const NUM_DISPLAYS: usize = 2;

pub fn connect(
    i2c: i2c::I2C0,
    scl: impl InputPin + OutputPin + 'static,
    sda: impl InputPin + OutputPin + 'static,
) -> Result<mpsc::Sender<DisplayCommand>> {
    let (tx, rx) = mpsc::channel();

    let bus = get_bus(i2c, scl, sda).unwrap();
    let builder = thread::Builder::new().stack_size(8 * 1024);

    builder.spawn(move || {
        let display0 = get_display(bus.acquire_i2c(), 0x3C).unwrap();
        let display1 = get_display(bus.acquire_i2c(), 0x3D).unwrap();
        let mut displays: [_; NUM_DISPLAYS] = [display0, display1];
        let buttons: [_; NUM_PER_PAGE] = [
            Button::new(0, Rectangle::new(Point::new(0, 0), Size::new(128, 64))),
            Button::new(1, Rectangle::new(Point::new(0, 0), Size::new(128, 64))),
        ];

        display_thread::<_, NUM_PER_PAGE, NUM_DISPLAYS>(&mut displays, &buttons, rx);
    })?;

    Ok(tx)
}
