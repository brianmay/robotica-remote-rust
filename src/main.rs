#![allow(unused_imports)]
#![allow(clippy::single_component_path_imports)]
#![feature(backtrace)]

use std::env;
use std::panic;
use std::sync::mpsc;

use anyhow::bail;
use anyhow::Error;
use log::*;

use url;

use smol;

use embedded_hal::adc::OneShot;
use embedded_hal::digital::v2::InputPin;

use esp_idf_hal::prelude::Peripherals;

use esp_idf_sys::EspError;

mod button;
use crate::button::{Active, Button, ButtonEvent, Debouncer};

mod button_controllers;
use crate::button_controllers::lights::LightConfig;
use crate::button_controllers::switch::SwitchConfig;
use crate::button_controllers::CommonConfig;
use crate::button_controllers::Config;

mod displays;
use crate::displays::DisplayMessage;

mod messages;
mod mqtt;
mod wifi;

const MQTT_URL: &str = env!("MQTT_URL");

#[cfg(esp32s2)]
include!(env!("EMBUILD_GENERATED_SYMBOLS_FILE"));

#[cfg(esp32s2)]
const ULP: &[u8] = include_bytes!(env!("EMBUILD_GENERATED_BIN_FILE"));

type Result<T, E = Error> = core::result::Result<T, E>;

fn get_button_config() -> [Box<dyn Config>; 2] {
    [
        Box::new(LightConfig {
            c: CommonConfig {
                name: "Brian".to_string(),
                location: "Brian".to_string(),
                device: "Light".to_string(),
                action: button_controllers::Action::Toggle,
                icon: button_controllers::Icon::Light,
            },
            scene: "auto".to_string(),
            priority: 100,
        }),
        Box::new(SwitchConfig {
            c: CommonConfig {
                name: "Brian".to_string(),
                location: "Brian".to_string(),
                device: "Fan".to_string(),
                action: button_controllers::Action::Toggle,
                icon: button_controllers::Icon::Fan,
            },
        }),
    ]
}

fn initialize() -> Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    Ok(())
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

fn main() {
    loop {
        match main_inner() {
            Ok(()) => error!("main unexpected returned"),
            Err(err) => error!("main genereated error: {}", err),
        }
    }
}

fn main_inner() -> Result<()> {
    initialize().unwrap();

    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;
    let displays = displays::connect(peripherals.i2c0, pins.gpio4, pins.gpio5)?;

    let _wifi = wifi::connect();

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

    let pin = pins.gpio16.into_input().unwrap();
    configure_button(pin, &tx, 0)?;

    let pin = pins.gpio17.into_input().unwrap();
    configure_button(pin, &tx, 1)?;

    for received in rx {
        match received {
            messages::Message::MqttReceived(topic, data, label) => {
                info!("got message {} {}", topic, data);
                let id = label.component_id;
                let sid = label.subscription_id;
                let controller = controllers.get_mut(id as usize).unwrap();
                controller.process_message(sid, data);
                let icon = controller.get_icon();
                let state = controller.get_display_state();
                let message = DisplayMessage::DisplayState(state, icon, id);
                displays.send(message)?;
            }
            messages::Message::MqttConnect => {}
            messages::Message::MqttDisconnect => {
                for (id, controller) in controllers.iter_mut().enumerate() {
                    controller.process_disconnected();
                    let state = controller.get_display_state();
                    let icon = controller.get_icon();
                    let message = DisplayMessage::DisplayState(state, icon, id as u32);
                    displays.send(message)?;
                }
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
