pub mod lca2021_badge;

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

use crate::{
    button_controllers::{self, DisplayState, Icon},
    messages::Message,
};

pub enum DisplayCommand {
    DisplayState(
        button_controllers::DisplayState,
        button_controllers::Icon,
        usize,
        String,
    ),
    BlankAll,
    UnBlankAll,
    PageUp,
    PageDown,
    ButtonPressed(usize),
    ButtonReleased(usize),
}

trait FlushableDrawTarget: DrawTarget {
    fn flush(&mut self) -> Result<(), Self::Error>;
    fn set_display_on(&mut self, on: bool) -> Result<(), Self::Error>;
}

#[derive(Clone)]
struct State {
    state: DisplayState,
    icon: Icon,
    name: String,
    pressed: bool,
}

fn display_thread<D, const NUM_PAGES: usize, const NUM_DISPLAYS: usize>(
    tx_main: mpsc::Sender<Message>,
    displays: &mut [D; NUM_DISPLAYS],
    rx: mpsc::Receiver<DisplayCommand>,
) where
    D: FlushableDrawTarget,
    D::Color: PixelColor + From<Gray8> + From<Rgb555> + From<Rgb888>,
    D::Error: std::fmt::Debug,
{
    let mut states: Vec<Vec<Option<State>>> = vec![vec![None; NUM_PAGES]; NUM_DISPLAYS];
    let mut selected_page_number: usize = 0;
    tx_main
        .send(Message::DisplayPage(selected_page_number))
        .unwrap();
    for display in displays.iter_mut() {
        led_draw_loading(display);
        display.flush().unwrap();
    }
    for received in rx {
        let mut update_displays: [bool; NUM_DISPLAYS] = [false; NUM_DISPLAYS];

        match received {
            DisplayCommand::DisplayState(state, icon, id, name) => {
                let display_number: usize = id % NUM_DISPLAYS;
                let page_number: usize = id / NUM_DISPLAYS;

                let pressed = if let Some(old) = &states[display_number][page_number] {
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
                states[display_number][page_number] = Some(page);

                if selected_page_number == page_number {
                    update_displays[display_number] = true;
                }
            }
            DisplayCommand::BlankAll => {
                for display in displays.iter_mut() {
                    display.set_display_on(false).unwrap();
                }
                update_displays = [true; NUM_DISPLAYS];
            }
            DisplayCommand::UnBlankAll => {
                for display in displays.iter_mut() {
                    display.set_display_on(true).unwrap();
                }
                update_displays = [true; NUM_DISPLAYS];
            }
            DisplayCommand::PageUp => {
                if selected_page_number + 1 < NUM_PAGES {
                    selected_page_number += 1
                };
                update_displays = [true; NUM_DISPLAYS];
                tx_main
                    .send(Message::DisplayPage(selected_page_number))
                    .unwrap();
            }
            DisplayCommand::PageDown => {
                if selected_page_number > 0 {
                    selected_page_number -= 1
                };
                update_displays = [true; NUM_DISPLAYS];
                tx_main
                    .send(Message::DisplayPage(selected_page_number))
                    .unwrap();
            }
            DisplayCommand::ButtonPressed(id) => {
                let display_number: usize = id % NUM_DISPLAYS;
                let page_number: usize = id / NUM_DISPLAYS;
                if let Some(page) = &mut states[display_number][page_number] {
                    page.pressed = true;
                }
                if selected_page_number == page_number {
                    update_displays[display_number] = true;
                }
            }
            DisplayCommand::ButtonReleased(id) => {
                let display_number: usize = id % NUM_DISPLAYS;
                let page_number: usize = id / NUM_DISPLAYS;
                if let Some(page) = &mut states[display_number][page_number] {
                    page.pressed = false;
                }
                if selected_page_number == page_number {
                    update_displays[display_number] = true;
                }
            }
        }

        for (i, display) in displays.iter_mut().enumerate() {
            if update_displays[i] {
                info!("Drawing display {}", i);
                let number = selected_page_number * NUM_DISPLAYS + i;
                page_draw(display, &states[i][selected_page_number], number);
                display.flush().unwrap();
            }
        }

        info!("Done flushing");
    }
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
