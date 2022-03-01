// Adapted from https://github.com/iamabetterdogtht/esp32-touch-sensor-example/blob/a45cd34c43963305bd84fd7dbc31414a5e4c41f4/src/touch.rs

use anyhow::Result;
use esp_idf_hal::gpio;
use esp_idf_sys as sys;
use esp_idf_sys::esp;

use embedded_hal::digital::blocking::InputPin;
use embedded_hal::digital::ErrorType;

const NUM_TOUCH_PINS: usize = 10;

pub struct TouchControllerBuilder {
    touch_pins: [bool; NUM_TOUCH_PINS],
}

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

    pub fn build(self) -> Result<()> {
        esp!(unsafe { sys::touch_pad_filter_start(10) })?;
        esp!(unsafe { sys::touch_pad_clear_status() })?;
        Ok(())
    }
}

impl TouchPin {
    pub fn read(&self) -> Result<u16> {
        let mut touch_value = 0;
        esp!(unsafe { sys::touch_pad_read_filtered(self.pin, &mut touch_value) })?;

        Ok(touch_value)
    }
}

impl ErrorType for TouchPin {
    type Error = anyhow::Error;
}

impl InputPin for TouchPin {
    fn is_high(&self) -> Result<bool> {
        Ok(self.read()? > 400)
    }

    fn is_low(&self) -> Result<bool> {
        Ok(!self.is_high()?)
    }
}
