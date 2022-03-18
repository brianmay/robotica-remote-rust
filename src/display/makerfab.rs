use super::graphics::FlushableDrawTarget;
use super::DisplayCommand;
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
use esp_idf_hal::gpio::Gpio26;
use esp_idf_hal::gpio::Gpio33;
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

pub const NUM_PER_PAGE: usize = 4;
pub const NUM_DISPLAYS: usize = 1;

// #[derive(Clone)]
// struct Rgb666(pixelcolor::Rgb666);

// impl From<pixelcolor::Rgb666> for Rgb666 {
//     fn from(src: pixelcolor::Rgb666) -> Self {
//         Rgb666(src)
//     }
// }

// impl From<pixelcolor::Rgb565> for Rgb666 {
//     fn from(src: pixelcolor::Rgb565) -> Self {
//         Rgb666(pixelcolor::Rgb666::new(src.r(), src.g(), src.b()))
//     }
// }

// impl From<pixelcolor::Gray8> for Rgb666 {
//     fn from(src: pixelcolor::Gray8) -> Self {
//         Rgb666(pixelcolor::Rgb666::new(src.luma(), src.luma(), src.luma()))
//     }
// }

// impl From<pixelcolor::Rgb888> for Rgb666 {
//     fn from(src: pixelcolor::Rgb888) -> Self {
//         Rgb666(pixelcolor::Rgb666::new(src.r(), src.g(), src.b()))
//     }
// }
// impl Copy for Rgb666 {

// }
// impl PartialEq for Rgb666 {
//     fn eq(&self, other: &Self) -> bool {
//         self.0 == other.0
//     }
// }

// impl PixelColor for Rgb666 {
//     type Raw = RawU18;
// }

// impl RgbColor for Rgb666 {
//     fn r(&self) -> u8 {
//         self.0.r()
//     }

//     fn g(&self) -> u8 {
//         self.0.g()
//     }

//     fn b(&self) -> u8 {
//         self.0.b()
//     }

//     const MAX_R: u8 = pixelcolor::Rgb666::MAX_R;

//     const MAX_G: u8 = pixelcolor::Rgb666::MAX_G;

//     const MAX_B: u8 =  Rgb666::MAX_B;

//     const BLACK: Self = Rgb666(pixelcolor::Rgb666::BLACK);

//     const RED: Self = Rgb666(pixelcolor::Rgb666::RED);

//     const GREEN: Self = Rgb666(pixelcolor::Rgb666::GREEN);

//     const BLUE: Self = Rgb666(pixelcolor::Rgb666::BLUE);

//     const YELLOW: Self = Rgb666(pixelcolor::Rgb666::YELLOW);

//     const MAGENTA: Self = Rgb666(pixelcolor::Rgb666::MAGENTA);

//     const CYAN: Self = Rgb666(pixelcolor::Rgb666::CYAN);

//     const WHITE: Self = Rgb666(pixelcolor::Rgb666::WHITE);
// }

type SpiInterface = SPIInterface<
    Master<SPI2, Gpio14<Unknown>, Gpio13<Unknown>, Gpio12<Unknown>, Gpio21<Unknown>>,
    Gpio33<Output>,
    Gpio15<Output>,
>;
type OrigDisplay = mipidsi::Display<SpiInterface, Gpio26<Output>, ILI9486Rgb666>;

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
        self.fill_contiguous(area, core::iter::repeat(color))
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

// fn get_bus<SDA: InputPin + OutputPin, SCL: OutputPin>(
//     i2c: i2c::I2C0,
//     scl: SCL,
//     sda: SDA,
// ) -> Result<SharedBus<SDA, SCL>> {
//     let config = <i2c::config::MasterConfig as Default>::default().baudrate(400.kHz().into());
//     let xxx =
//         i2c::Master::<i2c::I2C0, _, _>::new(i2c, i2c::MasterPins { sda, scl }, config).unwrap();
//     let bus: SharedBus<SDA, SCL> = shared_bus::BusManagerSimple::new(xxx);

//     Ok(bus)
// }

// // This clippy warning is false, lifetimes are required here.
// #[allow(clippy::needless_lifetimes)]
// fn get_display<'a>(
//     bus: Bus<'a, impl InputPin + OutputPin, impl OutputPin>,
//     address: u8,
// ) -> Result<
//     impl FlushableDrawTarget<
//             Error = impl std::fmt::Debug,
//             Color = impl PixelColor + From<Gray8> + From<Rgb555> + From<Rgb888>,
//         > + 'a,
// > {
//     let di = ili9341::I2CDisplayInterface::new_custom_address(bus, address);

