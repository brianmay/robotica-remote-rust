use std::cmp::max;
use std::sync::mpsc;

use log::*;

use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
use embedded_graphics::{
    draw_target::DrawTarget,
    image::Image,
    mono_font::ascii::FONT_5X8,
    pixelcolor::{Gray8, Rgb555, Rgb888},
    prelude::{ImageDrawable, PixelColor, Point, Primitive, RgbColor, Size},
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::{Alignment, Text},
    Drawable,
};
use tinytga::DynamicTga;

use crate::button_controllers::{self, DisplayState, Icon};

use super::DisplayCommand;

pub trait FlushableDrawTarget: DrawTarget {
    fn flush(&mut self) -> Result<(), Self::Error>;
    fn set_display_on(&mut self, on: bool) -> Result<(), Self::Error>;
}

#[derive(Clone)]
pub struct State {
    state: DisplayState,
    icon: Icon,
    name: String,
    pressed: bool,
}

pub fn display_thread<D, const NUM_PER_PAGE: usize, const NUM_DISPLAYS: usize>(
    displays: &mut [D; NUM_DISPLAYS],
    components: &[Button; NUM_PER_PAGE],
    rx: mpsc::Receiver<DisplayCommand>,
) where
    D: FlushableDrawTarget,
    D::Color: PixelColor + From<Gray8> + From<Rgb555> + From<Rgb888>,
    D::Error: std::fmt::Debug,
{
    let mut states: Vec<Option<State>> = vec![None; NUM_PER_PAGE];
    let mut selected_page_number: usize = 0;

    for display in displays.iter_mut() {
        display.set_display_on(true).unwrap();
        led_draw_loading(display);
        display.flush().unwrap();
    }

    for received in rx {
        let mut update_components: [bool; NUM_PER_PAGE] = [false; NUM_PER_PAGE];

        match received {
            DisplayCommand::Started => {
                for display in displays.iter_mut() {
                    display.clear(Rgb555::GREEN.into()).unwrap();
                    display.flush().unwrap();
                }
            }
            DisplayCommand::DisplayState(state, icon, id, name) => {
                let pressed = if let Some(old) = &states[id] {
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
                states[id] = Some(page);
                update_components[id] = true;
            }
            DisplayCommand::DisplayNone(id) => {
                states[id] = None;
                update_components[id] = true;
            }
            DisplayCommand::BlankAll => {
                for display in displays.iter_mut() {
                    display.set_display_on(false).unwrap();
                }
            }
            DisplayCommand::UnBlankAll => {
                for display in displays.iter_mut() {
                    display.set_display_on(true).unwrap();
                }
                // update_components = [true; NUM_PER_PAGE];
            }
            DisplayCommand::ShowPage(page_num) => {
                selected_page_number = page_num;
                update_components = [true; NUM_PER_PAGE];
            }
            DisplayCommand::ButtonPressed(id) => {
                if let Some(page) = &mut states[id] {
                    page.pressed = true;
                }
                update_components[id] = true;
            }
            DisplayCommand::ButtonReleased(id) => {
                if let Some(page) = &mut states[id] {
                    page.pressed = false;
                }
                update_components[id] = true;
            }
        }

        for (id, component) in components.iter().enumerate() {
            let state = &states[id];
            if update_components[id] {
                component.draw(displays, state, selected_page_number);
            }
        }

        if update_components.iter().any(|x| *x) {
            for display in displays.iter_mut() {
                display.flush().unwrap();
            }
        }

        info!("Done flushing");
    }
}
pub struct Button {
    display: usize,
    bounding_box: Rectangle,
}

impl Button {
    pub fn new(display: usize, bounding_box: Rectangle) -> Button {
        Button {
            display,
            bounding_box,
        }
    }

    fn draw<D>(&self, displays: &mut [D], state: &Option<State>, page_num: usize)
    where
        D: FlushableDrawTarget,
        D::Color: PixelColor + From<Gray8> + From<Rgb555> + From<Rgb888>,
        D::Error: std::fmt::Debug,
    {
        let display = &mut displays[self.display];
        page_draw(display, state, page_num, &self.bounding_box);
    }
}

fn page_draw<D>(
    display: &mut D,
    state_or_none: &Option<State>,
    number: usize,
    bounding_box: &Rectangle,
) where
    D: DrawTarget,
    D::Color: PixelColor + From<Gray8> + From<Rgb555> + From<Rgb888>,
    D::Error: std::fmt::Debug,
{
    led_clear(display, bounding_box);

    if let Some(state) = state_or_none {
        let image_category = get_image_category(&state.state);
        let image_data = get_image_data(&image_category, &state.icon);
        led_draw_image(display, image_data, bounding_box);
        led_draw_overlay(display, &state.state, bounding_box);
        led_draw_name(display, &state.name, bounding_box);
        if state.pressed {
            led_draw_pressed(display, bounding_box);
        }
    }

    led_draw_number(display, number, bounding_box);
}

fn led_clear<D>(display: &mut D, bounding_box: &Rectangle)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    display
        .fill_solid(bounding_box, Rgb555::BLACK.into())
        .unwrap();
}

fn led_draw_loading<D>(display: &mut D)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    display
        .bounding_box()
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

fn led_draw_pressed<D>(display: &mut D, bounding_box: &Rectangle)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    bounding_box
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

fn led_draw_number<D>(display: &mut D, number: usize, bounding_box: &Rectangle)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    let t = format!("{}", number);

    Text::new(
        &t,
        Point::new(bounding_box.top_left.x + 2, bounding_box.top_left.y + 14),
        MonoTextStyle::new(&FONT_10X20, Rgb555::WHITE.into()),
    )
    .draw(display)
    .unwrap();
}

fn led_draw_name<D>(display: &mut D, name: &str, bounding_box: &Rectangle)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    Text::new(
        name,
        Point::new(
            bounding_box.top_left.x + 2,
            (bounding_box.bottom_right().unwrap().y - 4) as i32,
        ),
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

fn led_draw_image<D, I, C>(display: &mut D, tga: I, bounding_box: &Rectangle)
where
    D: DrawTarget<Color = C>,
    D::Error: std::fmt::Debug,
    I: ImageDrawable<Color = C>,
{
    let size = tga.size();
    let center = bounding_box.center();

    let x = max(
        bounding_box.bottom_right().unwrap().x - size.width as i32,
        bounding_box.top_left.x,
    );
    let y = max(center.y - size.height as i32 / 2, bounding_box.top_left.y);

    Image::new(&tga, Point::new(x, y)).draw(display).unwrap();
}

fn led_draw_overlay<D>(display: &mut D, state: &DisplayState, bounding_box: &Rectangle)
where
    D: DrawTarget,
    D::Color: From<Rgb555>,
    D::Error: std::fmt::Debug,
{
    let text = match state {
        DisplayState::HardOff => "Hard off",
        DisplayState::Error => "Error",
        DisplayState::Unknown => "Lost",
        DisplayState::On => "On",
        DisplayState::Off => "Off",
        DisplayState::OnOther => "Other",
    };

    if matches!(state, DisplayState::Error | DisplayState::Unknown) {
        let center = bounding_box.center();
        let size = Size::new(60, 24);

        let x = center.x - size.width as i32 / 2;
        let y = bounding_box.bottom_right().unwrap().y - 30;
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
