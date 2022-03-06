use std::fmt::Display;

use embedded_hal::digital::blocking::InputPin;

pub mod esp32;

#[derive(Copy, Clone, Debug)]
pub enum Value {
    Low,
    High,
}

pub type Callback = dyn Fn(i32, Value) + Send + 'static;

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Low => write!(f, "Low"),
            Value::High => write!(f, "High"),
        }
    }
}
pub trait InputPinNotify: InputPin {
    fn subscribe<F: Fn(i32, Value) + Send + 'static>(&self, callback: F);
}
