use std::sync::mpsc;
use std::thread;

use log::*;

use anyhow::Result;

use embedded_graphics::mono_font::ascii::FONT_5X8;
use ssd1306;
use ssd1306::mode::BufferedGraphicsMode;
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

use gpio::{Gpio4, Gpio5, Unknown};
use i2c::{Master, I2C0};
use shared_bus::BusManager;
use shared_bus::I2cProxy;
use shared_bus::NullMutex;
use ssd1306::prelude::I2CInterface;
use ssd1306::size::DisplaySize128x64;
use ssd1306::Ssd1306;
type SharedBus = BusManager<NullMutex<Master<I2C0, Gpio5<Unknown>, Gpio4<Unknown>>>>;
type Bus<'a> = I2cProxy<'a, NullMutex<Master<I2C0, Gpio5<Unknown>, Gpio4<Unknown>>>>;
type Display<'a> =
    Ssd1306<I2CInterface<Bus<'a>>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>;

pub trait FlushableDrawTarget: DrawTarget {
    fn flush(&mut self) -> Result<(), Self::Error>;
    fn set_display_on(&mut self, on: bool) -> Result<(), Self::Error>;
}

struct FlushableAdaptor<A, B, D> {
    flush_adaptor: A,
    set_display_on_adaptor: B,
    display: D,
}

impl<A, B, D> FlushableAdaptor<A, B, D> {
    pub fn new(flush_adaptor: A, set_display_on_adaptor: B, display: D) -> Self {
        Self {
            flush_adaptor,
            set_display_on_adaptor,
            display,
        }
    }
}

impl<A, B, D> FlushableDrawTarget for FlushableAdaptor<A, B, D>
where
    A: Fn(&mut D) -> Result<(), D::Error>,
    B: Fn(&mut D, bool) -> Result<(), D::Error>,
    D: DrawTarget,
{
    fn flush(&mut self) -> Result<(), Self::Error> {
        (self.flush_adaptor)(&mut self.display)
    }
    fn set_display_on(&mut self, on: bool) -> Result<(), Self::Error> {
        (self.set_display_on_adaptor)(&mut self.display, on)
    }
}

impl<A, B, D> DrawTarget for FlushableAdaptor<A, B, D>
where
    D: DrawTarget,
{
    type Error = D::Error;

    type Color = D::Color;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.display.draw_iter(pixels)
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        self.display.fill_contiguous(area, colors)
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        self.display.fill_solid(area, color)
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.display.clear(color)
    }
}

impl<A, B, D> Dimensions for FlushableAdaptor<A, B, D>
where
    D: Dimensions,
{
    fn bounding_box(&self) -> Rectangle {
        self.display.bounding_box()
    }
}

pub fn get_bus(
    i2c: i2c::I2C0,
    scl: gpio::Gpio4<gpio::Unknown>,
    sda: gpio::Gpio5<gpio::Unknown>,
) -> Result<SharedBus> {
    let config = <i2c::config::MasterConfig as Default>::default().baudrate(400.kHz().into());
    let xxx =
        i2c::Master::<i2c::I2C0, _, _>::new(i2c, i2c::MasterPins { sda, scl }, config).unwrap();
    let bus = shared_bus::BusManagerSimple::new(xxx);

    Ok(bus)
}

// This clippy warning is false, lifetimes are required here.
#[allow(clippy::needless_lifetimes)]
pub fn get_display<'a>(
    bus: Bus<'a>,
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

    let flush = move |d: &mut Display| d.flush();
    let set_display_on = move |d: &mut Display, on: bool| d.set_display_on(on);

    Ok(FlushableAdaptor::new(flush, set_display_on, display))
}

struct State {
    state: DisplayState,
    icon: Icon,
    name: String,
    pressed: bool,
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

    let bus = get_bus(i2c, scl, sda).unwrap();
    let builder = thread::Builder::new().stack_size(8 * 1024);

