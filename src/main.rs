#![allow(unused_imports)]
#![allow(clippy::single_component_path_imports)]
#![feature(backtrace)]

use std::env;
use std::panic;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::bail;
use anyhow::Error;
use log::*;

use esp_idf_hal::prelude::Peripherals;

mod button;
use crate::button::{Active, Button, ButtonEvent, Debouncer};

mod button_controllers;
use crate::button_controllers::lights::LightConfig;
use crate::button_controllers::switch::SwitchConfig;
use crate::button_controllers::CommonConfig;
use crate::button_controllers::Config;

mod display;
use crate::display::DisplayCommand;

mod wifi;

mod messages;
mod mqtt;

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

fn configure_lca2021_badge(
    tx: mpsc::Sender<messages::Message>,
) -> Result<(Box<dyn wifi::Wifi>, mpsc::Sender<display::DisplayCommand>)> {
    let peripherals = Peripherals::take().unwrap();
    let pins = peripherals.pins;

    let display = display::lca2021_badge::connect(peripherals.i2c0, pins.gpio4, pins.gpio5)?;

    let wifi = wifi::esp::connect()?;

    let pin = pins.gpio16.into_input().unwrap();
    button::esp::configure_button(pin, tx.clone(), 0)?;

    let pin = pins.gpio17.into_input().unwrap();
    button::esp::configure_button(pin, tx, 1)?;

    Ok((Box::new(wifi), display))
}

fn initialize() -> Result<()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    Ok(())
}

fn main() -> Result<()> {
    initialize().unwrap();

    let (tx, rx) = mpsc::channel();

    let (_wifi, display) = configure_lca2021_badge(tx.clone())?;
    let config = get_button_config();

    let mut controllers: Vec<Box<dyn button_controllers::Controller>> =
        config.iter().map(|x| x.create_controller()).collect();

    let mut mqtt = mqtt::Mqtt::new(MQTT_URL);
    mqtt.connect(tx);

    for (index, f) in controllers.iter().enumerate() {
        let subscriptions = f.get_subscriptions();
        for s in subscriptions {
            let label = mqtt::Label {
                component_id: index as u32,
                subscription_id: s.label,
            };
            mqtt.subscribe(&s.topic, label);
        }
    }

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
                let message = DisplayCommand::DisplayState(state, icon, id);
                display.send(message).unwrap();
            }
            messages::Message::MqttConnect => {}
            messages::Message::MqttDisconnect => {
                for (id, controller) in controllers.iter_mut().enumerate() {
                    controller.process_disconnected();
                    let state = controller.get_display_state();
                    let icon = controller.get_icon();
                    let message = DisplayCommand::DisplayState(state, icon, id as u32);
                    display.send(message).unwrap();
                }
            }
            messages::Message::ButtonPress(id) => {
                let controller = controllers.get_mut(id as usize).unwrap();
                let commands = controller.get_press_commands();
                for command in commands {
                    let topic = command.get_topic();
                    let data = command.get_message();
                    info!("press {}: {}", topic, data);
                    mqtt.publish(&topic, false, &data);
                }
            }
            messages::Message::ButtonRelease(_id) => {}
        }
    }

    Ok(())
}
