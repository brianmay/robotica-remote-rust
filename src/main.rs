#![allow(clippy::single_component_path_imports)]
#![feature(backtrace)]

use std::env;
use std::sync::mpsc;

use anyhow::Error;
use embedded_svc::timer::OnceTimer;
use embedded_svc::timer::Timer;
use embedded_svc::timer::TimerService;
use esp_idf_svc::timer::EspTimer;
use esp_idf_svc::timer::EspTimerService;
use log::*;

mod button;

mod button_controllers;
use crate::button_controllers::lights::LightConfig;
use crate::button_controllers::switch::SwitchConfig;
use crate::button_controllers::CommonConfig;
use crate::button_controllers::Config;

mod display;
use crate::display::DisplayCommand;
use crate::messages::Message;

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

    let mqtt = mqtt::Mqtt::connect(MQTT_URL, tx.clone());

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

    let update_controller =
        |id: u32, controller: &dyn button_controllers::Controller, blank: bool| {
            let state = controller.get_display_state();
            let icon = controller.get_icon();
            if !blank {
                let message = DisplayCommand::DisplayState(state, icon, id as u32);
                display.send(message).unwrap();
            }
        };

    let update_displays = |controllers: &Vec<Box<dyn button_controllers::Controller>>,
                           blank: bool| {
        for (id, controller) in controllers.iter().enumerate() {
            update_controller(id as u32, controller.as_ref(), blank);
        }
    };

    let do_blank = |timer: &mut EspTimer, blank: &mut bool| {
        if !*blank {
            info!("Blanking screen");
            timer.cancel().unwrap();
            *blank = true;
            display.send(DisplayCommand::BlankAll).unwrap();
        }
    };

    let do_unblank = |controllers: &Vec<Box<dyn button_controllers::Controller>>,
                      timer: &mut EspTimer,
                      blank: &mut bool| {
        if *blank {
            info!("Unblanking screen");
            timer.cancel().unwrap();
            timer.after(std::time::Duration::new(10, 0)).unwrap();
            *blank = false;
            update_displays(controllers, *blank);
        }
    };

    let mut timer_service = EspTimerService::new().unwrap();
    let mut timer = timer_service
        .timer(move || {
            tx.send(Message::BlankDisplays).unwrap();
        })
        .unwrap();
    timer.after(std::time::Duration::new(10, 0)).unwrap();

    let mut blank = false;

    for received in rx {
        match received {
            Message::MqttReceived(topic, data, label) => {
                info!("Got message {}: {}", topic, data);
                let id = label.component_id;
                let sid = label.subscription_id;
                let controller = controllers.get_mut(id as usize).unwrap();
                controller.process_message(sid, data);
                update_controller(id, controller.as_ref(), blank);
            }
            Message::MqttConnect => {
                info!("Got connected");
            }
            Message::MqttDisconnect => {
                info!("Got disconnected");
                for controller in controllers.iter_mut() {
                    controller.process_disconnected();
                }
                update_displays(&controllers, blank);
            }
            Message::ButtonPress(id) => {
                info!("Got button {} press", id);
                do_unblank(&controllers, &mut timer, &mut blank);

                let controller = controllers.get_mut(id as usize).unwrap();
                let commands = controller.get_press_commands();
                for command in commands {
                    let topic = command.get_topic();
                    let data = command.get_message();
                    info!("Send {}: {}", topic, data);
                    mqtt.publish(&topic, false, &data);
                }
            }
            Message::ButtonRelease(id) => {
                info!("Got button {} release", id);
                do_unblank(&controllers, &mut timer, &mut blank);
            }
            Message::BlankDisplays => {
                do_blank(&mut timer, &mut blank);
            }
        }
    }

    Ok(())
}
