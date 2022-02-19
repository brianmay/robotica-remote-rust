#![allow(unused_imports)]
#![allow(clippy::single_component_path_imports)]
#![feature(backtrace)]

use std::error::Error;
use std::panic;
use std::sync::mpsc;
use std::{cell::RefCell, env, sync::atomic::*, sync::Arc, thread, time::*};

use anyhow::bail;
use display_interface::DisplayError;
use log::*;

use url;

use smol;

use embedded_hal::adc::OneShot;
use embedded_hal::digital::v2::InputPin;

use embedded_svc::eth;
use embedded_svc::eth::Eth;
use embedded_svc::httpd::registry::*;
use embedded_svc::httpd::*;
use embedded_svc::io;
use embedded_svc::utils::anyerror::*;
use embedded_svc::wifi::*;

use esp_idf_svc::netif::*;
use esp_idf_svc::nvs::*;
use esp_idf_svc::sysloop::*;
use esp_idf_svc::wifi::*;

use esp_idf_hal::adc;
use esp_idf_hal::delay;
use esp_idf_hal::gpio::{self, GpioPin, Input, Pins, Pull};
use esp_idf_hal::i2c;
use esp_idf_hal::prelude::*;
use esp_idf_hal::spi;

use esp_idf_sys::esp;
use esp_idf_sys::EspError;
use esp_idf_sys::{self, c_types};

use display_interface_spi::SPIInterfaceNoCS;

use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
use embedded_graphics::pixelcolor::*;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::*;
use embedded_graphics::text::*;

use ssd1306;
use ssd1306::mode::DisplayConfig;

mod button;
use crate::button::{Active, Button, ButtonEvent, Debouncer};

mod button_controllers;
use crate::button_controllers::lights::LightConfig;
use crate::button_controllers::CommonConfig;
use crate::button_controllers::Config;

mod messages;
mod mqtt;

const SSID: &str = env!("WIFI_SSID");
const PASS: &str = env!("WIFI_PASS");
const MQTT_URL: &str = env!("MQTT_URL");

#[cfg(esp32s2)]
include!(env!("EMBUILD_GENERATED_SYMBOLS_FILE"));

#[cfg(esp32s2)]
const ULP: &[u8] = include_bytes!(env!("EMBUILD_GENERATED_BIN_FILE"));

thread_local! {
    static TLS: RefCell<u32> = RefCell::new(13);
}

fn get_button_config() -> [Box<dyn Config>; 2] {
    [
        Box::new(LightConfig {
            c: CommonConfig {
                name: "Brian".to_string(),
                location: "Brian".to_string(),
                device: "Light".to_string(),
                action: button_controllers::Action::TurnOn,
            },
            scene: "auto".to_string(),
            priority: 100,
        }),
        Box::new(LightConfig {
            c: CommonConfig {
                name: "Brian".to_string(),
                location: "Brian".to_string(),
                device: "Light".to_string(),
                action: button_controllers::Action::TurnOff,
            },
            scene: "auto".to_string(),
            priority: 100,
        }),
    ]
}

enum DisplayMessage {
    DisplayState(button_controllers::DisplayState, u32),
}

fn initialize() -> Result<Box<EspWifi>> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let netif_stack = Arc::new(EspNetifStack::new()?);
    let sys_loop_stack = Arc::new(EspSysLoopStack::new()?);
    let default_nvs = Arc::new(EspDefaultNvs::new()?);

    let wifi = wifi(
        netif_stack.clone(),
        sys_loop_stack.clone(),
        default_nvs.clone(),
    )?;

    Ok(wifi)
}

fn configure_button<T: 'static + InputPin<Error = EspError> + Send>(
    pin: T,
    tx: &messages::Sender,
    id: u32,
) -> Result<()> {
    let frequency = 100;

    let debounced_encoder_pin = Debouncer::new(pin, Active::Low, 30, frequency);
    let encoder_button_1 = Button::new(debounced_encoder_pin, id);
    encoder_button_1.connect(tx.clone());

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let result = panic::catch_unwind(|| main_inner());

    match result {
        Ok(rc) => match rc {
            Ok(_) => error!("main unexpected returned"),
            Err(err) => error!("main genereated error: {}", err),
        },
        Err(err) => error!("main genereated error: {:?}", err),
    };

    Ok(())
}

