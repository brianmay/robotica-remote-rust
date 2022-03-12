#![allow(clippy::single_component_path_imports)]
#![feature(backtrace)]

use std::env;
use std::sync::mpsc;

use anyhow::Result;
use embedded_svc::timer::OnceTimer;
use embedded_svc::timer::Timer;
use embedded_svc::timer::TimerService;
use esp_idf_svc::timer::EspTimer;
use esp_idf_svc::timer::EspTimerService;
use log::*;

mod button;
use button::ButtonId;

mod button_controllers;

mod display;
use crate::button_controllers::DisplayState;
use crate::display::DisplayCommand;
use crate::messages::Message;

mod boards;
mod config;

mod input;
mod messages;
mod mqtt;
mod wifi;

const MQTT_URL: &str = env!("MQTT_URL");

#[cfg(esp32s2)]
include!(env!("EMBUILD_GENERATED_SYMBOLS_FILE"));

#[cfg(esp32s2)]
const ULP: &[u8] = include_bytes!(env!("EMBUILD_GENERATED_BIN_FILE"));

enum TimeOfDay {
    Day,
    Night,
}

struct RequestedDisplayStatus {
    time_of_day: TimeOfDay,
    forced_on: bool,
    night_timer: bool,
}

struct ActualDisplayStatus {
    timer_on: bool,
    display_on: bool,
}

impl RequestedDisplayStatus {
    fn get_timer_required(&self) -> bool {
        self.night_timer
    }

    fn get_display_required(&self) -> bool {
        matches!(self.time_of_day, TimeOfDay::Day) || self.forced_on || self.night_timer
    }

    fn turn_night_timer_on(&mut self) {
        self.night_timer = true;
    }

    fn turn_night_timer_off(&mut self) {
        self.night_timer = false;
    }
}

fn update_display(
    display: &mpsc::Sender<DisplayCommand>,
    id: usize,
    controller: &dyn button_controllers::Controller,
    state: button_controllers::DisplayState,
) {
    let icon = controller.get_icon();
    let name = controller.get_name();
    let message = DisplayCommand::DisplayState(state, icon, id, name);
    display.send(message).unwrap();
}

fn update_displays(
    display: &mpsc::Sender<DisplayCommand>,
    controllers: &[Box<dyn button_controllers::Controller>],
) {
    for (id, controller) in controllers.iter().enumerate() {
        let state = controller.get_display_state();
        update_display(display, id, controller.as_ref(), state);
    }
}

fn do_blank(
    display: &mpsc::Sender<DisplayCommand>,
    timer: &mut EspTimer,
    requested_display_status: &RequestedDisplayStatus,
    status: &mut ActualDisplayStatus,
    force_timer_reset: bool,
) {
    let timer_required = requested_display_status.get_timer_required();
    let display_required = requested_display_status.get_display_required();

    match (timer_required, status.timer_on) {
        (true, true) => {
            if force_timer_reset {
                info!("resetting blank timer");
                timer.cancel().unwrap();
                timer.after(std::time::Duration::new(10, 0)).unwrap();
                status.timer_on = true;
            }
        }
        (true, false) => {
            info!("starting blank timer");
            timer.cancel().unwrap();
            timer.after(std::time::Duration::new(10, 0)).unwrap();
            status.timer_on = true;
        }
        (false, true) => {
            info!("stopping blank timer");
            timer.cancel().unwrap();
            status.timer_on = false;
        }
        (false, false) => {}
    };

    match (display_required, status.display_on) {
        (true, false) => {
            info!("turning display on");
            status.display_on = true;
            display.send(DisplayCommand::UnBlankAll).unwrap();
        }
        (false, true) => {
            info!("turning display off");
            status.display_on = false;
            display.send(DisplayCommand::BlankAll).unwrap();
        }
        (true, true) => {}
        (false, false) => {}
    };
}

fn button_press(
    controllers: &mut [Box<dyn button_controllers::Controller>],
    id: usize,
    mqtt: &mqtt::Mqtt,
) {
    info!("Got button {} press", id);
    let controller_or_none = controllers.get_mut(id as usize);
    if let Some(controller) = controller_or_none {
        let commands = controller.get_press_commands();
        for command in commands {
            let topic = command.get_topic();
            let data = command.get_message();
            info!("Send {}: {}", topic, data);
            mqtt.publish(&topic, false, &data);
        }
    } else {
        error!("Controller for button {} does not exist", id);
    }
}

