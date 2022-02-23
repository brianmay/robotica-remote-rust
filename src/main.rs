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

enum RequestedDisplayStatus {
    Day,
    Night(bool),
}

struct ActualDisplayStatus {
    timer_on: bool,
    display_on: bool,
}

impl RequestedDisplayStatus {
    fn get_timer_required(&self) -> bool {
        matches!(self, RequestedDisplayStatus::Night(true))
    }

    fn get_display_required(&self) -> bool {
        match self {
            RequestedDisplayStatus::Day => true,
            RequestedDisplayStatus::Night(display_on) => *display_on,
        }
    }

    fn reset_timer(&mut self) {
        if let RequestedDisplayStatus::Night(false) = self {
            *self = RequestedDisplayStatus::Night(true);
        }
    }

    fn got_timer(&mut self) {
        if let RequestedDisplayStatus::Night(true) = self {
            *self = RequestedDisplayStatus::Night(false);
        }
    }
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

    mqtt.subscribe(
        "state/Brian/Night/power",
        mqtt::Label {
            component_id: 100,
            subscription_id: 0,
        },
    );

    let update_controller =
        |id: u32, controller: &dyn button_controllers::Controller, status: &ActualDisplayStatus| {
            let state = controller.get_display_state();
            let icon = controller.get_icon();
            if status.display_on {
                let message = DisplayCommand::DisplayState(state, icon, id as u32);
                display.send(message).unwrap();
            }
        };

    let update_displays = |controllers: &Vec<Box<dyn button_controllers::Controller>>,
                           status: &ActualDisplayStatus| {
        for (id, controller) in controllers.iter().enumerate() {
            update_controller(id as u32, controller.as_ref(), status);
        }
    };

    let do_blank = |controllers: &Vec<Box<dyn button_controllers::Controller>>,
                    timer: &mut EspTimer,
                    requested_display_status: &RequestedDisplayStatus,
                    status: &mut ActualDisplayStatus| {
        let timer_required = requested_display_status.get_timer_required();
        let display_required = requested_display_status.get_display_required();

        match (timer_required, status.timer_on) {
            (true, false) => {
                timer.cancel().unwrap();
                timer.after(std::time::Duration::new(10, 0)).unwrap();
                status.timer_on = true;
            }
            (false, true) => {
                timer.cancel().unwrap();
                status.timer_on = false;
            }
            (true, true) => {}
            (false, false) => {}
        };

        match (display_required, status.display_on) {
            (true, false) => {
                status.display_on = true;
                update_displays(controllers, status);
            }
            (false, true) => {
                status.display_on = false;
                display.send(DisplayCommand::BlankAll).unwrap();
            }
            (true, true) => {}
            (false, false) => {}
        };
    };

    let mut timer_service = EspTimerService::new().unwrap();
    let mut timer = timer_service
        .timer(move || {
            tx.send(Message::BlankDisplays).unwrap();
        })
        .unwrap();
    timer.after(std::time::Duration::new(10, 0)).unwrap();

    let mut requested_display_status: RequestedDisplayStatus = RequestedDisplayStatus::Day;
    let mut status: ActualDisplayStatus = ActualDisplayStatus {
        display_on: true,
        timer_on: false,
    };

    for received in rx {
        match received {
            Message::MqttReceived(
                _,
                power,
                mqtt::Label {
                    component_id: 100,
                    subscription_id: _,
                },
            ) => {
                requested_display_status = match power.as_str() {
                    "ON" => RequestedDisplayStatus::Night(status.display_on),
                    _ => RequestedDisplayStatus::Day,
                };
                do_blank(
                    &controllers,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                );
            }
            Message::MqttReceived(topic, data, label) => {
                info!("Got message {}: {}", topic, data);
                let id = label.component_id;
                let sid = label.subscription_id;
                let controller = controllers.get_mut(id as usize).unwrap();
                controller.process_message(sid, data);
                update_controller(id, controller.as_ref(), &status);
            }
            Message::MqttConnect => {
                info!("Got connected");
            }
            Message::MqttDisconnect => {
                info!("Got disconnected");
                for controller in controllers.iter_mut() {
                    controller.process_disconnected();
                }
                update_displays(&controllers, &status);
            }
            Message::ButtonPress(id) => {
                info!("Got button {} press", id);
                requested_display_status.reset_timer();
                do_blank(
                    &controllers,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                );

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
                requested_display_status.reset_timer();
                do_blank(
                    &controllers,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                );
            }
            Message::BlankDisplays => {
                info!("Got blank display timer");
                requested_display_status.got_timer();
                do_blank(
                    &controllers,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                );
            }
        }
    }

    Ok(())
}
