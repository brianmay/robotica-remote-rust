use std::{thread, time::Duration};

use esp_idf_hal::{
    gpio::{InputPin, OutputPin},
    i2c,
    prelude::*,
};
use ft6x36::{Ft6x36, Point};
use log::*;

use crate::{
    button::ButtonId,
    display::makerfab::NUM_PER_PAGE,
    messages::{self, Message},
};

use super::ButtonInfo;

fn translate(p: Point) -> Point {
    Point {
        x: p.y,
        y: 320u16.saturating_sub(p.x),
    }
}

fn get_button_for_point(buttons: &[ButtonInfo], p: Point) -> Option<&ButtonInfo> {
    for button in buttons {
        let tl = button.position.top_left;
        let br = button.position.bottom_right().unwrap();
        let x = p.x as i32;
        let y = p.y as i32;

        if tl.x <= x && br.x > x && tl.y <= y && br.y > y {
            println!("goit button {:?}", button.id);
            return Some(button);
        }
    }
    None
}

pub(crate) fn connect(
    i2c1: i2c::I2C1,
    sda: impl OutputPin + InputPin + 'static,
    scl: impl OutputPin + 'static,
    buttons: [ButtonInfo; NUM_PER_PAGE],
    tx: messages::Sender,
) {
    let config = <i2c::config::MasterConfig as Default>::default().baudrate(400_u32.kHz().into());
    let i2c1 =
        i2c::Master::<i2c::I2C1, _, _>::new(i2c1, i2c::MasterPins { sda, scl }, config).unwrap();
    let mut touch_screen = Ft6x36::new(i2c1);
    touch_screen.init().unwrap();
    match touch_screen.get_info() {
        Some(info) => info!("Touch screen info: {info:?}"),
        None => warn!("No info"),
    }
    thread::spawn(move || {
        let mut pressed: Option<ButtonId> = None;

        loop {
            // match touch_screen.get_diagnostics() {
            //     Ok(diagnostics) => println!("Touch screen info: {diagnostics:?}"),
            //     Err(err) => println!("No info: {err}"),
            // }

            let x = touch_screen.get_touch_event().unwrap();
            // println!("get_touch_event: {x:?}");

            let button_id = if let Some(p1) = x.p1 {
                let p1 = translate(p1);
                get_button_for_point(&buttons, p1).map(|button| button.id)
            } else {
                Some(ButtonId::NotAButton)
            };

            let (do_release, do_press) = match (pressed, button_id) {
                (None, None) => (None, None),
                (None, Some(button_id)) => (None, Some(button_id)),
                (Some(button_id), None) => (Some(button_id), None),
                (Some(p), Some(r)) if p == r => (None, None),
                (Some(p), Some(r)) => (Some(p), Some(r)),
            };

            if let Some(button_id) = do_release {
                tx.send(Message::ButtonRelease(button_id)).unwrap();
            }

            if let Some(button_id) = do_press {
                tx.send(Message::ButtonPress(button_id)).unwrap();
            }

            pressed = button_id;
            thread::sleep(Duration::from_millis(100));
        }
    });
}
