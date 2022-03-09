use std::cell::RefCell;
use std::fmt::Debug;
use std::fmt::Display;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;

use embedded_hal::digital::blocking::InputPin;
use embedded_hal::digital::ErrorType;
use embedded_svc::timer::OnceTimer;
use embedded_svc::timer::Timer;
use embedded_svc::timer::TimerService;
use esp_idf_svc::timer::EspTimerService;
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
    pin.subscribe(move |v| {
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

enum DebouncerMessage {
    Input(Value),
    Subscribe(InputNotifyCallback),
    GetValue(mpsc::Sender<Option<Value>>),
    Timer,
}

pub struct Debouncer {
    tx: mpsc::Sender<DebouncerMessage>,
}

impl Debouncer {
    pub fn new<T: InputPinNotify<Error = impl Debug + Display> + Send + 'static>(
        pin: T,
        debounce_time_ms: u16,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let debounce_time = Duration::from_millis(debounce_time_ms as u64);

        let tx_clone = tx.clone();
        pin.subscribe(move |value| {
            tx_clone.send(DebouncerMessage::Input(value)).unwrap();
        });

        let tx_clone = tx.clone();
        let mut timer_service = EspTimerService::new().unwrap();
        let mut timer = timer_service
            .timer(move || {
                tx_clone.send(DebouncerMessage::Timer).unwrap();
            })
            .unwrap();

        thread::spawn(move || {
            let mut timer_set = false;
            let mut value: Option<Value> = None;
            let mut raw_value: Option<Value> = value;
            let mut subscriber: Option<InputNotifyCallback> = None;

            for msg in rx.iter() {
                match msg {
                    DebouncerMessage::Input(new_value) => {
                        if !timer_set {
                            // println!("Got first value {:?}", new_value);
                            value = Some(new_value);
                            notify(&subscriber, value);
                            timer.cancel().unwrap();
                            timer.after(debounce_time).unwrap();
                            timer_set = true;
                        } else {
                            // println!("Ignoring value {:?}", value);
                        }
                        raw_value = Some(new_value);
                    }
                    DebouncerMessage::Subscribe(new_subscriber) => {
                        // println!("Adding subscribe");
                        subscriber = Some(new_subscriber);
                    }
                    DebouncerMessage::GetValue(reply_tx) => {
                        let out_value = if value.is_some() {
                            value
                        } else if pin.is_high().unwrap_or(false) {
                            Some(Value::High)
                        } else if pin.is_low().unwrap_or(false) {
                            Some(Value::Low)
                        } else {
                            None
                        };
                        reply_tx.send(out_value).unwrap();
                    }
                    DebouncerMessage::Timer => {
                        // println!("Got timer");
                        if value != raw_value {
                            value = raw_value;
                            // println!("Sending {:?}", value);
                            notify(&subscriber, value);
                        }
                        timer_set = false;
                    }
                }
            }
        });

        Debouncer { tx }
    }

    fn get_value(&self) -> Option<Value> {
        let (tx, rx) = mpsc::channel();
        self.tx.send(DebouncerMessage::GetValue(tx)).unwrap();
        rx.recv().unwrap()
    }
}

fn notify(subscriber: &Option<InputNotifyCallback>, new_state: Option<Value>) {
    if let Some(new_state) = new_state {
        match subscriber {
            Some(s) => {
                (*s)(new_state);
            }
            None => {}
        }
    }
}

impl InputPinNotify for Debouncer {
    fn subscribe<F: Fn(Value) + Send + 'static>(&self, callback: F) {
        self.tx
            .send(DebouncerMessage::Subscribe(Box::new(callback)))
            .unwrap();
    }
}

impl InputPin for Debouncer {
    fn is_high(&self) -> Result<bool, Self::Error> {
        let value = self.get_value();
        Ok(matches!(value, Some(Value::High)))
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        let value = self.get_value();
        Ok(matches!(value, Some(Value::Low)))
    }
}

impl ErrorType for Debouncer {
    type Error = anyhow::Error;
}

enum TouchDebouncerMessage {
    Input(Value),
    Subscribe(InputNotifyCallback),
    GetValue(mpsc::Sender<Option<Value>>),
    Timer,
}
pub struct TouchDebouncer {
    tx: mpsc::Sender<TouchDebouncerMessage>,
}

#[derive(Debug)]
enum TouchDebouncerState {
    Inactive,
    Debounce,
    ActivePoll
}