//     let mut display = ili9341::ili9341::new(
//         di,
//         ili9341::size::DisplaySize128x64,
//         ili9341::rotation::DisplayRotation::Rotate0,
//     )
//     .into_buffered_graphics_mode();

//     display.init().unwrap();

//     Ok(display)
// }

// pub fn connect(
//     i2c: i2c::I2C0,
//     scl: impl OutputPin + 'static,
//     sda: impl InputPin + OutputPin + 'static,
//     tx_main: Sender,
// ) -> Result<mpsc::Sender<DisplayCommand>> {
//     let (tx, rx) = mpsc::channel();

//     // let bus = get_bus(i2c, scl, sda).unwrap();
//     let builder = thread::Builder::new().stack_size(8 * 1024);

//     builder.spawn(move || {
//         // let display0 = get_display(bus.acquire_i2c(), 0x3C).unwrap();
//         // let display1 = get_display(bus.acquire_i2c(), 0x3D).unwrap();
//         // let mut displays: [_; NUM_DISPLAYS] = [display0, display1];
//         // display_thread::<_, NUM_PAGES, NUM_DISPLAYS>(tx_main, &mut displays, rx);
//     })?;

//     Ok(tx)
// }

pub fn connect(
    dc: gpio::Gpio33<gpio::Unknown>,
    rst: gpio::Gpio26<gpio::Unknown>,
    spi: spi::SPI2,
    sclk: gpio::Gpio14<gpio::Unknown>,
    sdo: gpio::Gpio13<gpio::Unknown>,
    sdi: gpio::Gpio12<gpio::Unknown>,
    cs: gpio::Gpio15<gpio::Unknown>,
    bl: impl OutputPin + Send + 'static,
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

    // reset.set_low()?;
    // delay::Ets.delay_ms(500)?;
    // reset.set_high()?;
    // delay::Ets.delay_ms(500)?;

    // cs.set_low()?;
    // dc.set_low()?;
    // spi.write(&[0x01])?;
    // delay::Ets.delay_ms(200)?;

    // cs.set_low()?;
    // dc.set_low()?;
    // spi.write(&[0x3a])?;
    // dc.set_high()?;
    // spi.write(&[0x66])?;
    // delay::Ets.delay_ms(200)?;

    // cs.set_low()?;
    // dc.set_low()?;
    // spi.write(&[0x0C])?;
    // delay::Ets.delay_ms(200)?;

    // cs.set_low()?;
    // dc.set_high()?;
    // let mut buffer: [u8; 5] = [0xff; 5];
    // println!("------> {}", hex::encode(buffer));
    // spi.read(&mut buffer)?;
    // println!("------> {}", hex::encode(buffer));

    // cs.set_low()?;
    // dc.set_low()?;
    // spi.write(&[0x29])?;
    // delay::Ets.delay_ms(100)?;

    // cs.set_low()?;
    // dc.set_low()?;
    // spi.write(&[0x11])?;
    // delay::Ets.delay_ms(100)?;

    // cs.set_low()?;
    // dc.set_low()?;
    // spi.write(&[0x2C])?;
    // dc.set_high()?;
    // spi.write(&[0x00])?;
    // for _i in 1..480 * 960 {
    //     spi.write(&[0xFF])?;
    //     spi.write(&[0x00])?;
    //     spi.write(&[0x00])?;
    // }

    // cs.set_high()?;
    // dc.set_low()?;

    let mut di = SPIInterface::new(spi, dc.into_output()?, cs);

    // reset.set_low()?;
    // delay::Ets.delay_ms(500)?;
    // reset.set_high()?;
    // delay::Ets.delay_ms(500)?;

    // di.send_commands(DataFormat::U8(&[0x01])).unwrap();
    // delay::Ets.delay_ms(200)?;

    // ! di.send_commands(DataFormat::U8(&[0x3a])).unwrap();
    // ! di.send_data(DataFormat::U8(&[0x66])).unwrap();
    // delay::Ets.delay_ms(200)?;

    // ! di.send_commands(DataFormat::U8(&[0x29])).unwrap();
    // delay::Ets.delay_ms(100)?;

    // ! di.send_commands(DataFormat::U8(&[0x11])).unwrap();
    // delay::Ets.delay_ms(100)?;

    // ! di.send_commands(DataFormat::U8(&[0x36])).unwrap();
    // ! di.send_data(DataFormat::U8(&[40])).unwrap();

    // di.send_commands(DataFormat::U8(&[0x2C])).unwrap();
    // // di.send_data(DataFormat::U8(&[0x00])).unwrap();
    // for _i in 1..480 * 960 {
    //     di.send_data(DataFormat::U8(&[0xFF, 0x00, 0x00])).unwrap();
    // }
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

    let display = mipidsi::Display::ili9486_rgb666(di, reset);
    let display = Display(display, bl);

    // let mut display = ILI9486Rgb666::new();
    // display.init(&mut di, &mut Some(reset), &mut delay::Ets);

    // display
    //     .set_orientation(mipidsi::Orientation::LandscapeSwapped)
    //     .unwrap();

    // let mut display = ili9341::Ili9341::new(
    //     di,
    //     reset,
    //     &mut delay::Ets,
    //     Orientation::Portrait,
    //     ili9341::DisplaySize240x320,
    // )
    // .unwrap();

    // display.clear(Rgb666::GREEN).unwrap();
    // let on = PrimitiveStyle::with_fill(Rgb666::RED);

    // Rectangle::new(Point::new(10, 20), Size::new(10, 10))
    //     .into_styled(on)
    //     .draw(&mut display)
    //     .unwrap();

    // led_draw(&mut display).unwrap();
    let bounding_box = display.bounding_box();
    println!("sssssssssssss {:?}", bounding_box);

    let builder = thread::Builder::new().stack_size(8 * 1024);
    builder.spawn(move || {
        let mut displays: [_; NUM_DISPLAYS] = [display];
        let buttons: [_; NUM_PER_PAGE] = [
            Button::new(0, Rectangle::new(Point::new(0, 0), Size::new(128, 64))),
            Button::new(0, Rectangle::new(Point::new(128, 0), Size::new(128, 64))),
            Button::new(0, Rectangle::new(Point::new(0, 128), Size::new(128, 64))),
            Button::new(0, Rectangle::new(Point::new(128, 128), Size::new(128, 64))),
        ];

        display_thread::<_, NUM_PER_PAGE, NUM_DISPLAYS>(&mut displays, &buttons, rx);
    })?;

    Ok(tx)
}

