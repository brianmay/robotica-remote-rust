use super::graphics::FlushableDrawTarget;
use super::DisplayCommand;
use crate::boards::makerfab::ButtonInfo;
use crate::display::graphics::display_thread;
use crate::display::graphics::Button;
use anyhow::Result;
use display_interface_spi::SPIInterface;
// use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_hal::digital::OutputPin;
// use embedded_hal::digital::blocking::OutputPin;
// use embedded_hal::spi::MODE_0;
use esp_idf_hal::delay;
use esp_idf_hal::gpio;
// use esp_idf_hal::gpio::Gpio12;
// use esp_idf_hal::gpio::Gpio13;
// use esp_idf_hal::gpio::Gpio14;
use esp_idf_hal::gpio::Gpio15;
// use esp_idf_hal::gpio::Gpio21;
use esp_idf_hal::gpio::Gpio33;
use esp_idf_hal::gpio::Gpio4;
use esp_idf_hal::gpio::Output;
use esp_idf_hal::gpio::PinDriver;
// use esp_idf_hal::gpio::Output;
// use esp_idf_hal::gpio::Unknown;
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi;
// use esp_idf_hal::spi::Master;
use esp_idf_hal::spi::SpiDeviceDriver;
use esp_idf_hal::spi::SpiDriver;
// use esp_idf_hal::spi::SPI2;
use log::info;
use mipidsi::models::ILI9486Rgb666;
use mipidsi::Builder;
use mipidsi::ColorOrder;
use mipidsi::Orientation;
use std::sync::mpsc;
use std::thread;

pub const NUM_PER_PAGE: usize = 12;
pub const NUM_DISPLAYS: usize = 1;

type SpiInterface<'a> = SPIInterface<
    SpiDeviceDriver<'a, SpiDriver<'a>>,
    PinDriver<'a, Gpio33, Output>,
    PinDriver<'a, Gpio15, Output>,
>;

type OrigDisplay<'a> =
    mipidsi::Display<SpiInterface<'a>, ILI9486Rgb666, PinDriver<'a, Gpio4, Output>>;

struct Display<'a, BL>(OrigDisplay<'a>, BL);

impl<'a, BL> OriginDimensions for Display<'a, BL> {
    fn size(&self) -> Size {
        self.0.size()
    }
}

impl<'a, BL> DrawTarget for Display<'a, BL> {
    type Color = <OrigDisplay<'a> as DrawTarget>::Color;
    type Error = <OrigDisplay<'a> as DrawTarget>::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.0.draw_iter(pixels)
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        self.0.fill_contiguous(area, colors)
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        self.0.fill_solid(area, color)
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.0.clear(color)
    }
}

impl<'a, BL: OutputPin> FlushableDrawTarget for Display<'a, BL> {
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_display_on(&mut self, on: bool) -> Result<(), Self::Error> {
        if on {
            self.1.set_high().unwrap();
        } else {
            self.1.set_low().unwrap();
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn connect(
    dc: gpio::Gpio33,
    reset: gpio::Gpio4,
    spi: spi::SPI2,
    sclk: gpio::Gpio14,
    sdo: gpio::Gpio13,
    _sdi: gpio::Gpio12,
    cs: gpio::Gpio15,
    bl: impl OutputPin + Send + 'static,
    buttons: &[ButtonInfo; NUM_PER_PAGE],
) -> Result<mpsc::Sender<DisplayCommand>> {
    let (tx, rx) = mpsc::channel();

    info!("About to initialize the SPI LED driver");

    // let config = <spi::config::Config as Default>::default()
    //     .baudrate((60).MHz().into())
    //     .data_mode(MODE_0);

    let mut cs = gpio::PinDriver::output(cs)?;
    cs.set_high()?;

    let mut reset = gpio::PinDriver::output(reset)?;
    reset.set_high()?;

    let mut dc = gpio::PinDriver::output(dc)?;
    dc.set_high()?;

    // let (sdi, sdo) = (sdo, sdi);
    // let pins = spi::Pins {
    //     sclk,
    //     sdo,
    //     sdi: Some(sdi),
    //     // cs: Some(cs),
    //     cs: Option::<gpio::Gpio21>::None,
    // };

    // let spi = spi::Master::<spi::SPI2, _, _, _, _>::new(spi, pins, config)?;
    // let di = SPIInterface::new(spi, dc.into_output()?, cs);

    let di = SPIInterface::new(
        spi::SpiDeviceDriver::new_single(
            spi,
            sclk,
            sdo,
            Option::<gpio::AnyIOPin>::None,
            spi::Dma::Disabled,
            Option::<gpio::Gpio21>::None,
            &spi::SpiConfig::new().baudrate(60.MHz().into()),
        )?,
        dc,
        cs,
    );

    let display = Builder::ili9486_rgb666(di)
        // .with_size(Size::new(320, 480))
        .with_orientation(Orientation::Landscape(false))
        .with_color_order(ColorOrder::Bgr)
        .init(&mut delay::Ets, Some(reset))
        .unwrap();

    let display = Display(display, bl);

    let bounding_box = display.bounding_box();
    println!("sssssssssssss {:?}", bounding_box);

    let buttons: [_; NUM_PER_PAGE] = [
        Button::new(0, buttons[0].position),
        Button::new(0, buttons[1].position),
        Button::new(0, buttons[2].position),
        Button::new(0, buttons[3].position),
        Button::new(0, buttons[4].position),
        Button::new(0, buttons[5].position),
        Button::new(0, buttons[6].position),
        Button::new(0, buttons[7].position),
        Button::new(0, buttons[8].position),
        Button::new(0, buttons[9].position),
        Button::new(0, buttons[10].position),
        Button::new(0, buttons[11].position),
    ];

    let builder = thread::Builder::new().stack_size(8 * 1024);
    builder.spawn(move || {
        let mut displays: [_; NUM_DISPLAYS] = [display];

        display_thread::<_, NUM_PER_PAGE, NUM_DISPLAYS>(&mut displays, &buttons, rx);
    })?;

    Ok(tx)
}