    builder.spawn(move || {
        let mut states: [[Option<State>; NUM_PAGES as usize]; NUM_COLUMNS as usize] =
            Default::default();
        let mut page_number: usize = 0;

        tx_main
            .send(Message::DisplayPage(page_number as u32))
            .unwrap();

        let display0 = get_display(bus.acquire_i2c(), 0x3C).unwrap();
        let display1 = get_display(bus.acquire_i2c(), 0x3D).unwrap();

        let mut displays: [_; NUM_COLUMNS as usize] = [display0, display1];

        for display in &mut displays {
            led_draw_loading(display);
            display.flush().unwrap();
        }

        let mut update_displays: [bool; NUM_COLUMNS as usize];
        for received in rx {
            update_displays = [true, true];

            match received {
                DisplayCommand::DisplayState(state, icon, id, name) => {
                    let column: usize = (id % NUM_COLUMNS) as usize;
                    let number: usize = (id / NUM_COLUMNS) as usize;

                    let pressed = if let Some(old) = &states[column][number] {
                        old.pressed
                    } else {
                        false
                    };

                    let page = State {
                        state,
                        icon,
                        name,
                        pressed,
                    };
                    states[column][number] = Some(page);

                    update_displays = [false, false];
                    if page_number == number {
                        update_displays[column] = true;
                    }
                }
                DisplayCommand::BlankAll => {
                    for display in &mut displays {
                        display.set_display_on(false).unwrap();
                    }
                }
                DisplayCommand::UnBlankAll => {
                    for display in &mut displays {
                        display.set_display_on(true).unwrap();
                    }
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
                DisplayCommand::ButtonPressed(id) => {
                    let column: usize = (id % NUM_COLUMNS) as usize;
                    let number: usize = (id / NUM_COLUMNS) as usize;
                    if let Some(page) = &mut states[column][number] {
                        page.pressed = true;
                    }
                    update_displays = [false, false];
                    if page_number == number {
                        update_displays[column] = true;
                    }
                }
                DisplayCommand::ButtonReleased(id) => {
                    let column: usize = (id % NUM_COLUMNS) as usize;
                    let number: usize = (id / NUM_COLUMNS) as usize;
                    if let Some(page) = &mut states[column][number] {
                        page.pressed = false;
                    }
                    update_displays = [false, false];
                    if page_number == number {
                        update_displays[column] = true;
                    }
                }
            }

            for (i, display) in &mut displays.iter_mut().enumerate() {
                if update_displays[i] {
                    info!("Drawing display {}", i);
                    let number = page_number * NUM_COLUMNS as usize + i;
                    page_draw(display, &states[i][page_number], number);
                    display.flush().unwrap();
                }
            }

            info!("Done flushing");
        }
    })?;

    Ok(tx)
}

fn page_draw<D>(display: &mut D, state_or_none: &Option<State>, number: usize)
where
    D: DrawTarget,
    D::Color: PixelColor + From<Gray8> + From<Rgb555> + From<Rgb888>,
    D::Error: std::fmt::Debug,
{
    led_clear(display);

    if let Some(state) = state_or_none {
        let image_category = get_image_category(&state.state);
        let image_data = get_image_data(&image_category, &state.icon);
        led_draw_image(display, image_data);
        led_draw_overlay(display, &state.state);
        led_draw_name(display, &state.name);
        if state.pressed {
            led_draw_pressed(display);
        }
    }

    led_draw_number(display, number);
}

fn led_clear<D>(display: &mut D)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    display.clear(Rgb555::BLACK.into()).unwrap();
}

fn led_draw_loading<D>(display: &mut D)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    Rectangle::new(display.bounding_box().top_left, display.bounding_box().size)
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(Rgb555::BLUE.into())
                .stroke_color(Rgb555::YELLOW.into())
                .stroke_width(1)
                .build(),
        )
        .draw(display)
        .unwrap();

    let t = "Loading";

    Text::new(
        t,
        Point::new(10, (display.bounding_box().size.height - 10) as i32 / 2),
        MonoTextStyle::new(&FONT_10X20, Rgb555::WHITE.into()),
    )
    .draw(display)
    .unwrap();
}

