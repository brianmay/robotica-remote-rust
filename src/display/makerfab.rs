use super::graphics::FlushableDrawTarget;
use super::DisplayCommand;
use crate::boards::makerfab::ButtonInfo;
use crate::display::graphics::display_thread;
use crate::display::graphics::Button;
use anyhow::Result;
use display_interface::DisplayError;
use display_interface_spi::SPIInterface;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_hal::delay::blocking::DelayUs;
use embedded_hal::digital::blocking::OutputPin;
use embedded_hal::spi::MODE_0;
use esp_idf_hal::delay;
use esp_idf_hal::gpio;
use esp_idf_hal::gpio::Gpio12;
use esp_idf_hal::gpio::Gpio13;
use esp_idf_hal::gpio::Gpio14;
use esp_idf_hal::gpio::Gpio15;
use esp_idf_hal::gpio::Gpio21;
use esp_idf_hal::gpio::Gpio33;
use esp_idf_hal::gpio::Gpio4;
use esp_idf_hal::gpio::Output;
use esp_idf_hal::gpio::Unknown;
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi;
use esp_idf_hal::spi::Master;
use esp_idf_hal::spi::SPI2;
use log::info;
use mipidsi::instruction::Instruction;
use mipidsi::models::write_command;
use mipidsi::models::ILI9486Rgb666;
use std::sync::mpsc;
use std::thread;

pub const NUM_PER_PAGE: usize = 8;
pub const NUM_DISPLAYS: usize = 1;

type SpiInterface = SPIInterface<
    Master<SPI2, Gpio14<Unknown>, Gpio13<Unknown>, Gpio12<Unknown>, Gpio21<Unknown>>,
    Gpio33<Output>,
    Gpio15<Output>,
>;
type OrigDisplay = mipidsi::Display<SpiInterface, Gpio4<Output>, ILI9486Rgb666>;

struct Display<BL>(OrigDisplay, BL);

impl<BL> OriginDimensions for Display<BL> {
    fn size(&self) -> Size {
        self.0.size()
    }
}

impl<BL> DrawTarget for Display<BL> {
    type Color = <OrigDisplay as DrawTarget>::Color;
    type Error = <OrigDisplay as DrawTarget>::Error;

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

impl<BL: OutputPin> FlushableDrawTarget for Display<BL> {
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
    dc: gpio::Gpio33<gpio::Unknown>,
    rst: gpio::Gpio4<gpio::Unknown>,
    spi: spi::SPI2,
    sclk: gpio::Gpio14<gpio::Unknown>,
    sdo: gpio::Gpio13<gpio::Unknown>,
    sdi: gpio::Gpio12<gpio::Unknown>,
    cs: gpio::Gpio15<gpio::Unknown>,
    bl: impl OutputPin + Send + 'static,
    buttons: &[ButtonInfo; NUM_PER_PAGE],
) -> Result<mpsc::Sender<DisplayCommand>> {
    let (tx, rx) = mpsc::channel();

    info!("About to initialize the SPI LED driver");

    let config = <spi::config::Config as Default>::default()
        .baudrate((60).MHz().into())
        .data_mode(MODE_0);

    let mut cs = cs.into_output()?;
    cs.set_high()?;

    let mut reset = rst.into_output()?;
    reset.set_high()?;

    let mut dc = dc.into_output()?;
    dc.set_high()?;

    // let (sdi, sdo) = (sdo, sdi);
    let pins = spi::Pins {
        sclk,
        sdo,
        sdi: Some(sdi),
        // cs: Some(cs),
        cs: Option::<gpio::Gpio21<gpio::Unknown>>::None,
    };

    let spi = spi::Master::<spi::SPI2, _, _, _, _>::new(spi, pins, config)?;
    let mut di = SPIInterface::new(spi, dc.into_output()?, cs);

    let mut x = || -> Result<(), DisplayError> {
        write_command(&mut di, Instruction::SLPOUT, &[])?; // turn off sleep
        write_command(&mut di, Instruction::COLMOD, &[0b0110_0110])?; // 18bit 256k colors
        write_command(&mut di, Instruction::MADCTL, &[0b0000_0000])?; // left -> right, bottom -> top RGB
        write_command(&mut di, Instruction::VCMOFSET, &[0x00, 0x48, 0x00, 0x48])?; //VCOM  Control 1 [00 40 00 40]
        write_command(&mut di, Instruction::INVCO, &[0x0])?; //Inversion Control [00]

        // optional gamma setup
        // write_command(di, Instruction::PGC, &[0x00, 0x2C, 0x2C, 0x0B, 0x0C, 0x04, 0x4C, 0x64, 0x36, 0x03, 0x0E, 0x01, 0x10, 0x01, 0x00])?; // Positive Gamma Control
        // write_command(di, Instruction::NGC, &[0x0F, 0x37, 0x37, 0x0C, 0x0F, 0x05, 0x50, 0x32, 0x36, 0x04, 0x0B, 0x00, 0x19, 0x14, 0x0F])?; // Negative Gamma Control

        write_command(&mut di, Instruction::DFC, &[0b0000_0010, 0x02, 0x3B])?;
        write_command(&mut di, Instruction::NORON, &[])?; // turn to normal mode
        write_command(&mut di, Instruction::DISPON, &[])?; // turn on display
                                                           // write_command(&mut di, Instruction::BRIGHTNESS, &[0x00])?; // turn on display

        Ok(())
    };
    x().unwrap();

    // DISPON requires some time otherwise we risk SPI data issues
    delay::Ets.delay_us(120_000).unwrap();

    let mut display = mipidsi::Display::ili9486_rgb666(di, reset);

    display
        .set_orientation(mipidsi::Orientation::Landscape, true, false)
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
    ];

    let builder = thread::Builder::new().stack_size(8 * 1024);
    builder.spawn(move || {
        let mut displays: [_; NUM_DISPLAYS] = [display];

        display_thread::<_, NUM_PER_PAGE, NUM_DISPLAYS>(&mut displays, &buttons, rx);
    })?;

    Ok(tx)
}
