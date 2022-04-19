use std::default::Default;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use arr_macro::arr;
use embedded_svc::event_bus::EventBus;
use embedded_svc::event_bus::Postbox;

use esp_idf_hal::gpio::Pin;
use esp_idf_svc::notify::{Configuration, EspNotify, EspSubscription};
use esp_idf_sys::{c_types::c_void, gpio_int_type_t_GPIO_INTR_ANYEDGE};

use super::super::*;

const NUM_PINS: usize = 40;

static mut INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut NOTIFY: [Option<EspNotify>; NUM_PINS] = arr![None; 40];
static mut SUBSCRIPTION: [Option<EspSubscription>; NUM_PINS] = arr![None; 40];

impl<T: 'static + Pin + InputPin + Send> InputPinNotify for T {
    fn subscribe<F: Fn(Value) + Send + 'static>(&self, callback: F) {
        let pin_number: i32 = self.pin();

        if unsafe { !INITIALIZED.load(Ordering::SeqCst) } {
            initialize();
            unsafe { INITIALIZED.store(true, Ordering::SeqCst) };
        }

        let config = Configuration::default();
        let mut notify = EspNotify::new(&config).unwrap();

        let s = notify
            .subscribe(move |v| {
                let v: Value = if *v != 0 { Value::High } else { Value::Low };
                callback(v);
            })
            .unwrap();

        unsafe {
            NOTIFY[pin_number as usize] = Some(notify);
            SUBSCRIPTION[pin_number as usize] = Some(s);
        }

        let state_ptr: *mut c_void = pin_number as *mut c_void;
        unsafe {
            // esp_idf_sys::rtc_gpio_deinit(pin_number);
            esp_idf_sys::gpio_set_intr_type(pin_number, gpio_int_type_t_GPIO_INTR_ANYEDGE);
            esp_idf_sys::gpio_isr_handler_add(pin_number, Some(gpio_handler), state_ptr);
            // esp_idf_sys::gpio_intr_enable(pin_number);
        }
    }
}

fn initialize() {
    unsafe {
        esp_idf_sys::gpio_install_isr_service(0);
    }
}

#[no_mangle]
#[inline(never)]
extern "C" fn gpio_handler(data: *mut c_void) {
    let pin_number = data as i32;
    let value = unsafe { esp_idf_sys::gpio_get_level(pin_number) } as u32;

    unsafe {
        let notify = NOTIFY.get_mut(pin_number as usize);

        match notify {
            Some(Some(x)) => {
                x.post(&value, Some(Duration::from_secs(0))).unwrap();
            }
            Some(None) => {}
            None => {}
        }
    }
}
