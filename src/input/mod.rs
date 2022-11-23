use std::fmt::Display;

use embedded_hal::digital::InputPin;

pub mod esp32;

#[derive(Copy, Clone, Debug, std::cmp::Eq, std::cmp::PartialEq)]
pub enum Value {
    Low,
    High,
}

#[allow(dead_code)]
pub type InputNotifyCallback = Box<dyn Fn(Value) + Send + 'static>;

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Low => write!(f, "Low"),
            Value::High => write!(f, "High"),
        }
    }
}
pub trait InputPinNotify: InputPin {
    fn safe_subscribe<F: Fn(Value) + Send + 'static>(&mut self, callback: F);
}
