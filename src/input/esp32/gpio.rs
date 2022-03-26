use std::default::Default;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use arr_macro::arr;
use embedded_svc::event_bus::EventBus;
use embedded_svc::event_bus::Postbox;
use log::*;

use esp_idf_hal::gpio::Pin;
use esp_idf_svc::eventloop::{
    EspBackgroundEventLoop, EspBackgroundSubscription, EspEventFetchData, EspEventPostData,
    EspTypedEventDeserializer, EspTypedEventSerializer, EspTypedEventSource,
};
use esp_idf_sys::{
    c_types::{self, c_void},
    gpio_int_type_t_GPIO_INTR_ANYEDGE,
};

use super::super::*;

const NUM_PINS: usize = 40;

#[derive(Copy, Clone, Debug)]
struct EventLoopMessage(i32, Value);

impl EspTypedEventSource for EventLoopMessage {
    fn source() -> *const c_types::c_char {
        b"GPIO-SERVICE\0".as_ptr() as *const _
    }
}
impl EspTypedEventSerializer<EventLoopMessage> for EventLoopMessage {
    fn serialize<R>(
        event: &EventLoopMessage,
        f: impl for<'a> FnOnce(&'a EspEventPostData) -> R,
    ) -> R {
        f(&unsafe { EspEventPostData::new(Self::source(), Self::event_id(), event) })
    }
}

impl EspTypedEventDeserializer<EventLoopMessage> for EventLoopMessage {
    fn deserialize<R>(
        data: &EspEventFetchData,
        f: &mut impl for<'a> FnMut(&'a EventLoopMessage) -> R,
    ) -> R {
        f(unsafe { data.as_payload() })
    }
}

static mut INITIALIZED: AtomicBool = AtomicBool::new(false);
static mut EVENT_LOOP: Option<EspBackgroundEventLoop> = None;
static mut SUBSCRIPTION: Option<EspBackgroundSubscription> = None;
static mut CALLBACKS: [Option<InputNotifyCallback>; NUM_PINS] = arr![None; 40];

impl<T: 'static + Pin + InputPin + Send> InputPinNotify for T {
    fn subscribe<F: Fn(Value) + Send + 'static>(&self, callback: F) {
        let pin_number: i32 = self.pin();

        if unsafe { !INITIALIZED.load(Ordering::SeqCst) } {
            initialize();
            unsafe { INITIALIZED.store(true, Ordering::SeqCst) };
        }

        unsafe {
            CALLBACKS[pin_number as usize] = Some(Box::new(callback));
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
    info!("About to start a background event loop");
    let mut event_loop = EspBackgroundEventLoop::new(&Default::default()).unwrap();
    info!("About to subscribe to the background event loop");
    let subscription = event_loop
        .subscribe(move |message: &EventLoopMessage| {
            let pin_number = message.0;
            let value = message.1;
            info!(
                "Got message from the event loop: {} {:?} ",
                pin_number, value
            );
            let callback = unsafe { &CALLBACKS[pin_number as usize] };
            if let Some(callback) = callback {
                callback(value);
            }
            info!("returned from callback");
        })
        .unwrap();

    unsafe {
        esp_idf_sys::gpio_install_isr_service(0);
    }

    unsafe {
        EVENT_LOOP = Some(event_loop);
        SUBSCRIPTION = Some(subscription);
    }
}

#[no_mangle]
#[inline(never)]
extern "C" fn gpio_handler(data: *mut c_void) {
    let pin_number = data as i32;
    let value = unsafe { esp_idf_sys::gpio_get_level(pin_number) };
    let value = if value != 0 { Value::High } else { Value::Low };

    unsafe {
        match &mut EVENT_LOOP {
            Some(x) => {
                x.post(
                    &EventLoopMessage(pin_number, value),
                    Some(Duration::from_secs(0)),
                )
                .unwrap();
            }
            None => {}
        }
    }
}