impl TouchDebouncer {
    pub fn new<T: InputPinNotify<Error = impl Debug + Display> + Send + 'static>(
        pin: T,
        debounce_time_ms: u16,
        poll_time_ms: u16,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let debounce_time = Duration::from_millis(debounce_time_ms as u64);
        let poll_time = Duration::from_millis(poll_time_ms as u64);

        let tx_clone = tx.clone();
        pin.subscribe(move |value| {
            tx_clone.send(TouchDebouncerMessage::Input(value)).unwrap();
        });

        let mut timer_service = EspTimerService::new().unwrap();

        let tx_clone = tx.clone();
        let mut timer = timer_service
            .timer(move || {
                tx_clone.send(TouchDebouncerMessage::Timer).unwrap();
            })
            .unwrap();

        thread::spawn(move || {
            let mut state = TouchDebouncerState::Inactive;
            let mut value: Option<Value> = None;
            let mut raw_value: Option<Value> = value;
            let mut subscriber: Option<InputNotifyCallback> = None;

            for msg in rx.iter() {
                match msg {
                    TouchDebouncerMessage::Input(_) => {
                        if pin.is_low().unwrap_or(false) {
                            if let TouchDebouncerState::Inactive = state {
                                // println!("Processing value low value");
                                value = Some(Value::Low);
                                notify(&subscriber, value);
                                timer.cancel().unwrap();
                                timer.after(debounce_time).unwrap();
                                state = TouchDebouncerState::Debounce;
                            } else {
                                // println!("Ignoring value {:?}", value);
                            }
                            raw_value = Some(Value::Low);
                        } else {
                            println!("got event for high value");
                        }
                    }
                    TouchDebouncerMessage::Subscribe(new_subscriber) => {
                        // println!("Adding subscribe");
                        subscriber = Some(new_subscriber);
                    }
                    TouchDebouncerMessage::GetValue(reply_tx) => {
                        let out_value = if value.is_some() {
                            value
                        } else if pin.is_high().unwrap_or(false) {
                            Some(Value::High)
                        } else if pin.is_low().unwrap_or(false) {
                            Some(Value::Low)
                        } else {
                            None
                        };
                        reply_tx.send(out_value).unwrap();
                    }
                    TouchDebouncerMessage::Timer => {
                        if let TouchDebouncerState::Debounce = state {
                            // println!("Got debounce timer");
                            if value != raw_value {
                                value = raw_value;
                                // println!("Sending {:?}", value);
                                notify(&subscriber, value);
                            }
                            state = TouchDebouncerState::ActivePoll;
                        }

                        if let TouchDebouncerState::ActivePoll = state {
                            // println!("Got poll timer");
                            if pin.is_high().unwrap_or(false) {
                                // println!(".... is high");
                                state = TouchDebouncerState::Inactive;
                                value = Some(Value::High);
                                notify(&subscriber, value);
                            }
                        }

                        if let TouchDebouncerState::ActivePoll = state {
                            // println!("reseting poll timer");
                            timer.after(poll_time).unwrap();
                        }
                    }
                }
            }
        });

        TouchDebouncer { tx }
    }

    fn get_value(&self) -> Option<Value> {
        let (tx, rx) = mpsc::channel();
        self.tx.send(TouchDebouncerMessage::GetValue(tx)).unwrap();
        rx.recv().unwrap()
    }
}

impl InputPinNotify for TouchDebouncer {
    fn subscribe<F: Fn(Value) + Send + 'static>(&self, callback: F) {
        self.tx
            .send(TouchDebouncerMessage::Subscribe(Box::new(callback)))
            .unwrap();
    }
}

impl InputPin for TouchDebouncer {
    fn is_high(&self) -> Result<bool, Self::Error> {
        let value = self.get_value();
        Ok(matches!(value, Some(Value::High)))
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        let value = self.get_value();
        Ok(matches!(value, Some(Value::Low)))
    }
}

impl ErrorType for TouchDebouncer {
    type Error = anyhow::Error;
}

pub fn configure_button<T: 'static + InputPinNotify<Error = impl Debug + Display> + Send>(
    pin: T,
    tx: messages::Sender,
    id: ButtonId,
) -> Result<()> {
    let debounced_encoder_pin = Debouncer::new(pin, 30);
    button(debounced_encoder_pin, Active::Low, id, tx);
    Ok(())
}


pub fn configure_touch_button<T: 'static + InputPinNotify<Error = impl Debug + Display> + Send>(
    pin: T,
    tx: messages::Sender,
    id: ButtonId,
) -> Result<()> {
    let debounced_encoder_pin = TouchDebouncer::new(pin, 30, 100);
    button(debounced_encoder_pin, Active::Low, id, tx);
    Ok(())
}