fn main_inner() -> Result<(), Box<dyn Error>> {
    let _wifi = initialize()?;
    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let displays = ssd1306g_hello_world(peripherals.i2c0, pins.gpio4, pins.gpio5)?;

    let (tx, rx) = mpsc::channel();

    let config = get_button_config();
    let mut controllers: Vec<Box<dyn button_controllers::Controller>> =
        config.iter().map(|x| x.create_controller()).collect();

    let mut mqtt = mqtt::Mqtt::new(MQTT_URL);

    mqtt.connect(tx.clone())?;

    for (index, f) in controllers.iter().enumerate() {
        let subscriptions = f.get_subscriptions();
        for s in subscriptions {
            let label = mqtt::Label {
                component_id: index as u32,
                subscription_id: s.label,
            };
            mqtt.subscribe(&s.topic, label)?;
        }
    }

    let pin = pins.gpio16.into_input()?;
    configure_button(pin, &tx, 0)?;

    let pin = pins.gpio17.into_input()?;
    configure_button(pin, &tx, 1)?;

    for received in rx {
        match received {
            messages::Message::MqttMessage(topic, data, label) => {
                info!("got message {} {}", topic, data);
                let id = label.component_id;
                let sid = label.subscription_id;
                let controller = controllers.get_mut(id as usize).unwrap();
                controller.process_message(sid, data);
                let state = controller.get_display_state();
                let message = DisplayMessage::DisplayState(state, id);
                displays.send(message)?;
            }
            messages::Message::ButtonPress(id) => {
                let controller = controllers.get_mut(id as usize).unwrap();
                let commands = controller.get_press_commands();
                for command in commands {
                    let topic = command.get_topic();
                    let data = command.get_message();
                    info!("press {}: {}", topic, data);
                    mqtt.publish(&topic, false, &data)?;
                }
            }
            messages::Message::ButtonRelease(_id) => {}
        }
    }

    Ok(())
}

fn ssd1306g_hello_world(
    i2c: i2c::I2C0,
    scl: gpio::Gpio4<gpio::Unknown>,
    sda: gpio::Gpio5<gpio::Unknown>,
) -> Result<mpsc::Sender<DisplayMessage>> {
    let (tx, rx) = mpsc::channel();

    let config = <i2c::config::MasterConfig as Default>::default().baudrate(400.kHz().into());
    let xxx =
        i2c::Master::<i2c::I2C0, _, _>::new(i2c, i2c::MasterPins { sda, scl }, config).unwrap();
    let bus = shared_bus::BusManagerSimple::new(xxx);

    let builder = thread::Builder::new().stack_size(8 * 1024);

    builder.spawn(move || {
        use ssd1306::Ssd1306;

        let di0 = ssd1306::I2CDisplayInterface::new_custom_address(bus.acquire_i2c(), 0x3C);
        let di1 = ssd1306::I2CDisplayInterface::new_custom_address(bus.acquire_i2c(), 0x3D);

        let mut display0 = ssd1306::Ssd1306::new(
            di0,
            ssd1306::size::DisplaySize128x64,
            ssd1306::rotation::DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();

        let mut display1 = ssd1306::Ssd1306::new(
            di1,
            ssd1306::size::DisplaySize128x64,
            ssd1306::rotation::DisplayRotation::Rotate0,
        )
        .into_buffered_graphics_mode();

        let z = AnyError::<display_interface::DisplayError>::wrap(|| {
            display0.init()?;
            display1.init()?;

            led_draw_number(&mut display0, 0)?;
            led_draw_number(&mut display1, 1)?;

            display0.flush()?;
            display1.flush()
        });

        match z {
            Ok(_) => {}
            Err(err) => error!("Got error {}", err),
        }

        for received in rx {
            match received {
                DisplayMessage::DisplayState(state, id) => {
                    info!("got message to display on {}", id);
                    let display = match id {
                        0 => &mut display0,
                        1 => &mut display1,
                        _ => panic!("Invalid display value received"),
                    };

                    let message = match state {
                        button_controllers::DisplayState::HardOff => "hard off",
                        button_controllers::DisplayState::Error => "error",
                        button_controllers::DisplayState::Unknown => "unknown",
                        button_controllers::DisplayState::On => "on",
                        button_controllers::DisplayState::Off => "off",
                        button_controllers::DisplayState::Auto => "auto",
                        button_controllers::DisplayState::Rainbow => "rainbow",
                    };

                    led_draw_string(display, message).unwrap();
                    display.flush().unwrap();
                }
            }
        }
    })?;

    Ok(tx)
}

fn led_draw_number<D>(display: &mut D, number: u8) -> Result<(), D::Error>
where
    D: DrawTarget<Error = DisplayError> + Dimensions,
    D::Color: From<Rgb565>,
{
    display.clear(Rgb565::BLACK.into())?;

    Rectangle::new(display.bounding_box().top_left, display.bounding_box().size)
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(Rgb565::BLUE.into())
                .stroke_color(Rgb565::YELLOW.into())
                .stroke_width(1)
                .build(),
        )
        .draw(display)?;

    let t = format!("Hello Rusty\n{}", number);

    Text::new(
        &t,
        Point::new(10, (display.bounding_box().size.height - 10) as i32 / 2),
        MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE.into()),
    )
    .draw(display)?;

    info!("LED rendering number done {}", number);

    Ok(())
}

