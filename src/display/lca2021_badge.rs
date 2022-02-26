use std::sync::mpsc;
use std::thread;

use anyhow::Result;

use display_interface::DisplayError;

use ssd1306;
use ssd1306::mode::DisplayConfig;

use esp_idf_hal::gpio;
use esp_idf_hal::i2c;
use esp_idf_hal::prelude::*;

use embedded_graphics::geometry::Point;
use embedded_graphics::image::*;
use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
use embedded_graphics::pixelcolor::*;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::*;
use embedded_graphics::text::*;

use tinytga::DynamicTga;

use crate::button_controllers;
use crate::button_controllers::DisplayState;
use crate::button_controllers::Icon;
use crate::messages::Message;
use crate::messages::Sender;

use crate::display::DisplayCommand;

// pub fn get_displays<D>(
//     i2c: i2c::I2C0,
//     scl: gpio::Gpio4<gpio::Unknown>,
//     sda: gpio::Gpio5<gpio::Unknown>,
// ) -> Result<D>
// where
//     D: From<ssd1306::Ssd1306>
//     // D: DrawTarget<Error = DisplayError, Color = BinaryColor> + Dimensions,
//     // D::Color: From<Rgb565>,
// {
//     let config = <i2c::config::MasterConfig as Default>::default().baudrate(400.kHz().into());
//     let xxx =
//         i2c::Master::<i2c::I2C0, _, _>::new(i2c, i2c::MasterPins { sda, scl }, config).unwrap();
//     let bus = shared_bus::BusManagerSimple::new(xxx);

//     let di = ssd1306::I2CDisplayInterface::new_custom_address(bus.acquire_i2c(), 0x3C);

//     let mut display = ssd1306::Ssd1306::new(
//         di,
//         ssd1306::size::DisplaySize128x64,
//         ssd1306::rotation::DisplayRotation::Rotate0,
//     )
//     .into_buffered_graphics_mode();

//     Ok(display)
// }

struct Page {
    state: DisplayState,
    icon: Icon,
}

pub const NUM_COLUMNS: u32 = 2;
pub const NUM_PAGES: u32 = 4;

pub fn connect(
    i2c: i2c::I2C0,
    scl: gpio::Gpio4<gpio::Unknown>,
    sda: gpio::Gpio5<gpio::Unknown>,
    tx_main: Sender,
) -> Result<mpsc::Sender<DisplayCommand>> {
    let (tx, rx) = mpsc::channel();
    let mut pages: [[Option<Page>; NUM_PAGES as usize]; NUM_COLUMNS as usize] = Default::default();
    let mut page_number: usize = 0;
    let mut blanked = false;

    let config = <i2c::config::MasterConfig as Default>::default().baudrate(400.kHz().into());
    let xxx =
        i2c::Master::<i2c::I2C0, _, _>::new(i2c, i2c::MasterPins { sda, scl }, config).unwrap();
    let bus = shared_bus::BusManagerSimple::new(xxx);

    let builder = thread::Builder::new().stack_size(8 * 1024);

    tx_main
        .send(Message::DisplayPage(page_number as u32))
        .unwrap();

    builder.spawn(move || {
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

        display0.init().unwrap();
        display1.init().unwrap();

        led_draw_loading(&mut display0);
        led_draw_loading(&mut display1);

        display0.flush().unwrap();
        display1.flush().unwrap();

        for received in rx {
            match received {
                DisplayCommand::DisplayState(state, icon, id) => {
                    let column: usize = (id % NUM_COLUMNS) as usize;
                    let page = Page { state, icon };
                    let number: usize = (id / NUM_COLUMNS) as usize;
                    pages[column][number] = Some(page);
                }
                DisplayCommand::BlankAll => {
                    blanked = true;
                }
                DisplayCommand::UnBlankAll => {
                    blanked = false;
                }
                DisplayCommand::PageUp => {
                    if page_number + 1 < NUM_PAGES as usize {
                        page_number += 1
                    };
                    tx_main
                        .send(Message::DisplayPage(page_number as u32))
                        .unwrap();
                }
                DisplayCommand::PageDown => {
                    if page_number > 0 {
                        page_number -= 1
                    };
                    tx_main
                        .send(Message::DisplayPage(page_number as u32))
                        .unwrap();
                }
            }

            let number = page_number * NUM_COLUMNS as usize;
            page_draw(&mut display0, &pages[0][page_number], number, blanked);
            display0.flush().unwrap();

            let number = number + 1;
            page_draw(&mut display1, &pages[1][page_number], number, blanked);
            display1.flush().unwrap();
        }
    })?;

    Ok(tx)
}

fn page_draw<D>(display: &mut D, page_or_none: &Option<Page>, number: usize, blanked: bool)
where
    D: DrawTarget<Error = DisplayError, Color = BinaryColor> + Dimensions,
    D::Color: From<Rgb565>,
{
    if blanked {
        led_clear(display);
    } else if let Some(page) = page_or_none {
        let image_category = get_image_category(&page.state);
        let image_data = get_image_data(&image_category, &page.icon);
        led_clear(display);
        led_draw_image(display, image_data);
        led_draw_overlay(display, &page.state);
        led_draw_number(display, number);
    } else {
        led_clear(display);
        led_draw_number(display, number);
    }
}

