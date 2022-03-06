use std::cell::RefCell;
use std::fmt::Debug;
use std::fmt::Display;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;

use embedded_hal::digital::blocking::InputPin;
use embedded_hal::digital::ErrorType;
use std::thread;

use crate::input::InputNotifyCallback;
use crate::input::InputPinNotify;
use crate::input::Value;
use crate::messages;
use crate::messages::Message::ButtonPress;
use crate::messages::Message::ButtonRelease;

#[derive(Clone, Debug)]
pub enum ButtonId {
    Physical(usize),
    Controller(usize),
    PageUp,
    PageDown,
}

#[allow(dead_code)]
pub enum Active {
    Low,
    High,
}

pub fn button<T: InputPinNotify<Error = impl Debug + Display>>(
    pin: T,
    active: Active,
    id: ButtonId,
    tx: messages::Sender,
) {
    let value: RefCell<Option<Value>> = RefCell::new(None);
    pin.subscribe(move |pin_number, v| {
        println!("{} {} {:?}", pin_number, v, id);
        let pressed = matches!(
            (&active, v),
            (Active::Low, Value::Low) | (Active::High, Value::High)
        );

        let has_changed = match (*value.borrow(), v) {
            (None, _) => true,
            (Some(Value::High), Value::Low) => true,
            (Some(Value::Low), Value::High) => true,
            (Some(_), _) => false,
        };

        if has_changed {
            let id = id.clone();
            if pressed {
                tx.send(ButtonPress(id)).unwrap();
            } else {
                tx.send(ButtonRelease(id)).unwrap();
            }
            *value.borrow_mut() = Some(v);
        }
    });
}

pub struct Debouncer {
    value: Arc<Mutex<Value>>,
    subscriber: Arc<Mutex<Option<InputNotifyCallback>>>,
}

impl Debouncer {
    pub fn new<T: InputPinNotify<Error = impl Debug + Display>>(
        pin: T,
        debounce_time_ms: u16,
    ) -> Self {
        let (tx, rx) = mpsc::channel();

        pin.subscribe(move |pin_number, value| {
            tx.send((pin_number, value)).unwrap();
        });

        let value = Arc::new(Mutex::new(Value::Low));
        let subscriber: Arc<Mutex<Option<InputNotifyCallback>>> = Arc::new(Mutex::new(None));

        let value_clone = value.clone();
        let subscriber_clone = subscriber.clone();

        thread::spawn(move || {
            for (pin_number, msg) in rx {
                let mut value_lock = value_clone.lock().unwrap();
                *value_lock = msg;
                drop(value_lock);

                let subscribers_lock = subscriber_clone.lock().unwrap();
                let duration = std::time::Duration::new(0, debounce_time_ms as u32 * 1000);
                match &*subscribers_lock {
                    Some(s) => {
                        (*s)(pin_number, msg);
                    }
                    None => {}
                }
                drop(subscribers_lock);

                thread::sleep(duration);
            }
        });

        Debouncer {
            // pin: pin,
            value,
            subscriber,
        }
    }
}

impl InputPinNotify for Debouncer {
    fn subscribe<F: Fn(i32, Value) + Send + 'static>(&self, callback: F) {
        let mut value = self.subscriber.lock().unwrap();
        *value = Some(Box::new(callback));
    }
}

impl InputPin for Debouncer {
    fn is_high(&self) -> Result<bool, Self::Error> {
        let value = self.value.lock().unwrap();
        Ok(matches!(*value, Value::High))
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        let value = self.value.lock().unwrap();
        Ok(matches!(*value, Value::Low))
    }
}

impl ErrorType for Debouncer {
    type Error = anyhow::Error;
}

pub fn configure_button<T: 'static + InputPinNotify<Error = impl Debug + Display> + Send>(
    pin: T,
    tx: messages::Sender,
    id: ButtonId,
) -> Result<()> {
    let debounced_encoder_pin = Debouncer::new(pin, 30);
    let _encoder_button = button(debounced_encoder_pin, Active::Low, id, tx);
    Ok(())
}