fn led_draw_string<D>(display: &mut D, t: &str) -> Result<(), D::Error>
where
    D: DrawTarget<Error = DisplayError> + Dimensions,
    D::Color: From<Rgb565>,
{
    display.clear(Rgb565::BLACK.into())?;

    Rectangle::new(display.bounding_box().top_left, display.bounding_box().size)
        .into_styled(
            PrimitiveStyleBuilder::new()
                .fill_color(Rgb565::BLUE.into())
                .stroke_color(Rgb565::YELLOW.into())
                .stroke_width(1)
                .build(),
        )
        .draw(display)?;

    Text::new(
        &t,
        Point::new(10, (display.bounding_box().size.height - 10) as i32 / 2),
        MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE.into()),
    )
    .draw(display)?;

    info!("LED rendering string done {}", t);

    Ok(())
}

fn wifi(
    netif_stack: Arc<EspNetifStack>,
    sys_loop_stack: Arc<EspSysLoopStack>,
    default_nvs: Arc<EspDefaultNvs>,
) -> Result<Box<EspWifi>> {
    let mut wifi = Box::new(EspWifi::new(netif_stack, sys_loop_stack, default_nvs)?);

    info!("Wifi created, about to scan");

    let ap_infos = wifi.scan()?;

    let ours = ap_infos.into_iter().find(|a| a.ssid == SSID);

    let channel = if let Some(ours) = ours {
        info!(
            "Found configured access point {} on channel {}",
            SSID, ours.channel
        );
        Some(ours.channel)
    } else {
        info!(
            "Configured access point {} not found during scanning, will go with unknown channel",
            SSID
        );
        None
    };

    wifi.set_configuration(&Configuration::Mixed(
        ClientConfiguration {
            ssid: SSID.into(),
            password: PASS.into(),
            channel,
            ..Default::default()
        },
        AccessPointConfiguration {
            ssid: "aptest".into(),
            channel: channel.unwrap_or(1),
            ..Default::default()
        },
    ))?;

    info!("Wifi configuration set, about to get status");

    let status = wifi.get_status();

    if let Status(
        ClientStatus::Started(ClientConnectionStatus::Connected(ClientIpStatus::Done(
            _ip_settings,
        ))),
        ApStatus::Started(ApIpStatus::Done),
    ) = status
    {
        info!("Wifi connected");

        // ping(&ip_settings)?;
    } else {
        bail!("Unexpected Wifi status: {:?}", status);
    }

    Ok(wifi)
}