fn led_clear<D>(display: &mut D)
where
    D: DrawTarget<Error = DisplayError> + Dimensions,
    D::Color: From<Rgb565>,
{
    display.clear(Rgb565::BLACK.into()).unwrap();
}

fn led_draw_loading<D>(display: &mut D)
where
    D: DrawTarget<Error = DisplayError> + Dimensions,
    D::Color: From<Rgb565>,
{
    Rectangle::new(display.bounding_box().top_left, display.bounding_box().size)
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(Rgb565::BLUE.into())
                .stroke_color(Rgb565::YELLOW.into())
                .stroke_width(1)
                .build(),
        )
        .draw(display)
        .unwrap();

    let t = "Loading";

    Text::new(
        t,
        Point::new(10, (display.bounding_box().size.height - 10) as i32 / 2),
        MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE.into()),
    )
    .draw(display)
    .unwrap();
}

fn led_draw_number<D>(display: &mut D, number: usize)
where
    D: DrawTarget<Error = DisplayError> + Dimensions,
    D::Color: From<Rgb565>,
{
    let t = format!("{}", number);

    Text::new(
        &t,
        Point::new(0, 14),
        MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE.into()),
    )
    .draw(display)
    .unwrap();
}

enum ImageCategory {
    HardOff,
    On,
    OnOther,
    Off,
}

fn get_image_category(state: &DisplayState) -> ImageCategory {
    match state {
        DisplayState::HardOff => ImageCategory::HardOff,
        DisplayState::Error => ImageCategory::Off,
        DisplayState::Unknown => ImageCategory::Off,
        DisplayState::On => ImageCategory::On,
        DisplayState::Off => ImageCategory::Off,
        DisplayState::OnOther => ImageCategory::OnOther,
    }
}

fn get_image_data<'a>(
    image: &ImageCategory,
    icon: &button_controllers::Icon,
) -> DynamicTga<'a, BinaryColor> {
    let data = match icon {
        button_controllers::Icon::Light => match image {
            ImageCategory::HardOff => include_bytes!("images/light_hard_off_64x64.tga").as_slice(),
            ImageCategory::On => include_bytes!("images/light_on_64x64.tga").as_slice(),
            ImageCategory::Off => include_bytes!("images/light_off_64x64.tga").as_slice(),
            ImageCategory::OnOther => include_bytes!("images/light_on_other_64x64.tga").as_slice(),
        },
        button_controllers::Icon::Fan => match image {
            ImageCategory::HardOff => include_bytes!("images/fan_hard_off_64x64.tga").as_slice(),
            ImageCategory::On => include_bytes!("images/fan_on_64x64.tga").as_slice(),
            ImageCategory::Off => include_bytes!("images/fan_off_64x64.tga").as_slice(),
            ImageCategory::OnOther => include_bytes!("images/fan_on_other_64x64.tga").as_slice(),
        },
    };

    DynamicTga::from_slice(data).unwrap()
}

fn led_draw_image<D>(display: &mut D, tga: DynamicTga<BinaryColor>)
where
    D: DrawTarget<Error = DisplayError, Color = BinaryColor> + Dimensions,
    D::Color: From<Rgb565>,
{
    let size = tga.size();
    let display_size = display.bounding_box();
    let center = display_size.center();

    let x = center.x - size.width as i32 / 2;
    let y = center.y - size.height as i32 / 2;

    Image::new(&tga, Point::new(x, y)).draw(display).unwrap();
}

fn led_draw_overlay<D>(display: &mut D, state: &DisplayState)
where
    D: DrawTarget<Error = DisplayError, Color = BinaryColor> + Dimensions,
    D::Color: From<Rgb565>,
{
    let display_size = display.bounding_box();

    let text = match state {
        DisplayState::HardOff => "Hard off",
        DisplayState::Error => "Error",
        DisplayState::Unknown => "Lost",
        DisplayState::On => "On",
        DisplayState::Off => "Off",
        DisplayState::OnOther => "Other",
    };

    if matches!(state, DisplayState::Error | DisplayState::Unknown) {
        let center = display_size.center();
        let size = Size::new(60, 24);

        let x = center.x - size.width as i32 / 2;
        let y = display_size.bottom_right().unwrap().y - 30;
        let ul = Point::new(x, y);

        Rectangle::new(ul, size)
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .fill_color(Rgb565::BLACK.into())
                    .stroke_color(Rgb565::WHITE.into())
                    .stroke_width(1)
                    .build(),
            )
            .draw(display)
            .unwrap();

        Text::with_alignment(
            text,
            Point::new(center.x, y + 17),
            MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE.into()),
            Alignment::Center,
        )
        .draw(display)
        .unwrap();
    }
}
