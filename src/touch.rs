// Adapted from https://github.com/anichno/esp32-touch-sensor-example/blob/a45cd34c43963305bd84fd7dbc31414a5e4c41f4/src/touch.rs
use std::sync::atomic::{AtomicBool, Ordering};
use std::{ffi, ptr};

use anyhow::Result;
use arr_macro::arr;
use esp_idf_hal::gpio;
use esp_idf_sys as sys;
use esp_idf_sys::esp;

use embedded_hal::digital::blocking::InputPin;
use embedded_hal::digital::ErrorType;

const NUM_TOUCH_PINS: usize = 10;
static TOUCH_PIN_TOUCHED: [AtomicBool; NUM_TOUCH_PINS] = arr![AtomicBool::new(false); 10];

pub struct TouchControllerBuilder {
    touch_pins: [bool; NUM_TOUCH_PINS],
}

pub struct TouchController {}

pub struct TouchPin {
    pin: sys::touch_pad_t,
}

impl TouchControllerBuilder {
    pub fn new() -> Result<Self> {
        esp!(unsafe { sys::touch_pad_init() })?;
        esp!(unsafe { sys::touch_pad_set_fsm_mode(sys::touch_fsm_mode_t_TOUCH_FSM_MODE_TIMER) })?;
        esp!(unsafe {
            sys::touch_pad_set_voltage(
                sys::touch_high_volt_t_TOUCH_HVOLT_2V7,
                sys::touch_low_volt_t_TOUCH_LVOLT_0V5,
                sys::touch_volt_atten_t_TOUCH_HVOLT_ATTEN_1V,
            )
        })?;
        Ok(Self {
            touch_pins: [false; NUM_TOUCH_PINS],
        })
    }

    pub fn add_pin(&mut self, pin: impl gpio::TouchPin) -> Result<TouchPin> {
        let channel = pin.touch_channel();
        self.touch_pins[channel as usize] = true;
        esp!(unsafe { sys::touch_pad_config(channel, 0) })?;
        Ok(TouchPin { pin: channel })
    }

    pub fn build(self) -> Result<TouchController> {
        esp!(unsafe { sys::touch_pad_filter_start(10) })?;

        let mut touch_value = 0;
        for (i, channel) in self.touch_pins.iter().enumerate() {
            if *channel {
                esp!(unsafe { sys::touch_pad_read_filtered(i as _, &mut touch_value) })?;
                let threshold = touch_value * 2 / 3;
                esp!(unsafe { sys::touch_pad_set_thresh(i as _, threshold) })?;
            }
        }

        esp!(unsafe { sys::touch_pad_isr_register(Some(handle_touch), ptr::null_mut()) })?;
        esp!(unsafe { sys::touch_pad_clear_status() })?;
        esp!(unsafe { sys::touch_pad_intr_enable() })?;

        Ok(TouchController {})
    }
}

impl TouchPin {
    pub fn read(&self) -> Result<u16> {
        let mut touch_value = 0;
        esp!(unsafe { sys::touch_pad_read_filtered(self.pin, &mut touch_value) })?;

        Ok(touch_value)
    }

    // pub fn touched(&self) -> bool {
    //     let pin = self.pin as usize;
    //     if TOUCH_PIN_TOUCHED[pin].load(Ordering::SeqCst) {
    //         TOUCH_PIN_TOUCHED[pin].store(false, Ordering::SeqCst);
    //         return true;
    //     }

    //     false
    // }
}

unsafe extern "C" fn handle_touch(_: *mut ffi::c_void) {
    let pad_intr = sys::touch_pad_get_status();
    if esp!(sys::touch_pad_clear_status()).is_ok() {
        for (i, tracker) in TOUCH_PIN_TOUCHED.iter().enumerate() {
            if (pad_intr >> i) & 1 == 1 {
                tracker.store(true, Ordering::SeqCst);
            }
        }
    }
}

impl ErrorType for TouchPin {
    type Error = anyhow::Error;
}

impl InputPin for TouchPin {
    fn is_high(&self) -> Result<bool> {
        Ok(self.read()? > 300)
    }

    fn is_low(&self) -> Result<bool> {
        Ok(!self.is_high()?)
    }
}
