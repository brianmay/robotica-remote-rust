#![allow(clippy::single_component_path_imports)]
#![feature(backtrace)]

use std::env;
use std::sync::mpsc;

use anyhow::Error;
use log::*;

mod button;

mod button_controllers;
use crate::button_controllers::lights::LightConfig;
use crate::button_controllers::switch::SwitchConfig;
use crate::button_controllers::CommonConfig;
use crate::button_controllers::Config;

mod display;
use crate::display::DisplayCommand;

mod boards;

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

fn main() -> Result<()> {
    boards::lca2021_badge::initialize();

    let (tx, rx) = mpsc::channel();

    let (_wifi, display) = boards::lca2021_badge::configure_devices(tx.clone())?;

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
