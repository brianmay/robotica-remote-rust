// Adapted from https://github.com/tonarino/panel-firmware/blob/85540942acba71717b568b2d775ac1c21e0b199f/src/button.rs
use std::fmt::Debug;
use std::fmt::Display;

use anyhow::Result;

use embedded_hal::digital::blocking::InputPin;
use std::thread;

use crate::messages;

#[derive(Clone)]
pub enum ButtonId {
    Physical(usize),
    Controller(usize),
    PageUp,
    PageDown,
}

pub struct Button<T: InputPin> {
    pin: Debouncer<T>,
    button_state: ButtonState,
    id: ButtonId,
}

pub enum ButtonEvent {
    /// The button has just been pressed down.
    Press,

    /// The button was released.
    Release,
}

enum ButtonState {
    Released,
    Pressed,
}

impl<T: 'static + InputPin<Error = impl Debug + Display> + Send> Button<T> {
    pub fn new(pin: Debouncer<T>, id: ButtonId) -> Self {
        let button_state = ButtonState::Released;

        Self {
            pin,
            button_state,
            id,
        }
    }

    // pub fn is_pressed(&self) -> bool {
    //     self.pin.is_pressed()
    // }

    pub fn poll(&mut self) -> Option<ButtonEvent> {
        self.pin.poll();

        match self.button_state {
            ButtonState::Released => {
                if self.pin.is_pressed() {
                    self.button_state = ButtonState::Pressed;
                    return Some(ButtonEvent::Press);
                }
            }
            ButtonState::Pressed => {
                if !self.pin.is_pressed() {
                    self.button_state = ButtonState::Released;
                    return Some(ButtonEvent::Release);
                }
            }
        }

        None
    }

    pub fn connect(mut self, tx: messages::Sender) {
        thread::spawn(move || {
            let frequency = self.pin.get_sample_frequency();
            let duration = std::time::Duration::new(0, 1_000_000_000 / frequency as u32);

            loop {
                thread::sleep(duration);

                match self.poll() {
                    Some(ButtonEvent::Press) => {
                        tx.send(messages::Message::ButtonPress(self.id.clone()))
                            .unwrap();
                    }
                    Some(ButtonEvent::Release) => {
                        tx.send(messages::Message::ButtonRelease(self.id.clone()))
                            .unwrap();
                    }
                    _ => {}
                }
            }
        });
    }
}

// Debouncer code inspired by Kenneth Kuhn's C debouncer:
// http://www.kennethkuhn.com/electronics/debounce.c
pub struct Debouncer<T: InputPin> {
    sample_frequency: u16,
    pin: T,
    integrator: u8,
    max: u8,
    output: bool,
    active_mode: Active,
}

#[allow(dead_code)]
pub enum Active {
    Low,
    High,
}

impl<T: InputPin<Error = impl Debug + Display>> Debouncer<T> {
    pub fn new(pin: T, active_mode: Active, debounce_time_ms: u16, sample_frequency: u16) -> Self {
        let max = ((debounce_time_ms as f32 / 1000.0) * sample_frequency as f32) as u8;

        let integrator = match active_mode {
            Active::Low => max,
            Active::High => 0,
        };

        let output = match active_mode {
            Active::Low => true,
            Active::High => false,
        };

        Self {
            pin,
            integrator,
            max,
            output,
            active_mode,
            sample_frequency,
        }
    }

    pub fn poll(&mut self) {
        if self.pin.is_low().unwrap() {
            self.integrator = self.integrator.saturating_sub(1);
        } else if self.integrator < self.max {
            self.integrator += 1;
        }

        if self.integrator == 0 {
            self.output = false;
        } else if self.integrator >= self.max {
            self.output = true;
        }
    }

    pub fn is_pressed(&self) -> bool {
        matches!(
            (&self.active_mode, self.output),
            (Active::High, true) | (Active::Low, false)
        )
    }

    pub fn get_sample_frequency(&self) -> u16 {
        self.sample_frequency
    }
}

pub fn configure_button<T: 'static + InputPin<Error = impl Debug + Display> + Send>(
    pin: T,
    tx: messages::Sender,
    id: ButtonId,
) -> Result<()> {
    let frequency = 100;

    let debounced_encoder_pin = Debouncer::new(pin, Active::Low, 30, frequency);
    let encoder_button = Button::new(debounced_encoder_pin, id);
    encoder_button.connect(tx);

    Ok(())
}