fn led_draw_pressed<D>(display: &mut D)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    Rectangle::new(display.bounding_box().top_left, display.bounding_box().size)
        .into_styled(
            PrimitiveStyleBuilder::new()
                .reset_fill_color()
                .stroke_color(Rgb555::YELLOW.into())
                .stroke_width(1)
                .build(),
        )
        .draw(display)
        .unwrap();
}

fn led_draw_number<D>(display: &mut D, number: usize)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    let t = format!("{}", number);

    Text::new(
        &t,
        Point::new(0, 14),
        MonoTextStyle::new(&FONT_10X20, Rgb555::WHITE.into()),
    )
    .draw(display)
    .unwrap();
}

fn led_draw_name<D>(display: &mut D, name: &str)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    Text::new(
        name,
        Point::new(2, (display.bounding_box().size.height - 4) as i32),
        MonoTextStyle::new(&FONT_5X8, Rgb555::WHITE.into()),
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

fn get_image_data<T: PixelColor + From<Gray8> + From<Rgb555> + From<Rgb888>>(
    image: &ImageCategory,
    icon: &button_controllers::Icon,
) -> impl ImageDrawable<Color = T> {
    use ImageCategory::*;

    let data = match icon {
        Icon::Light => match image {
            HardOff => include_bytes!("images/light_hard_off_64x64.tga").as_slice(),
            On => include_bytes!("images/light_on_64x64.tga").as_slice(),
            Off => include_bytes!("images/light_off_64x64.tga").as_slice(),
            OnOther => include_bytes!("images/light_on_other_64x64.tga").as_slice(),
        },
        Icon::Fan => match image {
            HardOff => include_bytes!("images/fan_hard_off_64x64.tga").as_slice(),
            On => include_bytes!("images/fan_on_64x64.tga").as_slice(),
            Off => include_bytes!("images/fan_off_64x64.tga").as_slice(),
            OnOther => include_bytes!("images/fan_on_other_64x64.tga").as_slice(),
        },
        Icon::WakeUp => match image {
            HardOff => include_bytes!("images/wake_up_hard_off_64x64.tga").as_slice(),
            On => include_bytes!("images/wake_up_on_64x64.tga").as_slice(),
            Off => include_bytes!("images/wake_up_off_64x64.tga").as_slice(),
            OnOther => include_bytes!("images/wake_up_on_other_64x64.tga").as_slice(),
        },
        Icon::TV => match image {
            HardOff => include_bytes!("images/tv_hard_off_64x64.tga").as_slice(),
            On => include_bytes!("images/tv_on_64x64.tga").as_slice(),
            Off => include_bytes!("images/tv_off_64x64.tga").as_slice(),
            OnOther => include_bytes!("images/tv_on_other_64x64.tga").as_slice(),
        },
    };

    DynamicTga::from_slice(data).unwrap()
}

fn led_draw_image<D, I, C>(display: &mut D, tga: I)
where
    D: DrawTarget<Color = C>,
    D::Error: std::fmt::Debug,
    I: ImageDrawable<Color = C>,
{
    let size = tga.size();
    let display_size = display.bounding_box();
    let center = display_size.center();

    let x = display_size.bottom_right().unwrap().x - size.width as i32;
    let y = center.y - size.height as i32 / 2;

    Image::new(&tga, Point::new(x, y)).draw(display).unwrap();
}

fn led_draw_overlay<D>(display: &mut D, state: &DisplayState)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
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
                    .fill_color(Rgb555::BLACK.into())
                    .stroke_color(Rgb555::WHITE.into())
                    .stroke_width(1)
                    .build(),
            )
            .draw(display)
            .unwrap();

        Text::with_alignment(
            text,
            Point::new(center.x, y + 17),
            MonoTextStyle::new(&FONT_10X20, Rgb555::WHITE.into()),
            Alignment::Center,
        )
        .draw(display)
        .unwrap();
    }
}
