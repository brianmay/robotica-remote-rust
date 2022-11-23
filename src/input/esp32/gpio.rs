use std::default::Default;

use arr_macro::arr;
use esp_idf_hal::gpio::{self, Input, InterruptType, PinDriver};
use esp_idf_svc::notify::{Configuration, EspNotify, EspSubscription};

use super::super::*;

const NUM_PINS: usize = 40;

static mut SUBSCRIPTION: [Option<EspSubscription>; NUM_PINS] = arr![None; 40];

impl<T: gpio::InputPin> InputPinNotify for PinDriver<'_, T, Input> {
    fn safe_subscribe<F: Fn(Value) + Send + 'static>(&mut self, callback: F) {
        let pin_number: i32 = self.pin();
        let config = Configuration::default();
        let notify = EspNotify::new(&config).unwrap();

        let subscription = notify
            .subscribe(move |v| {
                println!("Pin {} changed to {}", pin_number, v);
                let v: Value = if *v != 0 { Value::High } else { Value::Low };
                callback(v);
            })
            .unwrap();

        self.set_interrupt_type(InterruptType::AnyEdge).unwrap();

        unsafe {
            self.subscribe(move || {
                let value = esp_idf_sys::gpio_get_level(pin_number) as u32;
                notify.post(&value).unwrap();
            })
            .unwrap();
        }

        unsafe {
            SUBSCRIPTION[pin_number as usize] = Some(subscription);
        }
    }
}