fn main() -> Result<()> {
    boards::initialize();

    let (tx, rx) = mpsc::channel();

    let (_wifi, display) = boards::configure_devices(tx.clone())?;

    let config_list = config::get_controllers_config();

    let mut controllers: Vec<Box<dyn button_controllers::Controller>> =
        config_list.iter().map(|x| x.create_controller()).collect();

    let mqtt = mqtt::Mqtt::connect(MQTT_URL, tx.clone());

    for (index, f) in controllers.iter().enumerate() {
        let subscriptions = f.get_subscriptions();
        for s in subscriptions {
            let label = mqtt::Label::Button(index, s.label);
            info!("Subscribing to {}.", s.topic);
            mqtt.subscribe(&s.topic, label);
        }
    }

    mqtt.subscribe(config::NIGHT_TOPIC, mqtt::Label::NightStatus);

    let mut timer_service = EspTimerService::new().unwrap();
    let mut timer = timer_service
        .timer(move || {
            tx.send(Message::BlankDisplays).unwrap();
        })
        .unwrap();

    let mut requested_display_status: RequestedDisplayStatus = RequestedDisplayStatus {
        time_of_day: TimeOfDay::Day,
        forced_on: false,
        night_timer: false,
    };
    let mut status: ActualDisplayStatus = ActualDisplayStatus {
        display_on: true,
        timer_on: false,
    };

    do_blank(
        &display,
        &mut timer,
        &requested_display_status,
        &mut status,
        false,
    );
    update_displays(&display, &controllers);

    let mut page = 0;

    for received in rx {
        match received {
            Message::MqttReceived(_, power, mqtt::Label::NightStatus) => {
                info!("Got night: {}", power);
                match power.as_str() {
                    "ON" => requested_display_status.time_of_day = TimeOfDay::Night,
                    "OFF" => requested_display_status.time_of_day = TimeOfDay::Day,
                    _ => {}
                };
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    false,
                );
            }
            Message::MqttReceived(topic, data, mqtt::Label::Button(id, sid)) => {
                info!("Got message: {} - {}", topic, data);
                let controller = controllers.get_mut(id as usize).unwrap();
                let old_state = controller.get_display_state();
                controller.process_message(sid, data);
                let state = controller.get_display_state();
                if id == config::NIGHT_CONTROLLER {
                    match state {
                        DisplayState::Off => requested_display_status.forced_on = false,
                        DisplayState::HardOff => requested_display_status.forced_on = false,
                        DisplayState::On => requested_display_status.forced_on = true,
                        DisplayState::OnOther => requested_display_status.forced_on = true,
                        DisplayState::Error => {}
                        DisplayState::Unknown => {}
                    }
                    do_blank(
                        &display,
                        &mut timer,
                        &requested_display_status,
                        &mut status,
                        false,
                    );
                }
                if old_state != state {
                    update_display(&display, id, controller.as_ref(), state);
                }
            }
            Message::MqttConnect => {
                info!("Got connected");
            }
            Message::MqttDisconnect => {
                info!("Got disconnected");
                for controller in controllers.iter_mut() {
                    controller.process_disconnected();
                }
                update_displays(&display, &controllers);
            }
            Message::ButtonPress(ButtonId::Physical(id)) => {
                if status.display_on {
                    let id = id + page * boards::NUM_DISPLAYS;
                    button_press(&mut controllers, id, &mqtt);
                    display.send(DisplayCommand::ButtonPressed(id)).unwrap();
                }
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonPress(ButtonId::Controller(id)) => {
                button_press(&mut controllers, id, &mqtt);
                display.send(DisplayCommand::ButtonPressed(id)).unwrap();
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonPress(ButtonId::PageUp) => {
                info!("got page up");
                display.send(DisplayCommand::PageUp).unwrap();
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonPress(ButtonId::PageDown) => {
                info!("got page down");
                display.send(DisplayCommand::PageDown).unwrap();
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonRelease(ButtonId::Physical(id)) => {
                info!("Got button release");
                let id = id + page * boards::NUM_DISPLAYS;
                display.send(DisplayCommand::ButtonReleased(id)).unwrap();
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonRelease(ButtonId::Controller(id)) => {
                info!("Got button release");
                display.send(DisplayCommand::ButtonReleased(id)).unwrap();
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::ButtonRelease(_) => {
                info!("Got button release");
                requested_display_status.turn_night_timer_on();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::BlankDisplays => {
                info!("Got blank display timer");
                requested_display_status.turn_night_timer_off();
                do_blank(
                    &display,
                    &mut timer,
                    &requested_display_status,
                    &mut status,
                    true,
                );
            }
            Message::PageIsDisplayed(number) => {
                page = number;
            }
        }
    }

    Ok(())
}