// Kaluga needs customized screen orientation commands
// (not a surprise; quite a few ILI9341 boards need these as evidenced in the TFT_eSPI & lvgl ESP32 C drivers)
// pub enum KalugaOrientation {
//     Portrait,
//     PortraitFlipped,
//     Landscape,
//     LandscapeFlipped,
// }

// impl ili9341::Mode for KalugaOrientation {
//     fn mode(&self) -> u8 {
//         match self {
//             Self::Portrait => 0,
//             Self::Landscape => 0x20 | 0x40,
//             Self::PortraitFlipped => 0x80 | 0x40,
//             Self::LandscapeFlipped => 0x80 | 0x20,
//         }
//     }

//     fn is_landscape(&self) -> bool {
//         matches!(self, Self::Landscape | Self::LandscapeFlipped)
//     }
// }

// fn led_draw<D>(display: &mut D) -> Result<(), D::Error>
// where
//     D: DrawTarget + Dimensions,
//     D::Color: From<mipidsi::models::Rgb666>,
// {
//     // display.clear(Rgb666::BLACK.into())?;

//     // Rectangle::new(display.bounding_box().top_left, display.bounding_box().size)
//     //     .into_styled(
//     //         PrimitiveStyleBuilder::new()
//     //             .fill_color(Rgb666::BLUE.into())
//     //             .stroke_color(Rgb666::YELLOW.into())
//     //             .stroke_width(1)
//     //             .build(),
//     //     )
//     //     .draw(display)?;

//     Text::new(
//         "Hello Rust!",
//         Point::new(10, (display.bounding_box().size.height - 10) as i32 / 2),
//         MonoTextStyle::new(&FONT_10X20, Rgb666::WHITE.into()),
//     )
//     .draw(display)?;

//     info!("LED rendering done");

//     Ok(())
// }
